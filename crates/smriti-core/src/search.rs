//! Search: a [`Query`] (what the UI's chips add up to) turns into one SQL
//! filter over indexed columns, and [`Library::suggest`] turns typed words
//! into chips ("Divik at 3", "Ooty", "Oct 2021", "videos", "only").

use chrono::{Datelike, NaiveDate};
use regex::Regex;
use rusqlite::types::Value;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use crate::people::ASSIGNED;
use crate::{Id, Kind, Library, Result};

/// Faces at least this clear count as "someone else" for `only`; tiny
/// faces in the background don't.
const OTHER_FACE_QUALITY: f32 = 0.45;
const MAX_SUGGESTIONS: usize = 8;

/// Every filter the search bar can express. All parts must match.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct Query {
    /// Photos with all of these people...
    pub people: Vec<Id>,
    /// ...and at least one of these...
    pub any_people: Vec<Id>,
    /// ...and none of these.
    pub without: Vec<Id>,
    /// Nobody else clearly in the photo.
    pub only: bool,
    /// Inclusive `YYYY-MM-DD` bounds.
    pub from: Option<String>,
    pub to: Option<String>,
    /// A child at an age, from their birth date.
    pub age: Option<AgeFilter>,
    /// `MM-DD`: this day in every year.
    pub on_this_day: Option<String>,
    pub place: Option<Id>,
    pub admin1: Option<String>,
    pub country: Option<String>,
    pub trip: Option<Id>,
    /// A folder path relative to its source, e.g. `ooty - nov 2021`.
    pub folder: Option<String>,
    pub kind: Option<Kind>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AgeFilter {
    pub person: Id,
    /// Age in whole months: from (inclusive) to (exclusive).
    pub from_months: u32,
    pub to_months: u32,
}

/// One search chip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Chip {
    Person { id: Id, name: String, face_id: Option<Id> },
    Place { id: Id, name: String },
    Region { admin1: String },
    Country { country: String },
    Trip { id: Id, title: String },
    Dates { from: String, to: String, label: String },
    Age { person: Id, from_months: u32, to_months: u32, label: String },
    OnThisDay,
    Folder { path: String },
    Kind { kind: Kind },
    Only,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub chip: Chip,
    pub label: String,
    /// Secondary text, e.g. "Tamil Nadu, India · 412 photos".
    pub detail: Option<String>,
}

