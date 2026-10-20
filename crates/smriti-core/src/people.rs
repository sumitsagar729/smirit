//! People: naming faces, age-aware recognition, and grouping unnamed faces.
//!
//! Faces the user names become *confirmed* references. Recognition compares
//! every other face with each person's references and assigns it (*auto*)
//! when one person clearly wins. Children change fast, so a child's face is
//! compared mainly with references from a similar age, and nobody is matched
//! in photos taken before they were born. Faces nobody claims are grouped by
//! similarity so the user can name a whole group at once.

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use rayon::prelude::*;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::faces::{self, EMBEDDING_DIM};
use crate::{Error, Id, Library, Result};

/// Mean of the best three similarities to someone's references needed to
/// recognise them (measured on this family's photos: about 1 error in 700).
const MATCH_MIN: f32 = 0.42;
/// The best person must beat the runner-up by this much.
const MATCH_MARGIN: f32 = 0.04;
/// Small, blurry or turned faces need a stronger match.
const WEAK_QUALITY: f32 = 0.35;
const WEAK_MATCH_MIN: f32 = 0.5;
/// Quality needed to start or join a group of unnamed faces.
const GROUP_QUALITY: f32 = 0.5;
/// Similarity to a group's average face needed to join it.
const GROUP_JOIN: f32 = 0.5;
/// Lower-quality faces may join an existing group with a stronger match.
const GROUP_ATTACH_QUALITY: f32 = 0.25;
const GROUP_ATTACH: f32 = 0.55;
/// Groups smaller than this aren't worth showing.
pub const MIN_GROUP_SIZE: u32 = 3;
/// Reference faces used per person and age band (best quality first).
const REFS_PER_BAND: usize = 300;
/// Similarity penalty per age band between a reference and the face matched.
const BAND_PENALTY: f32 = 0.03;
/// References more than this many bands away are ignored (a newborn looks
/// nothing like the same child at eight).
const MAX_BAND_GAP: u8 = 2;
/// Someone can't appear in a photo from well before their birth.
const BEFORE_BIRTH_SLACK_DAYS: i64 = 30;

/// Age bands by months of age: newborn year, toddler, preschool, school...
const BAND_LIMITS_MONTHS: [i64; 6] = [12, 24, 48, 84, 144, 216];

fn age_band(birth: NaiveDate, on: NaiveDate) -> u8 {
    let months = (on - birth).num_days() * 12 / 365;
    BAND_LIMITS_MONTHS.iter().position(|&m| months < m).unwrap_or(BAND_LIMITS_MONTHS.len()) as u8
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d").ok()
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub id: Id,
    pub name: String,
    /// `YYYY-MM-DD`; lets recognition follow a child as they grow.
    pub birth_date: Option<String>,
    pub is_family: bool,
    pub cover_face_id: Option<Id>,
    pub face_count: u32,
    pub photo_count: u32,
}

/// Unnamed faces that look alike.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FaceGroup {
    pub id: Id,
    pub size: u32,
    /// The clearest faces first.
    pub face_ids: Vec<Id>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FaceRef {
    pub id: Id,
    pub asset_id: Id,
    /// "confirmed" (named by you) or "auto" (recognised).
    pub kind: Option<String>,
    /// Recognition confidence for "auto" faces.
    pub score: Option<f32>,
    pub taken_at: Option<String>,
}

/// A face as drawn over a photo in the viewer.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AssetFace {
    pub id: Id,
    /// x, y, width, height as fractions of the upright photo.
    pub bbox: Vec<f32>,
    pub person_id: Option<Id>,
    pub person_name: Option<String>,
    pub confirmed: bool,
}

