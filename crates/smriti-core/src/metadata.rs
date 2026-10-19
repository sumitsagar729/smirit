//! Reads when, where and with what a photo or video was taken.
//!
//! Date priority: EXIF / container date → date in the file name → file time.
//! Header-only reads: full decoding happens later, for thumbnails only.

use std::path::Path;

use bytes::Bytes;
use chrono::{DateTime, NaiveDateTime};
use nom_exif::{ExifTag, MediaKind, MediaParser, MediaSource, TrackInfoTag};

use crate::filename_date;
use crate::media::Kind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateSource {
    Exif,
    Video,
    Filename,
    Mtime,
}

impl DateSource {
    pub fn as_str(self) -> &'static str {
        match self {
            DateSource::Exif => "exif",
            DateSource::Video => "video",
            DateSource::Filename => "filename",
            DateSource::Mtime => "mtime",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Meta {
    pub taken_at: Option<NaiveDateTime>,
    pub date_source: Option<DateSource>,
    pub tz_offset_min: Option<i32>,
    /// Display size, already swapped for 90°/270° EXIF orientation.
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub orientation: Option<u16>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub camera: Option<String>,
    pub duration_ms: Option<u64>,
}

/// Never fails: unreadable metadata just leaves fields empty, and the date
/// always falls back to the file name or the file's modified time.
/// `data` is the photo's bytes when the caller already read the file.
pub fn read(parser: &mut MediaParser, path: &Path, kind: Kind, mtime: i64, data: Option<&Bytes>) -> Meta {
    let mut meta = match kind {
        Kind::Photo => read_photo(parser, path, data),
        Kind::Video => read_video(parser, path),
    };

    if meta.taken_at.is_none()
        && let Some(dt) = path.file_name().and_then(|n| n.to_str()).and_then(filename_date::parse)
    {
        meta.taken_at = Some(dt);
        meta.date_source = Some(DateSource::Filename);
    }
    if meta.taken_at.is_none()
        && let Some(dt) = DateTime::from_timestamp(mtime, 0)
    {
        meta.taken_at = Some(dt.with_timezone(&chrono::Local).naive_local());
        meta.date_source = Some(DateSource::Mtime);
    }
    meta
}

fn plausible(dt: &NaiveDateTime) -> bool {
    use chrono::Datelike;
    // Cameras with a dead clock battery report 1970, 1980 or 2000-01-01.
    dt.year() >= 1990 && dt.year() <= chrono::Local::now().year() + 1 && !(dt.year() == 2000 && dt.ordinal() == 1)
}

fn read_photo(parser: &mut MediaParser, path: &Path, data: Option<&Bytes>) -> Meta {
    let mut meta = Meta::default();
    let size = match data {
        Some(d) => imagesize::blob_size(d),
        None => imagesize::size(path),
    };
    if let Ok(size) = size {
        meta.width = Some(size.width as u32);
        meta.height = Some(size.height as u32);
    }

    let exif: Option<nom_exif::Exif> = match data {
        Some(d) => MediaSource::from_memory(d.clone())
            .ok()
            .filter(|ms| ms.kind() == MediaKind::Image)
            .and_then(|ms| parser.parse_exif(ms).ok()),
        None => MediaSource::open(path)
            .ok()
            .filter(|ms| ms.kind() == MediaKind::Image)
            .and_then(|ms| parser.parse_exif(ms).ok()),
    }
    .map(Into::into);
    let Some(exif) = exif else { return meta };

    let date = [ExifTag::DateTimeOriginal, ExifTag::CreateDate, ExifTag::ModifyDate]
        .into_iter()
        .find_map(|t| exif.get(t).and_then(|v| v.as_datetime()));
    if let Some(dt) = date {
        let naive = dt.into_naive();
        if plausible(&naive) {
            meta.taken_at = Some(naive);
            meta.date_source = Some(DateSource::Exif);
            meta.tz_offset_min = dt.aware().map(|a| a.offset().local_minus_utc() / 60);
        }
    }

    if let Some(gps) = exif.gps_info()
        && let (Some(lat), Some(lon)) = (gps.latitude_decimal(), gps.longitude_decimal())
        && valid_coords(lat, lon)
    {
        meta.lat = Some(lat);
        meta.lon = Some(lon);
    }

    let text = |t| exif.get(t).and_then(|v| v.as_str()).map(|s| s.trim_matches(['\0', ' ']).to_string());
    meta.camera = camera_name(text(ExifTag::Make), text(ExifTag::Model));

    if meta.width.is_none() {
        meta.width = exif.get(ExifTag::ExifImageWidth).and_then(|v| v.as_u32());
        meta.height = exif.get(ExifTag::ExifImageHeight).and_then(|v| v.as_u32());
    }
    meta.orientation = exif.get(ExifTag::Orientation).and_then(|v| v.as_u16()).filter(|o| (1..=8).contains(o));
    if matches!(meta.orientation, Some(5..=8)) {
        std::mem::swap(&mut meta.width, &mut meta.height);
    }
    meta
}

fn read_video(parser: &mut MediaParser, path: &Path) -> Meta {
    let mut meta = Meta::default();
    let track = MediaSource::open(path)
        .ok()
        .filter(|ms| ms.kind() == MediaKind::Track)
        .and_then(|ms| parser.parse_track(ms).ok());
    let Some(track) = track else { return meta };

    if let Some(dt) = track.get(TrackInfoTag::CreateDate).and_then(|v| v.as_datetime()) {
        // QuickTime stores UTC; show the local wall-clock time it was filmed.
        let local = match dt.aware() {
            Some(a) => a.with_timezone(&chrono::Local).naive_local(),
            None => dt.into_naive(),
        };
        if plausible(&local) {
            meta.taken_at = Some(local);
            meta.date_source = Some(DateSource::Video);
        }
    }
    meta.duration_ms = track.get(TrackInfoTag::DurationMs).and_then(|v| v.as_u64());
    meta.width = track.get(TrackInfoTag::Width).and_then(|v| v.as_u32()).filter(|w| *w > 0);
    meta.height = track.get(TrackInfoTag::Height).and_then(|v| v.as_u32()).filter(|h| *h > 0);
    if let Some(gps) = track.gps_info()
        && let (Some(lat), Some(lon)) = (gps.latitude_decimal(), gps.longitude_decimal())
        && valid_coords(lat, lon)
    {
        meta.lat = Some(lat);
        meta.lon = Some(lon);
    }
    let text = |t| track.get(t).and_then(|v| v.as_str()).map(str::to_string);
    meta.camera = camera_name(text(TrackInfoTag::Make), text(TrackInfoTag::Model));
    meta
}

/// (0, 0) is what broken GPS chips write; it is never a real family photo.
fn valid_coords(lat: f64, lon: f64) -> bool {
    lat.abs() <= 90.0 && lon.abs() <= 180.0 && (lat.abs() > 0.01 || lon.abs() > 0.01)
}

/// "Canon" + "Canon EOS 1500D" → "Canon EOS 1500D"; "vivo" + "V2027" → "vivo V2027".
fn camera_name(make: Option<String>, model: Option<String>) -> Option<String> {
    match (make.filter(|s| !s.is_empty()), model.filter(|s| !s.is_empty())) {
        (Some(make), Some(model)) if model.to_lowercase().starts_with(&make.to_lowercase()) => Some(model),
        (Some(make), Some(model)) => Some(format!("{make} {model}")),
        (None, Some(model)) => Some(model),
        (Some(make), None) => Some(make),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_names_are_deduplicated() {
        assert_eq!(
            camera_name(Some("Canon".into()), Some("Canon EOS 1500D".into())).as_deref(),
            Some("Canon EOS 1500D")
        );
        assert_eq!(camera_name(Some("vivo".into()), Some("V2027".into())).as_deref(), Some("vivo V2027"));
        assert_eq!(camera_name(None, None), None);
    }

    #[test]
    fn rejects_null_island() {
        assert!(!valid_coords(0.0, 0.0));
        assert!(valid_coords(12.97, 77.59));
    }

    #[test]
    fn falls_back_to_filename_then_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let mut parser = MediaParser::new();

        let named = dir.path().join("IMG-20210512-WA0003.jpg");
        image::RgbImage::new(64, 48).save(&named).unwrap();
        let m = read(&mut parser, &named, Kind::Photo, 0, None);
        assert_eq!(m.date_source, Some(DateSource::Filename));
        assert_eq!((m.width, m.height), (Some(64), Some(48)));

        let plain = dir.path().join("DSC_0001.jpg");
        image::RgbImage::new(10, 10).save(&plain).unwrap();
        let m =
            read(&mut parser, &plain, Kind::Photo, 1_700_000_000, Some(&Bytes::from(std::fs::read(&plain).unwrap())));
        assert_eq!(m.date_source, Some(DateSource::Mtime));
    }
}
