use std::sync::LazyLock;

use rusqlite_migration::{M, Migrations};

/// Forward-only schema migrations, applied in order at start-up.
/// Never edit a released migration; add a new file instead.
pub static MIGRATIONS: LazyLock<Migrations<'static>> =
    LazyLock::new(|| Migrations::new(STEPS.iter().map(|sql| M::up(sql)).collect()));

const STEPS: &[&str] = &[
    include_str!("../migrations/0001_initial.sql"),
    include_str!("../migrations/0002_thumbs.sql"),
    include_str!("../migrations/0003_faces.sql"),
];

pub fn latest_version() -> u32 {
    STEPS.len() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        MIGRATIONS.validate().unwrap();
    }

    #[test]
    fn latest_version_matches_migration_count() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        let v: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, latest_version());
    }
}
