//! Places and trips. Every photo with GPS gets the town it was taken in;
//! photos without GPS borrow a town named in their folder. Home is where most
//! days' photos are taken; runs of days more than 70 km away become trips,
//! and back-to-back trips merge into one journey with several stops.

use std::collections::{BTreeMap, HashMap};

use chrono::NaiveDate;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::geo::{self, Geocoder, Place};
use crate::{Error, Id, Library, Result};

/// Days with photos further than this from home count as away.
const AWAY_KM: f64 = 70.0;
/// Away days this close together belong to one leg...
const LEG_GAP_DAYS: i64 = 2;
/// ...unless the family moved further than this between them.
const LEG_MAX_JUMP_KM: f64 = 400.0;
/// Legs this close together are one journey.
const JOURNEY_GAP_DAYS: i64 = 4;
/// Fewer photos than this away from home isn't a trip.
const MIN_TRIP_ITEMS: u32 = 10;
/// Bump to recompute places when the rules above change.
pub const PLACES_VERSION: &str = "1";

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Home {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub country_code: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSummary {
    pub id: Id,
    pub name: String,
    pub admin1: Option<String>,
    pub country: Option<String>,
    pub lat: f64,
    pub lon: f64,
    pub count: u32,
    pub cover_asset_id: Option<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Stop {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub start: String,
    pub end: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Trip {
    pub id: Id,
    pub title: String,
    /// `YYYY-MM-DD`, inclusive.
    pub start: String,
    pub end: String,
    pub distance_km: Option<f64>,
    pub stops: Vec<Stop>,
    pub count: u32,
    pub cover_asset_id: Option<Id>,
    pub user_edited: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlacesReport {
    pub placed: u32,
    pub places: u32,
    pub trips: u32,
    pub ms: u32,
}

struct Row {
    id: Id,
    gps: Option<(f64, f64)>,
    folder: String,
    date: Option<NaiveDate>,
    place: Option<Id>,
    source: Option<String>,
}

/// One trip before it's stored.
struct Found {
    start: NaiveDate,
    end: NaiveDate,
    stops: Vec<Stop>,
    distance_km: f64,
}

impl Library {
    /// Re-derives every asset's place, home and the trips. Fast (tens of
    /// milliseconds for 40k photos) since geocoding is in memory and cached.
    pub fn update_places(&self, geo: &Geocoder) -> Result<PlacesReport> {
        let started = std::time::Instant::now();
        let rows = self.place_rows()?;

        // 1. Towns for GPS photos (cached by ~1 km cell).
        let mut by_cell: HashMap<(i64, i64), Option<Place>> = HashMap::new();
        let mut resolved: Vec<Option<(Place, &'static str)>> = rows
            .iter()
            .map(|r| {
                let (lat, lon) = r.gps?;
                let key = ((lat * 100.0).round() as i64, (lon * 100.0).round() as i64);
                by_cell.entry(key).or_insert_with(|| geo.reverse(lat, lon)).clone().map(|p| (p, "gps"))
            })
            .collect();

        // 2. Home: the 0.1-degree cell with photos on the most days.
        let home = find_home(&rows, geo);
        let home_cc = home.as_ref().map(|h| h.country_code.clone());

        // 3. Towns named in folders, for photos without GPS (home country only).
        if let Some(cc) = &home_cc {
            let mut by_folder: HashMap<&str, Option<Place>> = HashMap::new();
            for (r, slot) in rows.iter().zip(resolved.iter_mut()) {
                if slot.is_some() || r.folder.is_empty() {
                    continue;
                }
                let found = by_folder
                    .entry(&r.folder)
                    .or_insert_with(|| {
                        // Deepest folder that names a town wins.
                        r.folder.rsplit('/').find_map(|part| geo.find_in_text(part, cc))
                    })
                    .clone();
                *slot = found.map(|p| (p, "folder"));
            }
        }

        // 4. Store places and assignments.
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let mut ids: HashMap<(String, Option<String>, Option<String>), Id> = HashMap::new();
        let mut placed = 0;
        {
            let mut upsert = tx.prepare_cached(
                "INSERT INTO places (name, admin1, country, lat, lon, country_code) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (name, admin1, country) DO UPDATE SET lat = excluded.lat, lon = excluded.lon
                 RETURNING id",
            )?;
            let mut set = tx.prepare_cached("UPDATE assets SET place_id = ?2, place_source = ?3 WHERE id = ?1")?;
            for (r, res) in rows.iter().zip(&resolved) {
                let (place, source) = match res {
                    Some((p, src)) => {
                        let key = (p.name.clone(), p.admin1.clone(), p.country.clone());
                        let id = match ids.get(&key) {
                            Some(&id) => id,
                            None => {
                                let id: Id = upsert.query_row(
                                    params![p.name, p.admin1, p.country, p.lat, p.lon, p.country_code],
                                    |r| r.get(0),
                                )?;
                                ids.insert(key, id);
                                id
                            }
                        };
                        placed += 1;
                        (Some(id), Some(*src))
                    }
                    None => (None, None),
                };
                if r.place != place || r.source.as_deref() != source {
                    set.execute(params![r.id, place, source])?;
                }
            }
        }
        tx.execute("DELETE FROM places WHERE id NOT IN (SELECT place_id FROM assets WHERE place_id IS NOT NULL)", [])?;
        match &home {
            Some(h) => tx.execute(
                "INSERT INTO settings (key, value) VALUES ('home', ?1) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [serde_json::to_string(h).unwrap_or_default()],
            )?,
            None => tx.execute("DELETE FROM settings WHERE key = 'home'", [])?,
        };
        tx.commit()?;
        drop(conn);

        // 5. Trips.
        let trips = match &home {
            Some(h) => detect_trips(&rows, &resolved, h),
            None => Vec::new(),
        };
        let n_trips = trips.len() as u32;
        self.store_trips(trips)?;
        self.conn().execute(
            "INSERT INTO settings (key, value) VALUES ('places_version', ?1) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [PLACES_VERSION],
        )?;

        let report =
            PlacesReport { placed, places: ids.len() as u32, trips: n_trips, ms: started.elapsed().as_millis() as u32 };
        tracing::info!(?report, home = ?home.map(|h| h.name), "places updated");
        Ok(report)
    }

    /// True when places were never computed with the current rules.
    pub fn places_outdated(&self) -> Result<bool> {
        let v: Option<String> = self
            .read()
            .query_row("SELECT value FROM settings WHERE key = 'places_version'", [], |r| r.get(0))
            .optional()?;
        Ok(v.as_deref() != Some(PLACES_VERSION))
    }

    fn place_rows(&self) -> Result<Vec<Row>> {
        let conn = self.read();
        let mut stmt = conn.prepare(
            "SELECT id, lat, lon, rel_path, taken_at, place_id, place_source FROM assets WHERE status = 'active'",
        )?;
        let rows = stmt
            .query_map([], |r| {
                let lat: Option<f64> = r.get(1)?;
                let lon: Option<f64> = r.get(2)?;
                let rel: String = r.get(3)?;
                Ok(Row {
                    id: r.get(0)?,
                    gps: lat.zip(lon),
                    folder: rel.rsplit_once('/').map(|(dir, _)| dir.to_string()).unwrap_or_default(),
                    date: r
                        .get::<_, Option<String>>(4)?
                        .as_deref()
                        .and_then(|s| NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d").ok()),
                    place: r.get(5)?,
                    source: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Replaces detected trips, keeping ids and titles the user edited when
    /// a new trip overlaps an old one.
    fn store_trips(&self, found: Vec<Found>) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let old: Vec<(Id, String, String, bool)> = tx
            .prepare("SELECT id, start_date, end_date, user_edited FROM trips")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut kept = Vec::new();
        tx.execute("UPDATE assets SET trip_id = NULL WHERE trip_id IS NOT NULL", [])?;
        for t in found {
            let (start, end) = (t.start.to_string(), t.end.to_string());
            let title = trip_title(&t.stops);
            let stops = serde_json::to_string(&t.stops).unwrap_or_else(|_| "[]".into());
            let reuse = old.iter().find(|(id, s, e, _)| !kept.contains(id) && *s <= end && start <= *e);
            let id = match reuse {
                Some((id, _, _, edited)) => {
                    tx.execute(
                        "UPDATE trips SET start_date = ?2, end_date = ?3, home_distance_km = ?4, stops = ?5,
                             title = CASE WHEN ?6 THEN title ELSE ?7 END WHERE id = ?1",
                        params![id, start, end, t.distance_km.round(), stops, edited, title],
                    )?;
                    *id
                }
                None => {
                    tx.execute(
                        "INSERT INTO trips (title, start_date, end_date, home_distance_km, stops) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![title, start, end, t.distance_km.round(), stops],
                    )?;
                    Id(tx.last_insert_rowid())
                }
            };
            kept.push(id);
            // Everything from those days belongs to the trip, GPS or not.
            tx.execute(
                "UPDATE assets SET trip_id = ?1
                 WHERE status = 'active' AND substr(taken_at, 1, 10) BETWEEN ?2 AND ?3",
                params![id, start, end],
            )?;
            // Cover: the photo with the most named people, else the first with a thumbnail.
            tx.execute(
                "UPDATE trips SET item_count = (SELECT count(*) FROM assets WHERE trip_id = ?1),
                     cover_asset_id = (
                         SELECT a.id FROM assets a LEFT JOIN faces f ON f.asset_id = a.id AND f.person_id IS NOT NULL
                         WHERE a.trip_id = ?1 AND a.thumb_state = 'ok' AND a.kind = 'photo'
                         GROUP BY a.id ORDER BY count(f.id) DESC, a.lat IS NULL, a.taken_at LIMIT 1)
                 WHERE id = ?1",
                [id],
            )?;
        }
        let keep = kept.iter().map(|i| i.0.to_string()).collect::<Vec<_>>().join(",");
        tx.execute(&format!("DELETE FROM trips WHERE id NOT IN ({})", if keep.is_empty() { "-1" } else { &keep }), [])?;
        tx.commit()?;
        Ok(())
    }

    pub fn home(&self) -> Result<Option<Home>> {
        let v: Option<String> =
            self.read().query_row("SELECT value FROM settings WHERE key = 'home'", [], |r| r.get(0)).optional()?;
        Ok(v.and_then(|s| serde_json::from_str(&s).ok()))
    }

    pub fn places(&self) -> Result<Vec<PlaceSummary>> {
        let conn = self.read();
        let mut stmt = conn.prepare_cached(
            "SELECT p.id, p.name, p.admin1, p.country, p.lat, p.lon, count(a.id),
                    (SELECT b.id FROM assets b WHERE b.place_id = p.id AND b.thumb_state = 'ok' AND b.kind = 'photo'
                     ORDER BY b.taken_at DESC LIMIT 1)
             FROM places p JOIN assets a ON a.place_id = p.id AND a.status = 'active' AND a.is_hidden = 0
             GROUP BY p.id ORDER BY count(a.id) DESC",
        )?;
        let places = stmt
            .query_map([], |r| {
                Ok(PlaceSummary {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    admin1: r.get(2)?,
                    country: r.get(3)?,
                    lat: r.get(4)?,
                    lon: r.get(5)?,
                    count: r.get(6)?,
                    cover_asset_id: r.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(places)
    }

    pub fn trips(&self) -> Result<Vec<Trip>> {
        let conn = self.read();
        let mut stmt = conn.prepare_cached(
            "SELECT id, title, start_date, end_date, home_distance_km, stops, item_count, cover_asset_id, user_edited
             FROM trips ORDER BY start_date DESC",
        )?;
        let trips = stmt
            .query_map([], |r| {
                let stops: String = r.get(5)?;
                Ok(Trip {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    start: r.get(2)?,
                    end: r.get(3)?,
                    distance_km: r.get(4)?,
                    stops: serde_json::from_str(&stops).unwrap_or_default(),
                    count: r.get(6)?,
                    cover_asset_id: r.get(7)?,
                    user_edited: r.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(trips)
    }

    pub fn trip(&self, id: Id) -> Result<Trip> {
        self.trips()?.into_iter().find(|t| t.id == id).ok_or_else(|| Error::NotFound(format!("trip {id}")))
    }

    pub fn rename_trip(&self, id: Id, title: &str) -> Result<()> {
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        if title.is_empty() || title.chars().count() > 100 {
            return Err(Error::Invalid("Please enter a trip name".into()));
        }
        let n =
            self.conn().execute("UPDATE trips SET title = ?2, user_edited = 1 WHERE id = ?1", params![id, title])?;
        if n == 0 {
            return Err(Error::NotFound(format!("trip {id}")));
        }
        Ok(())
    }
}

fn find_home(rows: &[Row], geo: &Geocoder) -> Option<Home> {
    let mut days: HashMap<(i64, i64), std::collections::HashSet<NaiveDate>> = HashMap::new();
    for r in rows {
        if let (Some((lat, lon)), Some(d)) = (r.gps, r.date) {
            days.entry(((lat * 10.0).round() as i64, (lon * 10.0).round() as i64)).or_default().insert(d);
        }
    }
    let (&(la, lo), _) = days.iter().max_by_key(|(k, v)| (v.len(), std::cmp::Reverse(**k)))?;
    let (lat, lon) = (la as f64 / 10.0, lo as f64 / 10.0);
    let place = geo.reverse(lat, lon);
    Some(Home {
        name: place.as_ref().map_or_else(|| "Home".into(), |p| p.name.clone()),
        lat,
        lon,
        country_code: place.map(|p| p.country_code).unwrap_or_default(),
    })
}

/// Away days -> legs -> journeys, as described in the module docs.
fn detect_trips(rows: &[Row], resolved: &[Option<(Place, &'static str)>], home: &Home) -> Vec<Found> {
    // Per day: every located item, with the town it's in.
    let mut by_day: BTreeMap<NaiveDate, Vec<(f64, f64, &Place)>> = BTreeMap::new();
    for (r, res) in rows.iter().zip(resolved) {
        let (Some(d), Some((p, _))) = (r.date, res) else { continue };
        let (lat, lon) = r.gps.unwrap_or((p.lat, p.lon));
        by_day.entry(d).or_default().push((lat, lon, p));
    }
    let home_pt = (home.lat, home.lon);

    // Away days: mostly far from home. The day's place is its most common far town.
    struct Day<'a> {
        date: NaiveDate,
        at: (f64, f64),
        place: &'a Place,
        count: u32,
        km: f64,
    }
    let mut away: Vec<Day<'_>> = Vec::new();
    for (&date, pts) in &by_day {
        let far: Vec<&(f64, f64, &Place)> = pts.iter().filter(|p| geo::km((p.0, p.1), home_pt) > AWAY_KM).collect();
        if far.len() < 2.max(pts.len() / 3) {
            continue;
        }
        let mut towns: HashMap<&str, (u32, &Place)> = HashMap::new();
        for p in &far {
            towns.entry(p.2.name.as_str()).or_insert((0, p.2)).0 += 1;
        }
        let Some((_, (_, place))) = towns.into_iter().max_by_key(|(name, (n, _))| (*n, std::cmp::Reverse(*name)))
        else {
            continue;
        };
        let at = (place.lat, place.lon);
        away.push(Day { date, at, place, count: far.len() as u32, km: geo::km(at, home_pt) });
    }

    // Legs, then journeys.
    let mut legs: Vec<Vec<Day<'_>>> = Vec::new();
    for day in away {
        match legs.last_mut() {
            Some(leg)
                if (day.date - leg.last().unwrap().date).num_days() <= LEG_GAP_DAYS
                    && geo::km(day.at, leg.last().unwrap().at) < LEG_MAX_JUMP_KM =>
            {
                leg.push(day)
            }
            _ => legs.push(vec![day]),
        }
    }
    let mut journeys: Vec<Vec<Day<'_>>> = Vec::new();
    for leg in legs {
        match journeys.last_mut() {
            Some(j) if (leg[0].date - j.last().unwrap().date).num_days() <= JOURNEY_GAP_DAYS => j.extend(leg),
            _ => journeys.push(leg),
        }
    }

    journeys
        .into_iter()
        .filter_map(|days| {
            let count: u32 = days.iter().map(|d| d.count).sum();
            if count < MIN_TRIP_ITEMS {
                return None;
            }
            let mut stops: Vec<Stop> = Vec::new();
            for d in &days {
                let date = d.date.to_string();
                match stops.last_mut() {
                    Some(s) if s.name == d.place.name => {
                        s.end = date;
                        s.count += d.count;
                    }
                    _ => stops.push(Stop {
                        name: d.place.name.clone(),
                        lat: d.place.lat,
                        lon: d.place.lon,
                        start: date.clone(),
                        end: date,
                        count: d.count,
                    }),
                }
            }
            Some(Found {
                start: days[0].date,
                end: days.last().unwrap().date,
                distance_km: days.iter().map(|d| d.km).fold(0.0, f64::max),
                stops,
            })
        })
        .collect()
}

/// "Ooty", "Coorg & Mysuru", or "Goa, Gokarna & Udupi" (busiest stops).
fn trip_title(stops: &[Stop]) -> String {
    let mut names: Vec<(&str, u32)> = Vec::new();
    for s in stops {
        match names.iter_mut().find(|(n, _)| *n == s.name) {
            Some(e) => e.1 += s.count,
            None => names.push((&s.name, s.count)),
        }
    }
    // Keep the 3 busiest, in travel order.
    let mut top: Vec<(usize, &str, u32)> = names.iter().enumerate().map(|(i, (n, c))| (i, *n, *c)).collect();
    top.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
    top.truncate(3);
    top.sort_by_key(|t| t.0);
    let n: Vec<&str> = top.iter().map(|t| t.1).collect();
    match n.as_slice() {
        [] => "Trip".into(),
        [a] => a.to_string(),
        [a, b] => format!("{a} & {b}"),
        [rest @ .., last] => format!("{} & {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::tests::sample;

    /// (folder, taken_at, GPS)
    type Item<'a> = (&'a str, &'a str, Option<(f64, f64)>);

    fn library_with_photos(items: &[Item<'_>]) -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(dir.path().join("lib")).unwrap();
        let photos = dir.path().join("photos");
        std::fs::create_dir_all(&photos).unwrap();
        let src = lib.add_source(&photos).unwrap();
        {
            let conn = lib.conn();
            for (i, (folder, date, gps)) in items.iter().enumerate() {
                conn.execute(
                    "INSERT INTO assets (source_id, rel_path, size, mtime, kind, taken_at, lat, lon, thumb_state)
                     VALUES (?1, ?2, 1, 0, 'photo', ?3, ?4, ?5, 'ok')",
                    params![src.id, format!("{folder}/{i}.jpg"), date, gps.map(|g| g.0), gps.map(|g| g.1)],
                )
                .unwrap();
            }
        }
        (dir, lib)
    }

    const BLR: Option<(f64, f64)> = Some((12.97, 77.59));
    const OOTY: Option<(f64, f64)> = Some((11.41, 76.70));

    #[test]
    fn finds_home_trips_and_folder_places() {
        let mut items: Vec<Item<'_>> = Vec::new();
        // Lots of home days.
        for d in ["2021-01-05", "2021-02-07", "2021-03-09", "2021-04-11", "2021-06-01"] {
            for _ in 0..5 {
                items.push(("2021", d, BLR));
            }
        }
        // A two-day trip to Ooty: 12 photos with GPS, 3 without.
        for _ in 0..6 {
            items.push(("ooty - nov 2021", "2021-11-20T10:00:00", OOTY));
            items.push(("ooty - nov 2021", "2021-11-21T10:00:00", OOTY));
        }
        for _ in 0..3 {
            items.push(("ooty - nov 2021", "2021-11-21T18:00:00", None));
        }
        // A WhatsApp forward at home the same weekend joins the trip by date.
        items.push(("whatsapp", "2021-11-21T20:00:00", None));
        let (_dir, lib) = library_with_photos(&items);

        let r = lib.update_places(&sample()).unwrap();
        assert_eq!(r.trips, 1);
        assert_eq!(lib.home().unwrap().unwrap().name, "Bengaluru");
        let trips = lib.trips().unwrap();
        let t = &trips[0];
        assert_eq!(
            (t.title.as_str(), t.start.as_str(), t.end.as_str(), t.count),
            ("Ooty", "2021-11-20", "2021-11-21", 16)
        );
        assert!(t.distance_km.unwrap() > 150.0);
        assert!(t.cover_asset_id.is_some());

        // GPS-less photos in the "ooty" folder are placed in Ooty too.
        let places = lib.places().unwrap();
        let ooty = places.iter().find(|p| p.name == "Ooty").unwrap();
        assert_eq!((ooty.count, ooty.admin1.as_deref()), (15, Some("Tamil Nadu")));

        // A renamed trip keeps its name and id when trips are recomputed.
        lib.rename_trip(t.id, "Nilgiris weekend").unwrap();
        lib.update_places(&sample()).unwrap();
        let again = lib.trips().unwrap();
        assert_eq!((again[0].id, again[0].title.as_str()), (t.id, "Nilgiris weekend"));
    }

    #[test]
    fn titles_read_naturally() {
        let stop = |name: &str, count| Stop {
            name: name.into(),
            lat: 0.0,
            lon: 0.0,
            start: String::new(),
            end: String::new(),
            count,
        };
        assert_eq!(trip_title(&[stop("Coorg", 30), stop("Mysuru", 12)]), "Coorg & Mysuru");
        assert_eq!(
            trip_title(&[stop("Goa", 50), stop("Gokarna", 20), stop("Hubli", 2), stop("Udupi", 10), stop("Goa", 5)]),
            "Goa, Gokarna & Udupi"
        );
    }
}
