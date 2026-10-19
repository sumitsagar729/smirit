//! Dates hidden in file names. WhatsApp strips EXIF, so `IMG-20210512-WA0003.jpg`
//! is often the only record of when a photo was taken.

use std::sync::LazyLock;

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use regex::Regex;

// 2021-05-12 / 20210512 / 2021_05_12, optionally followed by a time
// 093015 / 09-30-15 / 09.30.15. Digits must not be glued to other digits.
static PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        (?:^|[^0-9])
        (?P<y>(?:19|20)\d{2}) [-_.]? (?P<m>0[1-9]|1[0-2]) [-_.]? (?P<d>0[1-9]|[12]\d|3[01])
        (?: [-_.\sT]? (?P<H>[01]\d|2[0-3]) [-_.:]? (?P<M>[0-5]\d) [-_.:]? (?P<S>[0-5]\d) )?
        (?:[^0-9]|$)",
    )
    .expect("valid regex")
});

/// Earliest plausible year for a family photo library.
const MIN_YEAR: i32 = 1990;

pub fn parse(file_name: &str) -> Option<NaiveDateTime> {
    let c = PATTERN.captures(file_name)?;
    let num = |k: &str| c.name(k).and_then(|m| m.as_str().parse::<u32>().ok());
    let year = num("y")? as i32;
    if year < MIN_YEAR || year > chrono::Local::now().year() + 1 {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(year, num("m")?, num("d")?)?;
    let time = match (num("H"), num("M"), num("S")) {
        (Some(h), Some(m), Some(s)) => NaiveTime::from_hms_opt(h, m, s)?,
        _ => NaiveTime::from_hms_opt(12, 0, 0)?,
    };
    Some(date.and_time(time))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Option<String> {
        parse(s).map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
    }

    #[test]
    fn phone_camera_names() {
        assert_eq!(p("IMG_20210512_093015.jpg").as_deref(), Some("2021-05-12 09:30:15"));
        assert_eq!(p("20241110_090228.jpg").as_deref(), Some("2024-11-10 09:02:28"));
        assert_eq!(p("IMG20220102134047.jpg").as_deref(), Some("2022-01-02 13:40:47"));
        assert_eq!(p("PXL_20230704_101112345.jpg").as_deref(), Some("2023-07-04 12:00:00"));
        assert_eq!(p("VID_20211009_181500.mp4").as_deref(), Some("2021-10-09 18:15:00"));
    }

    #[test]
    fn whatsapp_and_screenshots() {
        assert_eq!(p("IMG-20210512-WA0003.jpg").as_deref(), Some("2021-05-12 12:00:00"));
        assert_eq!(p("VID-20200101-WA0011.mp4").as_deref(), Some("2020-01-01 12:00:00"));
        assert_eq!(p("Screenshot_2022-03-04-18-22-10.png").as_deref(), Some("2022-03-04 18:22:10"));
    }

    #[test]
    fn rejects_non_dates() {
        assert_eq!(p("DSC_4521.JPG"), None);
        assert_eq!(p("IMG_7684.JPG"), None);
        assert_eq!(p("3c0042be-d957-43b2.jpg"), None);
        assert_eq!(p("photo_20211345.jpg"), None); // month 13
        assert_eq!(p("1234520210512.jpg"), None); // glued to other digits
    }
}