/// Who to give faces to: an existing person, or a new one by name.
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PersonRef {
    pub person_id: Option<Id>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PersonUpdate {
    pub name: String,
    pub birth_date: Option<String>,
    pub is_family: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecognizeReport {
    pub faces: u32,
    pub recognized: u32,
    pub groups: u32,
    pub ms: u32,
}

/// Faces matched by people filters: recognised or confirmed.
pub(crate) const ASSIGNED: &str = "assign_kind IN ('auto', 'confirmed')";

impl Library {
    pub fn people(&self) -> Result<Vec<Person>> {
        let conn = self.read();
        let mut stmt = conn.prepare_cached(
            "SELECT p.id, p.name, p.birth_date, p.is_family,
                    coalesce(p.cover_face_id,
                             (SELECT f.id FROM faces f WHERE f.person_id = p.id
                              ORDER BY f.assign_kind = 'confirmed' DESC, f.quality DESC LIMIT 1)),
                    count(f.id), count(DISTINCT f.asset_id)
             FROM persons p
             LEFT JOIN faces f ON f.person_id = p.id AND f.assign_kind IN ('auto', 'confirmed')
                  AND f.asset_id IN (SELECT id FROM assets WHERE status = 'active')
             WHERE p.hidden = 0 AND p.name IS NOT NULL
             GROUP BY p.id
             ORDER BY p.is_family DESC, count(DISTINCT f.asset_id) DESC, p.name",
        )?;
        let people = stmt
            .query_map([], |r| {
                Ok(Person {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    birth_date: r.get(2)?,
                    is_family: r.get(3)?,
                    cover_face_id: r.get(4)?,
                    face_count: r.get(5)?,
                    photo_count: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(people)
    }

    pub fn person(&self, id: Id) -> Result<Person> {
        self.people()?.into_iter().find(|p| p.id == id).ok_or_else(|| Error::NotFound(format!("person {id}")))
    }

    /// Groups of unnamed look-alike faces, largest first.
    pub fn face_groups(&self, limit: u32) -> Result<Vec<FaceGroup>> {
        let conn = self.read();
        let mut stmt = conn.prepare_cached(
            "SELECT f.cluster_id, count(*) FROM faces f JOIN assets a ON a.id = f.asset_id
             WHERE f.cluster_id IS NOT NULL AND f.person_id IS NULL AND f.assign_kind IS NULL AND a.status = 'active'
             GROUP BY f.cluster_id HAVING count(*) >= ?1
             ORDER BY count(*) DESC, f.cluster_id LIMIT ?2",
        )?;
        let groups: Vec<(Id, u32)> = stmt
            .query_map(params![MIN_GROUP_SIZE, limit], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut sample = conn.prepare_cached(
            "SELECT id FROM faces WHERE cluster_id = ?1 AND person_id IS NULL AND assign_kind IS NULL
             ORDER BY quality DESC LIMIT 12",
        )?;
        groups
            .into_iter()
            .map(|(id, size)| {
                let face_ids = sample.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<Vec<Id>>>()?;
                Ok(FaceGroup { id, size, face_ids })
            })
            .collect()
    }

    /// Every face in a group, clearest first.
    pub fn group_faces(&self, group: Id) -> Result<Vec<FaceRef>> {
        self.face_refs(
            "WHERE f.cluster_id = ?1 AND f.person_id IS NULL AND f.assign_kind IS NULL ORDER BY f.quality DESC",
            group,
        )
    }

    /// A person's faces. With `suggestions`, only recognised (unconfirmed)
    /// ones, least certain first, for review.
    pub fn person_faces(&self, person: Id, suggestions: bool) -> Result<Vec<FaceRef>> {
        if suggestions {
            self.face_refs("WHERE f.person_id = ?1 AND f.assign_kind = 'auto' ORDER BY f.assign_score", person)
        } else {
            self.face_refs(
                "WHERE f.person_id = ?1 AND f.assign_kind IN ('auto', 'confirmed') ORDER BY a.taken_at DESC",
                person,
            )
        }
    }

    fn face_refs(&self, filter: &str, id: Id) -> Result<Vec<FaceRef>> {
        let conn = self.read();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT f.id, f.asset_id, f.assign_kind, f.assign_score, a.taken_at
             FROM faces f JOIN assets a ON a.id = f.asset_id AND a.status = 'active' {filter}"
        ))?;
        let faces = stmt
            .query_map([id], |r| {
                Ok(FaceRef {
                    id: r.get(0)?,
                    asset_id: r.get(1)?,
                    kind: r.get(2)?,
                    score: r.get::<_, Option<f64>>(3)?.map(|s| s as f32),
                    taken_at: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(faces)
    }

    pub fn asset_faces(&self, asset: Id) -> Result<Vec<AssetFace>> {
        let conn = self.read();
        let mut stmt = conn.prepare_cached(
            "SELECT f.id, f.bbox, f.person_id, p.name, f.assign_kind = 'confirmed'
             FROM faces f LEFT JOIN persons p ON p.id = f.person_id
             WHERE f.asset_id = ?1 AND (f.assign_kind IS NULL OR f.assign_kind != 'rejected')
             ORDER BY json_extract(f.bbox, '$[0]')",
        )?;
        let faces = stmt
            .query_map([asset], |r| {
                let bbox: String = r.get(1)?;
                Ok(AssetFace {
                    id: r.get(0)?,
                    bbox: serde_json::from_str(&bbox).unwrap_or_default(),
                    person_id: r.get(2)?,
                    person_name: r.get(3)?,
                    confirmed: r.get::<_, Option<bool>>(4)?.unwrap_or(false),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(faces)
    }

    /// Gives faces to a person (creating them if named anew) as confirmed references.
    pub fn assign_faces(&self, face_ids: &[Id], to: &PersonRef) -> Result<Id> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let person = resolve_person(&tx, to)?;
        {
            let mut set = tx.prepare_cached(
                "UPDATE faces SET person_id = ?2, assign_kind = 'confirmed', assign_score = NULL, cluster_id = NULL
                 WHERE id = ?1",
            )?;
            let mut unreject =
                tx.prepare_cached("DELETE FROM face_rejections WHERE face_id = ?1 AND person_id = ?2")?;
            for &f in face_ids {
                set.execute(params![f, person])?;
                unreject.execute(params![f, person])?;
            }
        }
        tx.commit()?;
        Ok(person)
    }

    /// Names every unnamed face in a group.
    pub fn name_group(&self, group: Id, to: &PersonRef) -> Result<Id> {
        let ids: Vec<Id> = self.group_faces(group)?.into_iter().map(|f| f.id).collect();
        if ids.is_empty() {
            return Err(Error::NotFound(format!("group {group}")));
        }
        self.assign_faces(&ids, to)
    }

    /// "Not this person": unassigns the faces and remembers not to suggest them again.
    pub fn reject_faces(&self, face_ids: &[Id]) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        {
            let mut remember = tx.prepare_cached(
                "INSERT OR IGNORE INTO face_rejections (face_id, person_id)
                 SELECT id, person_id FROM faces WHERE id = ?1 AND person_id IS NOT NULL",
            )?;
            let mut clear = tx.prepare_cached(
                "UPDATE faces SET person_id = NULL, assign_kind = NULL, assign_score = NULL WHERE id = ?1",
            )?;
            for &f in face_ids {
                remember.execute([f])?;
                clear.execute([f])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Strangers, posters, statues: never group or recognise these faces.
    pub fn ignore_faces(&self, face_ids: &[Id]) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        {
            let mut ignore = tx.prepare_cached(
                "UPDATE faces SET person_id = NULL, assign_kind = 'rejected', assign_score = NULL, cluster_id = NULL
                 WHERE id = ?1",
            )?;
            for &f in face_ids {
                ignore.execute([f])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn ignore_group(&self, group: Id) -> Result<()> {
        let ids: Vec<Id> = self.group_faces(group)?.into_iter().map(|f| f.id).collect();
        self.ignore_faces(&ids)
    }

    pub fn update_person(&self, id: Id, u: &PersonUpdate) -> Result<()> {
        let name = clean_name(&u.name)?;
        let birth = match u.birth_date.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(s) => Some(
                parse_date(s)
                    .filter(|_| s.len() == 10)
                    .ok_or_else(|| Error::Invalid("Birth date must look like 2020-10-03".into()))?
                    .to_string(),
            ),
            None => None,
        };
        let n = self.conn().execute(
            "UPDATE persons SET name = ?2, birth_date = ?3, is_family = ?4 WHERE id = ?1",
            params![id, name, birth, u.is_family],
        )?;
        if n == 0 {
            return Err(Error::NotFound(format!("person {id}")));
        }
        Ok(())
    }

    /// Moves everything from `from` to `into` and deletes `from`.
    pub fn merge_people(&self, from: Id, into: Id) -> Result<()> {
        if from == into {
            return Ok(());
        }
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute("UPDATE faces SET person_id = ?2 WHERE person_id = ?1", params![from, into])?;
        tx.execute(
            "INSERT OR IGNORE INTO face_rejections (face_id, person_id) SELECT face_id, ?2 FROM face_rejections WHERE person_id = ?1",
            params![from, into],
        )?;
        tx.execute(
            "UPDATE persons SET is_family = max(is_family, (SELECT is_family FROM persons WHERE id = ?1)),
                 birth_date = coalesce(birth_date, (SELECT birth_date FROM persons WHERE id = ?1))
             WHERE id = ?2",
            params![from, into],
        )?;
        tx.execute("DELETE FROM persons WHERE id = ?1", [from])?;
        tx.commit()?;
        Ok(())
    }

    /// Forgets a person; their faces become unnamed again.
    pub fn delete_person(&self, id: Id) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE faces SET person_id = NULL, assign_kind = NULL, assign_score = NULL WHERE person_id = ?1",
            [id],
        )?;
        tx.execute("DELETE FROM persons WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_cover_face(&self, person: Id, face: Id) -> Result<()> {
        self.conn().execute("UPDATE persons SET cover_face_id = ?2 WHERE id = ?1", params![person, face])?;
        Ok(())
    }

    /// Re-runs recognition and grouping over every face. Confirmed and ignored
    /// faces are left alone. Takes a few seconds for tens of thousands of faces.
    pub fn recognize(&self) -> Result<RecognizeReport> {
        let started = std::time::Instant::now();
        let data = self.load_recognition_data()?;
        let n = data.faces.len();

        // 1. Recognise against named people.
        let decisions: Vec<Option<(Id, f32)>> = (0..n).into_par_iter().map(|i| data.best_person(i)).collect();

        // 2. Group whatever is left.
        let unclaimed: Vec<usize> =
            (0..n).filter(|&i| data.faces[i].kind.is_none() && decisions[i].is_none()).collect();
        let clusters = group(&data, &unclaimed);

        // 3. Write back only what changed.
        let mut recognized = 0;
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        {
            let mut set_auto = tx.prepare_cached(
                "UPDATE faces SET person_id = ?2, assign_kind = 'auto', assign_score = ?3, cluster_id = NULL
                 WHERE id = ?1 AND (assign_kind IS NULL OR assign_kind = 'auto')",
            )?;
            let mut set_group = tx.prepare_cached(
                "UPDATE faces SET person_id = NULL, assign_kind = NULL, assign_score = NULL, cluster_id = ?2
                 WHERE id = ?1 AND (assign_kind IS NULL OR assign_kind = 'auto')",
            )?;
            for (i, f) in data.faces.iter().enumerate() {
                if matches!(f.kind, Some(Kind::Confirmed | Kind::Ignored)) {
                    continue;
                }
                match decisions[i] {
                    Some((person, score)) => {
                        recognized += 1;
                        let unchanged = f.kind == Some(Kind::Auto)
                            && f.person == Some(person)
                            && f.score.is_some_and(|s| (s - score).abs() < 1e-3);
                        if !unchanged {
                            set_auto.execute(params![f.id, person, score])?;
                        }
                    }
                    None => {
                        let cluster = clusters.get(&i).copied();
                        if f.kind.is_some() || f.cluster != cluster {
                            set_group.execute(params![f.id, cluster])?;
                        }
                    }
                }
            }
        }
        tx.commit()?;

        let groups = clusters.values().collect::<HashSet<_>>().len() as u32;
        let report = RecognizeReport { faces: n as u32, recognized, groups, ms: started.elapsed().as_millis() as u32 };
        tracing::info!(?report, "recognition finished");
        Ok(report)
    }

    fn load_recognition_data(&self) -> Result<RecognitionData> {
        let conn = self.read();
        let mut faces = Vec::new();
        let mut embeddings = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT f.id, f.embedding, coalesce(f.quality, 0), a.taken_at, f.person_id, f.assign_kind,
                        f.assign_score, f.cluster_id
                 FROM faces f JOIN assets a ON a.id = f.asset_id
                 WHERE a.status = 'active' AND f.embedding IS NOT NULL
                 ORDER BY f.id",
            )?;
            let mut rows = stmt.query([])?;
            while let Some(r) = rows.next()? {
                let blob: Vec<u8> = r.get(1)?;
                if blob.len() != EMBEDDING_DIM * 4 {
                    continue;
                }
                embeddings.extend(faces::from_blob(&blob));
                let kind: Option<String> = r.get(5)?;
                let person: Option<Id> = r.get(4)?;
                faces.push(FaceRow {
                    id: r.get(0)?,
                    quality: r.get::<_, f64>(2)? as f32,
                    date: r.get::<_, Option<String>>(3)?.as_deref().and_then(parse_date),
                    person,
                    kind: match kind.as_deref() {
                        Some("confirmed") if person.is_some() => Some(Kind::Confirmed),
                        Some("auto") if person.is_some() => Some(Kind::Auto),
                        Some("rejected") => Some(Kind::Ignored),
                        _ => None,
                    },
                    score: r.get::<_, Option<f64>>(6)?.map(|s| s as f32),
                    cluster: r.get(7)?,
                });
            }
        }

        let mut people: Vec<PersonRefs> = conn
            .prepare("SELECT id, birth_date FROM persons")?
            .query_map([], |r| {
                Ok(PersonRefs {
                    id: r.get(0)?,
                    birth: r.get::<_, Option<String>>(1)?.as_deref().and_then(parse_date),
                    refs: Vec::new(),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        // Confirmed faces become references, best quality first, capped per age band.
        let index: HashMap<Id, usize> = people.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
        let mut by_band: HashMap<(usize, Option<u8>), Vec<usize>> = HashMap::new();
        for (fi, f) in faces.iter().enumerate() {
            if f.kind != Some(Kind::Confirmed) {
                continue;
            }
            let Some(&pi) = f.person.as_ref().and_then(|p| index.get(p)) else { continue };
            let band = people[pi].birth.zip(f.date).map(|(b, d)| age_band(b, d));
            by_band.entry((pi, band)).or_default().push(fi);
        }
        for ((pi, band), mut members) in by_band {
            members.sort_by(|&a, &b| faces[b].quality.total_cmp(&faces[a].quality));
            members.truncate(REFS_PER_BAND);
            people[pi].refs.extend(members.into_iter().map(|fi| (fi, band)));
        }
        people.retain(|p| !p.refs.is_empty());

        let rejections: HashSet<(Id, Id)> = conn
            .prepare("SELECT face_id, person_id FROM face_rejections")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;

        Ok(RecognitionData { faces, embeddings, people, rejections })
    }
}

fn clean_name(name: &str) -> Result<String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(Error::Invalid("Please enter a name".into()));
    }
    if name.chars().count() > 80 {
        return Err(Error::Invalid("That name is too long".into()));
    }
    Ok(name)
}

/// An existing person, the person already using that name, or a new one.
fn resolve_person(tx: &rusqlite::Transaction<'_>, to: &PersonRef) -> Result<Id> {
    if let Some(id) = to.person_id {
        let exists: Option<Id> = tx.query_row("SELECT id FROM persons WHERE id = ?1", [id], |r| r.get(0)).optional()?;
        return exists.ok_or_else(|| Error::NotFound(format!("person {id}")));
    }
    let name = clean_name(to.name.as_deref().unwrap_or_default())?;
    let existing: Option<Id> = tx
        .query_row("SELECT id FROM persons WHERE name = ?1 COLLATE NOCASE ORDER BY id LIMIT 1", [&name], |r| r.get(0))
        .optional()?;
    if let Some(id) = existing {
        tx.execute("UPDATE persons SET hidden = 0 WHERE id = ?1", [id])?;
        return Ok(id);
    }
    tx.execute("INSERT INTO persons (name) VALUES (?1)", [&name])?;
    Ok(Id(tx.last_insert_rowid()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Auto,
    Confirmed,
    Ignored,
}

struct FaceRow {
    id: Id,
    quality: f32,
    date: Option<NaiveDate>,
    person: Option<Id>,
    kind: Option<Kind>,
    score: Option<f32>,
    cluster: Option<Id>,
}

struct PersonRefs {
    id: Id,
    birth: Option<NaiveDate>,
    /// (face index, age band when the photo was taken)
    refs: Vec<(usize, Option<u8>)>,
}

struct RecognitionData {
    faces: Vec<FaceRow>,
    /// Row-major, EMBEDDING_DIM per face.
    embeddings: Vec<f32>,
    people: Vec<PersonRefs>,
    rejections: HashSet<(Id, Id)>,
}

impl RecognitionData {
    fn emb(&self, i: usize) -> &[f32] {
        &self.embeddings[i * EMBEDDING_DIM..(i + 1) * EMBEDDING_DIM]
    }

    /// The person this face clearly belongs to, if any, with the match score.
    fn best_person(&self, i: usize) -> Option<(Id, f32)> {
        let face = &self.faces[i];
        if matches!(face.kind, Some(Kind::Confirmed | Kind::Ignored)) {
            return None;
        }
        let e = self.emb(i);
        let (mut best, mut second) = ((Id(0), f32::MIN), f32::MIN);
        for p in &self.people {
            if self.rejections.contains(&(face.id, p.id)) {
                continue;
            }
            let band = match (p.birth, face.date) {
                (Some(b), Some(d)) if (d - b).num_days() < -BEFORE_BIRTH_SLACK_DAYS => continue,
                (Some(b), Some(d)) => Some(age_band(b, d)),
                _ => None,
            };
            let mut top = [f32::MIN; 3];
            for &(r, rb) in &p.refs {
                let penalty = match (band, rb) {
                    (Some(a), Some(b)) if a.abs_diff(b) > MAX_BAND_GAP => continue,
                    (Some(a), Some(b)) => BAND_PENALTY * a.abs_diff(b) as f32,
                    _ => 0.0,
                };
                let s = faces::similarity(e, self.emb(r)) - penalty;
                if s > top[2] {
                    top[2] = s;
                    top.sort_by(|a, b| b.total_cmp(a));
                }
            }
            let used: Vec<f32> = top.into_iter().filter(|&s| s > f32::MIN).collect();
            if used.is_empty() {
                continue;
            }
            let score = used.iter().sum::<f32>() / used.len() as f32;
            if score > best.1 {
                second = best.1;
                best = (p.id, score);
            } else if score > second {
                second = score;
            }
        }
        let min = if face.quality < WEAK_QUALITY { WEAK_MATCH_MIN } else { MATCH_MIN };
        (best.1 >= min && best.1 - second >= MATCH_MARGIN).then_some(best)
    }
}

/// Greedy grouping: clearest faces first, each joins the most similar group
/// (by its average face) or starts a new one. Returns face index -> group id
/// (the id of the group's first face, so ids stay stable between runs).
fn group(data: &RecognitionData, candidates: &[usize]) -> HashMap<usize, Id> {
    let mut strong: Vec<usize> =
        candidates.iter().copied().filter(|&i| data.faces[i].quality >= GROUP_QUALITY).collect();
    strong.sort_by(|&a, &b| data.faces[b].quality.total_cmp(&data.faces[a].quality).then(a.cmp(&b)));

    let mut sums: Vec<Vec<f32>> = Vec::new(); // running sum per group
    let mut means: Vec<f32> = Vec::new(); // unit-length average, row-major
    let mut leaders: Vec<Id> = Vec::new();
    let mut out = HashMap::new();
    let nearest = |means: &[f32], e: &[f32], from: usize| -> (usize, f32) {
        let mut best = (usize::MAX, f32::MIN);
        for g in from..means.len() / EMBEDDING_DIM {
            let s = faces::similarity(e, &means[g * EMBEDDING_DIM..(g + 1) * EMBEDDING_DIM]);
            if s > best.1 {
                best = (g, s);
            }
        }
        best
    };

    // Batches: compare with existing groups in parallel, then settle in order.
    for batch in strong.chunks(512) {
        let existing = means.len() / EMBEDDING_DIM;
        let first_pass: Vec<(usize, f32)> = batch.par_iter().map(|&i| nearest(&means, data.emb(i), 0)).collect();
        for (&i, &(mut g, mut s)) in batch.iter().zip(&first_pass) {
            let e = data.emb(i);
            let (g2, s2) = nearest(&means, e, existing);
            if s2 > s {
                (g, s) = (g2, s2);
            }
            if s >= GROUP_JOIN {
                for (acc, x) in sums[g].iter_mut().zip(e) {
                    *acc += x;
                }
                let norm = sums[g].iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
                for (m, acc) in means[g * EMBEDDING_DIM..(g + 1) * EMBEDDING_DIM].iter_mut().zip(&sums[g]) {
                    *m = acc / norm;
                }
            } else {
                g = leaders.len();
                sums.push(e.to_vec());
                means.extend_from_slice(e);
                leaders.push(data.faces[i].id);
            }
            out.insert(i, leaders[g]);
        }
    }

    // Weaker faces may join a group but never start one.
    let weak: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|&i| (GROUP_ATTACH_QUALITY..GROUP_QUALITY).contains(&data.faces[i].quality))
        .collect();
    let joins: Vec<(usize, (usize, f32))> = weak.par_iter().map(|&i| (i, nearest(&means, data.emb(i), 0))).collect();
    for (i, (g, s)) in joins {
        if s >= GROUP_ATTACH {
            out.insert(i, leaders[g]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_bands_follow_a_growing_child() {
        let birth = NaiveDate::from_ymd_opt(2020, 10, 3).unwrap();
        let on = |y, m, d| age_band(birth, NaiveDate::from_ymd_opt(y, m, d).unwrap());
        assert_eq!(on(2020, 11, 1), 0); // newborn
        assert_eq!(on(2022, 1, 1), 1); // 15 months
        assert_eq!(on(2023, 6, 1), 2); // 2.7 years
        assert_eq!(on(2026, 10, 20), 3); // 6 years
        assert_eq!(on(2040, 1, 1), 6); // grown up
    }

    /// A library with faces whose embeddings are set directly.
    fn library_with_faces(embeddings: &[(Vec<f32>, &str, f32)]) -> (tempfile::TempDir, Library, Vec<Id>) {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(dir.path().join("lib")).unwrap();
        let photos = dir.path().join("photos");
        std::fs::create_dir_all(&photos).unwrap();
        let src = lib.add_source(&photos).unwrap();
        let mut ids = Vec::new();
        {
            let conn = lib.conn();
            for (i, (e, date, quality)) in embeddings.iter().enumerate() {
                conn.execute(
                    "INSERT INTO assets (source_id, rel_path, size, mtime, kind, taken_at) VALUES (?1, ?2, 1, 0, 'photo', ?3)",
                    params![src.id, format!("{i}.jpg"), date],
                )
                .unwrap();
                let asset = conn.last_insert_rowid();
                conn.execute(
                    "INSERT INTO faces (asset_id, bbox, det_score, quality, embedding) VALUES (?1, '[0.1,0.1,0.2,0.2]', 0.9, ?2, ?3)",
                    params![asset, quality, faces::to_blob(e)],
                )
                .unwrap();
                ids.push(Id(conn.last_insert_rowid()));
            }
        }
        (dir, lib, ids)
    }

    /// Unit vector along `axis`, nudged by `jitter` along the next axis.
    fn vec_near(axis: usize, jitter: f32) -> Vec<f32> {
        let mut v = vec![0.0; EMBEDDING_DIM];
        v[axis] = 1.0;
        v[axis + 1] = jitter;
        let n = (1.0 + jitter * jitter).sqrt();
        v.iter().map(|x| x / n).collect()
    }

    #[test]
    fn groups_then_recognises_after_naming() {
        let d = "2023-05-01T10:00:00";
        let (_dir, lib, ids) = library_with_faces(&[
            (vec_near(0, 0.0), d, 0.9),
            (vec_near(0, 0.1), d, 0.8),
            (vec_near(0, 0.2), d, 0.7),
            (vec_near(10, 0.0), d, 0.9),
            (vec_near(10, 0.1), d, 0.9),
            (vec_near(10, 0.15), d, 0.6),
            (vec_near(50, 0.0), d, 0.9), // a one-off stranger
        ]);
        let r = lib.recognize().unwrap();
        assert_eq!((r.faces, r.recognized), (7, 0));
        let groups = lib.face_groups(10).unwrap();
        assert_eq!(groups.iter().map(|g| g.size).collect::<Vec<_>>(), vec![3, 3]);

        // Name one face from the first group; recognition picks up the rest.
        let malika =
            lib.assign_faces(&[ids[0]], &PersonRef { person_id: None, name: Some(" Malika ".into()) }).unwrap();
        let r = lib.recognize().unwrap();
        assert_eq!(r.recognized, 2);
        let p = lib.person(malika).unwrap();
        assert_eq!((p.name.as_str(), p.face_count, p.photo_count), ("Malika", 3, 3));
        assert_eq!(lib.face_groups(10).unwrap().len(), 1);

        // Same name reuses the person.
        assert_eq!(
            lib.assign_faces(&[ids[1]], &PersonRef { person_id: None, name: Some("malika".into()) }).unwrap(),
            malika
        );

        // Rejected faces aren't suggested again.
        lib.reject_faces(&[ids[2]]).unwrap();
        lib.recognize().unwrap();
        assert_eq!(lib.person(malika).unwrap().face_count, 2);
    }

    #[test]
    fn nobody_appears_before_birth() {
        let (_dir, lib, ids) = library_with_faces(&[
            (vec_near(0, 0.0), "2021-03-01T10:00:00", 0.9),
            (vec_near(0, 0.05), "2021-04-01T10:00:00", 0.9),
            (vec_near(0, 0.05), "2019-01-01T10:00:00", 0.9), // two years before birth
        ]);
        let baby = lib.assign_faces(&[ids[0]], &PersonRef { person_id: None, name: Some("Divik".into()) }).unwrap();
        lib.update_person(
            baby,
            &PersonUpdate { name: "Divik".into(), birth_date: Some("2020-10-03".into()), is_family: true },
        )
        .unwrap();
        lib.recognize().unwrap();
        let faces = lib.person_faces(baby, false).unwrap();
        assert_eq!(faces.iter().map(|f| f.id).collect::<Vec<_>>().len(), 2);
        assert!(faces.iter().all(|f| f.id != ids[2]));
    }

    #[test]
    fn timeline_filters_by_people_together_and_alone() {
        let d = "2023-05-01T10:00:00";
        let (_dir, lib, ids) = library_with_faces(&[
            (vec_near(0, 0.0), d, 0.9),  // photo 1: A
            (vec_near(10, 0.0), d, 0.9), // photo 2: B
            (vec_near(0, 0.0), d, 0.9),  // photo 3: A ...
        ]);
        let a = lib.assign_faces(&[ids[0], ids[2]], &PersonRef { person_id: None, name: Some("A".into()) }).unwrap();
        let b = lib.assign_faces(&[ids[1]], &PersonRef { person_id: None, name: Some("B".into()) }).unwrap();
        // ...plus B and a clear stranger in photo 3.
        {
            let conn = lib.conn();
            let asset3: i64 =
                conn.query_row("SELECT asset_id FROM faces WHERE id = ?1", [ids[2]], |r| r.get(0)).unwrap();
            conn.execute(
                "INSERT INTO faces (asset_id, bbox, det_score, quality, person_id, assign_kind) VALUES (?1, '[0,0,0.1,0.1]', 0.9, 0.9, ?2, 'auto')",
                params![asset3, b],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO faces (asset_id, bbox, det_score, quality) VALUES (?1, '[0.5,0,0.1,0.1]', 0.9, 0.8)",
                [asset3],
            )
            .unwrap();
        }
        let count = |people: Vec<Id>, only| {
            lib.timeline(Some(&crate::Query { people, only, ..Default::default() })).unwrap().len()
        };
        assert_eq!(count(vec![a], false), 2);
        assert_eq!(count(vec![a, b], false), 1);
        assert_eq!(count(vec![a, b], true), 0); // the stranger is in it
        assert_eq!(count(vec![a], true), 1);
        assert_eq!(count(vec![b], true), 1);
        let stranger: Id = lib.read().query_row("SELECT max(id) FROM faces", [], |r| r.get(0)).unwrap();
        lib.ignore_faces(&[stranger]).unwrap();
        assert_eq!(count(vec![a, b], true), 1); // ignored faces don't count
    }

    #[test]
    fn rejects_bad_names_and_dates() {
        let (_dir, lib, ids) = library_with_faces(&[(vec_near(0, 0.0), "2021-03-01T10:00:00", 0.9)]);
        assert!(matches!(
            lib.assign_faces(&ids, &PersonRef { person_id: None, name: Some("  ".into()) }),
            Err(Error::Invalid(_))
        ));
        let p = lib.assign_faces(&ids, &PersonRef { person_id: None, name: Some("Aarna".into()) }).unwrap();
        let bad = PersonUpdate { name: "Aarna".into(), birth_date: Some("Nov 2024".into()), is_family: true };
        assert!(matches!(lib.update_person(p, &bad), Err(Error::Invalid(_))));
    }
}