impl Query {
    /// SQL conditions on `assets` (aliased as the table itself) plus parameters.
    pub(crate) fn to_sql(&self) -> (String, Vec<Value>) {
        let mut sql = String::new();
        let mut args: Vec<Value> = Vec::new();
        let ids = |v: &[Id]| {
            let mut v: Vec<i64> = v.iter().map(|p| p.0).collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        // Ids are integers, so formatting them into the SQL is safe.
        let list = |v: &[i64]| v.iter().map(i64::to_string).collect::<Vec<_>>().join(",");

        let mut all = ids(&self.people);
        if let Some(a) = &self.age
            && !all.contains(&a.person.0)
        {
            all.push(a.person.0);
        }
        if !all.is_empty() {
            sql += &format!(
                " AND assets.id IN (SELECT asset_id FROM faces WHERE person_id IN ({}) AND {ASSIGNED}
                                   GROUP BY asset_id HAVING count(DISTINCT person_id) = {})",
                list(&all),
                all.len()
            );
        }
        let any = ids(&self.any_people);
        if !any.is_empty() {
            sql += &format!(
                " AND EXISTS (SELECT 1 FROM faces f WHERE f.asset_id = assets.id AND f.person_id IN ({}) AND f.{ASSIGNED})",
                list(&any)
            );
        }
        let without = ids(&self.without);
        if !without.is_empty() {
            sql += &format!(
                " AND NOT EXISTS (SELECT 1 FROM faces f WHERE f.asset_id = assets.id AND f.person_id IN ({}) AND f.{ASSIGNED})",
                list(&without)
            );
        }
        if self.only {
            let mut wanted = all.clone();
            wanted.extend(&any);
            sql += &format!(
                " AND NOT EXISTS (SELECT 1 FROM faces o WHERE o.asset_id = assets.id
                     AND (o.assign_kind IS NULL OR o.assign_kind != 'rejected')
                     AND (o.person_id IS NULL OR o.person_id NOT IN ({}))
                     AND (o.person_id IS NOT NULL OR o.quality >= {OTHER_FACE_QUALITY}))",
                if wanted.is_empty() { "-1".into() } else { list(&wanted) }
            );
        }
        if let Some(from) = &self.from {
            sql += " AND taken_at >= ?";
            args.push(Value::Text(from.clone()));
        }
        if let Some(to) = &self.to {
            // Inclusive day: anything before the next day.
            sql += " AND taken_at < date(?, '+1 day')";
            args.push(Value::Text(to.clone()));
        }
        if let Some(a) = &self.age {
            sql += " AND taken_at >= (SELECT date(birth_date, '+' || ? || ' months') FROM persons WHERE id = ?)
                     AND taken_at < (SELECT date(birth_date, '+' || ? || ' months') FROM persons WHERE id = ?)";
            args.extend([
                Value::Integer(a.from_months.into()),
                Value::Integer(a.person.0),
                Value::Integer(a.to_months.into()),
                Value::Integer(a.person.0),
            ]);
        }
        if let Some(md) = &self.on_this_day {
            sql += " AND substr(taken_at, 6, 5) = ?";
            args.push(Value::Text(md.clone()));
        }
        if let Some(p) = self.place {
            sql += " AND place_id = ?";
            args.push(Value::Integer(p.0));
        }
        if let Some(r) = &self.admin1 {
            sql += " AND place_id IN (SELECT id FROM places WHERE admin1 = ?)";
            args.push(Value::Text(r.clone()));
        }
        if let Some(c) = &self.country {
            sql += " AND place_id IN (SELECT id FROM places WHERE country = ?)";
            args.push(Value::Text(c.clone()));
        }
        if let Some(t) = self.trip {
            sql += " AND trip_id = ?";
            args.push(Value::Integer(t.0));
        }
        if let Some(f) = &self.folder {
            sql += " AND (rel_path LIKE ? ESCAPE '\\')";
            let esc = f.trim_matches('/').replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            args.push(Value::Text(format!("{esc}/%")));
        }
        if let Some(k) = self.kind {
            sql += " AND kind = ?";
            args.push(Value::Text(k.as_str().into()));
        }
        (sql, args)
    }
}

static AGE_UNDER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<name>.+?)\s+(?:under|below|before)\s+(?P<n>\d{1,2})$").unwrap());
static AGE_AT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<name>.+?)\s+(?:at|aged?|turning)\s+(?P<n>\d{1,2})$").unwrap());
static AGE_BABY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<name>.+?)(?:'s)?\s+(?:first year|1st year|as a baby|baby|newborn|first birthday)$").unwrap()
});
static MONTH_YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<m>[a-z]{3,9})\s+(?P<y>(?:19|20)\d{2})$").unwrap());
static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:19|20)\d{2}$").unwrap());

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn month_end(y: i32, m: u32) -> NaiveDate {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    NaiveDate::from_ymd_opt(ny, nm, 1).expect("valid") - chrono::Duration::days(1)
}

fn age_label(name: &str, from: u32, to: u32) -> String {
    match (from, to) {
        (0, 12) => format!("{name}'s first year"),
        (0, n) if n % 12 == 0 => format!("{name} under {}", n / 12),
        (a, b) if a % 12 == 0 && b == a + 12 => format!("{name} at {}", a / 12),
        _ => format!("{name}, {from}–{to} months"),
    }
}

