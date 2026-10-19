use serde::{Deserialize, Serialize};

/// Database row id. SQLite rowids are `i64`, but ours stay far below 2^53,
/// so the UI receives them as plain JavaScript numbers without precision loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type)]
#[serde(transparent)]
#[specta(transparent)]
pub struct Id(#[specta(type = specta_typescript::Number)] pub i64);

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl rusqlite::types::FromSql for Id {
    fn column_result(v: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        i64::column_result(v).map(Id)
    }
}

impl rusqlite::ToSql for Id {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}