fn plural(n: u32, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

impl Library {
    /// Chips for what's typed so far, best first. `today` (`YYYY-MM-DD`) comes
    /// from the UI so "this year" and "on this day" follow the user's clock.
    pub fn suggest(&self, text: &str, today: &str) -> Result<Vec<Suggestion>> {
        let t = text.trim().to_lowercase();
        let today = NaiveDate::parse_from_str(today, "%Y-%m-%d").unwrap_or_else(|_| chrono::Local::now().date_naive());
        let people = self.people()?;
        let mut out: Vec<Suggestion> = Vec::new();
        let push = |out: &mut Vec<Suggestion>, chip: Chip, label: String, detail: Option<String>| {
            if out.len() < MAX_SUGGESTIONS && !out.iter().any(|s| s.chip == chip) {
                out.push(Suggestion { chip, label, detail });
            }
        };
        let find_person = |name: &str| {
            let name = name.trim().trim_end_matches("'s");
            people
                .iter()
                .find(|p| p.name.to_lowercase() == name)
                .or_else(|| people.iter().find(|p| p.name.to_lowercase().starts_with(name) && name.len() >= 2))
        };

        if t.is_empty() {
            for p in people.iter().take(5) {
                push(
                    &mut out,
                    Chip::Person { id: p.id, name: p.name.clone(), face_id: p.cover_face_id },
                    p.name.clone(),
                    Some(plural(p.photo_count, "photo")),
                );
            }
            push(&mut out, Chip::OnThisDay, "On this day".into(), Some("This date in past years".into()));
            for trip in self.trips()?.into_iter().take(2) {
                push(&mut out, Chip::Trip { id: trip.id, title: trip.title.clone() }, trip.title, Some(trip.start));
            }
            return Ok(out);
        }

        // Ages: "aarna under 1", "divik at 3", "aarna's first year".
        let age = [(&*AGE_UNDER, 0), (&*AGE_AT, 1), (&*AGE_BABY, 2)].into_iter().find_map(|(re, kind)| {
            let c = re.captures(&t)?;
            let p = find_person(c.name("name")?.as_str())?;
            let n: u32 = c.name("n").and_then(|n| n.as_str().parse().ok()).unwrap_or(0);
            let (from, to) = match kind {
                0 => (0, n.max(1) * 12),
                1 => (n * 12, n * 12 + 12),
                _ => (0, 12),
            };
            Some((p, from, to))
        });
        if let Some((p, from, to)) = age {
            let detail = if p.birth_date.is_some() { None } else { Some(format!("Add {}'s birth date first", p.name)) };
            let label = age_label(&p.name, from, to);
            push(
                &mut out,
                Chip::Age { person: p.id, from_months: from, to_months: to, label: label.clone() },
                label,
                detail,
            );
        }

        // Dates.
        if YEAR.is_match(&t) {
            let y: i32 = t.parse().unwrap_or(2000);
            push(
                &mut out,
                Chip::Dates { from: format!("{y}-01-01"), to: format!("{y}-12-31"), label: t.clone() },
                t.clone(),
                Some("Year".into()),
            );
        }
        if let Some(c) = MONTH_YEAR.captures(&t)
            && let Some(m) = MONTHS.iter().position(|m| c["m"].starts_with(m))
        {
            let y: i32 = c["y"].parse().unwrap_or(2000);
            let label = format!("{} {y}", MONTH_NAMES[m]);
            let end = month_end(y, m as u32 + 1);
            push(
                &mut out,
                Chip::Dates { from: format!("{y}-{:02}-01", m + 1), to: end.to_string(), label: label.clone() },
                label,
                Some("Month".into()),
            );
        }
        let rel: &[(&str, NaiveDate, NaiveDate, &str)] = &[
            ("this year", NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap(), today, "This year"),
            (
                "last year",
                NaiveDate::from_ymd_opt(today.year() - 1, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(today.year() - 1, 12, 31).unwrap(),
                "Last year",
            ),
            ("last month", today - chrono::Duration::days(31), today, "Last month"),
        ];
        for (words, from, to, label) in rel {
            if words.starts_with(&t) && t.len() >= 4 {
                push(
                    &mut out,
                    Chip::Dates { from: from.to_string(), to: to.to_string(), label: label.to_string() },
                    label.to_string(),
                    None,
                );
            }
        }
        if ["on this day", "today", "this day"].iter().any(|w| w.starts_with(&t) && t.len() >= 3) {
            push(
                &mut out,
                Chip::OnThisDay,
                "On this day".into(),
                Some(format!("{} in past years", today.format("%-d %B"))),
            );
        }

        // Types and "only".
        if "videos".starts_with(&t) && t.len() >= 3 {
            push(&mut out, Chip::Kind { kind: Kind::Video }, "Videos".into(), None);
        }
        if "photos".starts_with(&t) && t.len() >= 3 {
            push(&mut out, Chip::Kind { kind: Kind::Photo }, "Photos".into(), None);
        }
        if ["only", "just", "no one else"].iter().any(|w| w.starts_with(&t) && t.len() >= 3) {
            push(&mut out, Chip::Only, "No one else".into(), Some("Only the chosen people in the photo".into()));
        }

        // People, places, trips, folders by name.
        let word_match = |hay: &str| {
            let hay = hay.to_lowercase();
            hay.starts_with(&t) || hay.split(|c: char| !c.is_alphanumeric()).any(|w| !w.is_empty() && w.starts_with(&t))
        };
        for p in people.iter().filter(|p| word_match(&p.name)) {
            push(
                &mut out,
                Chip::Person { id: p.id, name: p.name.clone(), face_id: p.cover_face_id },
                p.name.clone(),
                Some(plural(p.photo_count, "photo")),
            );
        }
        let places = self.places()?;
        for p in places.iter().filter(|p| word_match(&p.name)).take(3) {
            let region =
                [p.admin1.as_deref(), p.country.as_deref()].into_iter().flatten().collect::<Vec<_>>().join(", ");
            push(
                &mut out,
                Chip::Place { id: p.id, name: p.name.clone() },
                p.name.clone(),
                Some(format!("{region} · {}", plural(p.count, "photo"))),
            );
        }
        let mut regions: Vec<(String, u32)> = Vec::new();
        for p in places.iter().filter(|p| p.admin1.as_deref().is_some_and(word_match)) {
            let r = p.admin1.clone().unwrap_or_default();
            match regions.iter_mut().find(|(n, _)| *n == r) {
                Some(e) => e.1 += p.count,
                None => regions.push((r, p.count)),
            }
        }
        for (r, n) in regions.into_iter().take(2) {
            push(&mut out, Chip::Region { admin1: r.clone() }, r, Some(format!("State · {}", plural(n, "photo"))));
        }
        let mut countries: Vec<(String, u32)> = Vec::new();
        for p in places.iter().filter(|p| p.country.as_deref().is_some_and(word_match)) {
            let c = p.country.clone().unwrap_or_default();
            match countries.iter_mut().find(|(n, _)| *n == c) {
                Some(e) => e.1 += p.count,
                None => countries.push((c, p.count)),
            }
        }
        for (c, n) in countries.into_iter().take(1) {
            push(&mut out, Chip::Country { country: c.clone() }, c, Some(format!("Country · {}", plural(n, "photo"))));
        }
        for trip in self
            .trips()?
            .into_iter()
            .filter(|tr| word_match(&tr.title) || (t.len() >= 4 && "trips".starts_with(&t)))
            .take(3)
        {
            push(
                &mut out,
                Chip::Trip { id: trip.id, title: trip.title.clone() },
                format!("{} trip", trip.title),
                Some(trip.start),
            );
        }
        for (folder, n) in self.top_folders()?.into_iter().filter(|(f, _)| word_match(f)).take(3) {
            push(
                &mut out,
                Chip::Folder { path: folder.clone() },
                folder,
                Some(format!("Folder · {}", plural(n, "item"))),
            );
        }
        Ok(out)
    }

    /// First-level folders under each source, with item counts.
    fn top_folders(&self) -> Result<Vec<(String, u32)>> {
        let conn = self.read();
        let mut stmt = conn.prepare_cached(
            "SELECT substr(rel_path, 1, instr(rel_path, '/') - 1) AS top, count(*) FROM assets
             WHERE status = 'active' AND instr(rel_path, '/') > 0 GROUP BY top ORDER BY count(*) DESC",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PersonRef, PersonUpdate};
    use rusqlite::params;

    fn lib_with(items: &[(&str, &str, &str)]) -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(dir.path().join("lib")).unwrap();
        let photos = dir.path().join("photos");
        std::fs::create_dir_all(&photos).unwrap();
        let src = lib.add_source(&photos).unwrap();
        {
            let conn = lib.conn();
            for (rel, date, kind) in items {
                conn.execute(
                    "INSERT INTO assets (source_id, rel_path, size, mtime, kind, taken_at) VALUES (?1, ?2, 1, 0, ?3, ?4)",
                    params![src.id, rel, kind, date],
                )
                .unwrap();
            }
        }
        (dir, lib)
    }

    fn ids(lib: &Library, q: &Query) -> Vec<String> {
        let tiles = lib.timeline(Some(q)).unwrap();
        let conn = lib.read();
        tiles
            .iter()
            .map(|t| conn.query_row("SELECT rel_path FROM assets WHERE id = ?1", [t.id], |r| r.get(0)).unwrap())
            .collect()
    }

    #[test]
    fn filters_by_date_folder_kind_and_day() {
        let (_d, lib) = lib_with(&[
            ("ooty - nov 2021/a.jpg", "2021-11-20T10:00:00", "photo"),
            ("ooty - nov 2021/b.mp4", "2021-11-21T23:59:00", "video"),
            ("2022/c.jpg", "2022-11-21T09:00:00", "photo"),
            ("ooty_x/d.jpg", "2021-11-20T10:00:00", "photo"),
        ]);
        let q = |f: fn(&mut Query)| {
            let mut q = Query::default();
            f(&mut q);
            q
        };
        assert_eq!(ids(&lib, &q(|q| q.from = Some("2021-11-21".into()))).len(), 2);
        assert_eq!(ids(&lib, &q(|q| q.to = Some("2021-11-21".into()))).len(), 3); // inclusive day
        assert_eq!(ids(&lib, &q(|q| q.folder = Some("ooty - nov 2021".into()))).len(), 2);
        assert_eq!(ids(&lib, &q(|q| q.kind = Some(Kind::Video))), vec!["ooty - nov 2021/b.mp4"]);
        assert_eq!(ids(&lib, &q(|q| q.on_this_day = Some("11-21".into()))).len(), 2);
    }

    #[test]
    fn suggests_chips_from_words() {
        let (_d, lib) = lib_with(&[("ooty - nov 2021/a.jpg", "2021-11-20T10:00:00", "photo")]);
        // A named child with a birth date.
        let face_asset: i64 = lib.read().query_row("SELECT id FROM assets", [], |r| r.get(0)).unwrap();
        lib.conn()
            .execute(
                "INSERT INTO faces (asset_id, bbox, det_score, quality) VALUES (?1, '[0,0,1,1]', 0.9, 0.9)",
                [face_asset],
            )
            .unwrap();
        let face: Id = lib.read().query_row("SELECT id FROM faces", [], |r| r.get(0)).unwrap();
        let aarna = lib.assign_faces(&[face], &PersonRef { person_id: None, name: Some("Aarna".into()) }).unwrap();
        lib.update_person(
            aarna,
            &PersonUpdate { name: "Aarna".into(), birth_date: Some("2021-11-01".into()), is_family: true },
        )
        .unwrap();

        let chips = |t: &str| lib.suggest(t, "2026-10-20").unwrap().into_iter().map(|s| s.chip).collect::<Vec<_>>();
        assert!(matches!(chips("aar")[0], Chip::Person { .. }));
        assert_eq!(
            chips("aarna under 1")[0],
            Chip::Age { person: aarna, from_months: 0, to_months: 12, label: "Aarna's first year".into() }
        );
        assert!(matches!(chips("aarna at 3")[0], Chip::Age { from_months: 36, to_months: 48, .. }));
        assert_eq!(
            chips("oct 2021")[0],
            Chip::Dates { from: "2021-10-01".into(), to: "2021-10-31".into(), label: "October 2021".into() }
        );
        assert!(matches!(chips("2021")[0], Chip::Dates { .. }));
        assert_eq!(chips("vid")[0], Chip::Kind { kind: Kind::Video });
        assert_eq!(chips("only")[0], Chip::Only);
        assert_eq!(chips("ooty")[0], Chip::Folder { path: "ooty - nov 2021".into() });

        // The age chip finds photos from her first year only.
        let q = Query { age: Some(AgeFilter { person: aarna, from_months: 0, to_months: 12 }), ..Default::default() };
        assert_eq!(lib.timeline(Some(&q)).unwrap().len(), 1);
        let q = Query { age: Some(AgeFilter { person: aarna, from_months: 12, to_months: 24 }), ..Default::default() };
        assert_eq!(lib.timeline(Some(&q)).unwrap().len(), 0);
    }
}
