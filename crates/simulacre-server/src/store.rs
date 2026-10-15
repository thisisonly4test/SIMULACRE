//! The vault: SQLite persistence for saved legends, their journal, and the
//! face library.

use std::path::Path;
use std::sync::Mutex;

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use simulacre_core::Persona;

pub const STATUSES: &[&str] = &["draft", "incubating", "active", "dormant", "burned"];

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("corrupt record: {0}")]
    Json(#[from] serde_json::Error),
    #[error("not found")]
    NotFound,
    #[error("invalid status '{0}'")]
    BadStatus(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub persona: Persona,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: i64,
    pub persona_id: String,
    pub at: String,
    pub kind: String,
    pub text: String,
}

pub struct Store {
    conn: Mutex<Connection>,
}

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS personas (
    id         TEXT PRIMARY KEY,
    seed       TEXT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'draft',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    data       TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS journal (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    persona_id TEXT NOT NULL REFERENCES personas(id) ON DELETE CASCADE,
    at         TEXT NOT NULL,
    kind       TEXT NOT NULL,
    text       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS journal_persona ON journal(persona_id, at);
CREATE TABLE IF NOT EXISTS faces (
    id         TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    source     TEXT NOT NULL
);
"#;

fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::init(Connection::open(path)?)
    }

    pub fn in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, StoreError> {
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        // A panic while holding the lock leaves SQLite consistent (each call is
        // one statement or one transaction), so recovering the guard is safe.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn insert(&self, persona: &Persona) -> Result<Record, StoreError> {
        let id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
        let t = now();
        self.db().execute(
            "INSERT INTO personas (id, seed, status, created_at, updated_at, data) VALUES (?1, ?2, 'draft', ?3, ?3, ?4)",
            params![id, persona.recipe.seed, t, serde_json::to_string(persona)?],
        )?;
        self.journal_add(&id, "system", "created")?;
        self.get(&id)
    }

    pub fn get(&self, id: &str) -> Result<Record, StoreError> {
        let row = self
            .db()
            .query_row("SELECT id, status, created_at, updated_at, data FROM personas WHERE id = ?1", [id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?))
            })
            .optional()?
            .ok_or(StoreError::NotFound)?;
        Ok(Record { id: row.0, status: row.1, created_at: row.2, updated_at: row.3, persona: serde_json::from_str(&row.4)? })
    }

    pub fn list(&self) -> Result<Vec<Record>, StoreError> {
        let db = self.db();
        let mut st = db.prepare("SELECT id, status, created_at, updated_at, data FROM personas ORDER BY updated_at DESC")?;
        let rows = st.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let row = row?;
            out.push(Record { id: row.0, status: row.1, created_at: row.2, updated_at: row.3, persona: serde_json::from_str(&row.4)? });
        }
        Ok(out)
    }

    pub fn update(&self, id: &str, persona: &Persona) -> Result<Record, StoreError> {
        let n = self.db().execute(
            "UPDATE personas SET data = ?2, seed = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, serde_json::to_string(persona)?, persona.recipe.seed, now()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        self.get(id)
    }

    pub fn set_status(&self, id: &str, status: &str) -> Result<Record, StoreError> {
        if !STATUSES.contains(&status) {
            return Err(StoreError::BadStatus(status.into()));
        }
        let before = self.get(id)?.status;
        self.db().execute("UPDATE personas SET status = ?2, updated_at = ?3 WHERE id = ?1", params![id, status, now()])?;
        if before != status {
            self.journal_add(id, "status", &format!("{before} → {status}"))?;
        }
        self.get(id)
    }

    pub fn delete(&self, id: &str) -> Result<(), StoreError> {
        let n = self.db().execute("DELETE FROM personas WHERE id = ?1", [id])?;
        if n == 0 { Err(StoreError::NotFound) } else { Ok(()) }
    }

    pub fn journal_add(&self, id: &str, kind: &str, text: &str) -> Result<JournalEntry, StoreError> {
        let t = now();
        let db = self.db();
        db.execute("INSERT INTO journal (persona_id, at, kind, text) VALUES (?1, ?2, ?3, ?4)", params![id, t, kind, text])?;
        Ok(JournalEntry { id: db.last_insert_rowid(), persona_id: id.into(), at: t, kind: kind.into(), text: text.into() })
    }

    pub fn journal(&self, id: &str) -> Result<Vec<JournalEntry>, StoreError> {
        let db = self.db();
        let mut st = db.prepare("SELECT id, persona_id, at, kind, text FROM journal WHERE persona_id = ?1 ORDER BY id DESC")?;
        let rows = st.query_map([id], |r| {
            Ok(JournalEntry { id: r.get(0)?, persona_id: r.get(1)?, at: r.get(2)?, kind: r.get(3)?, text: r.get(4)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn face_add(&self, id: &str, source: &str) -> Result<(), StoreError> {
        self.db().execute("INSERT OR IGNORE INTO faces (id, created_at, source) VALUES (?1, ?2, ?3)", params![id, now(), source])?;
        Ok(())
    }

    pub fn face_exists(&self, id: &str) -> Result<bool, StoreError> {
        Ok(self.db().query_row("SELECT 1 FROM faces WHERE id = ?1", [id], |_| Ok(())).optional()?.is_some())
    }

    /// Which saved legends use a face (a face shared by two legends links them).
    pub fn face_users(&self, face: &str) -> Result<Vec<String>, StoreError> {
        Ok(self.list()?.into_iter().filter(|r| r.persona.face.as_ref().is_some_and(|f| f.id == face)).map(|r| r.id).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use simulacre_core::{Recipe, generate};

    fn persona(seed: &str) -> Persona {
        generate(&Recipe::new(seed, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())).unwrap()
    }

    #[test]
    fn crud_and_journal() {
        let s = Store::in_memory().unwrap();
        let r = s.insert(&persona("SIM-S1")).unwrap();
        assert_eq!(r.status, "draft");
        assert_eq!(s.list().unwrap().len(), 1);
        let r2 = s.set_status(&r.id, "active").unwrap();
        assert_eq!(r2.status, "active");
        assert!(matches!(s.set_status(&r.id, "nope"), Err(StoreError::BadStatus(_))));
        s.journal_add(&r.id, "note", "opened Instagram").unwrap();
        let j = s.journal(&r.id).unwrap();
        assert_eq!(j.len(), 3, "created + status + note");
        assert_eq!(j[0].text, "opened Instagram");
        let mut p = r.persona.clone();
        p.identity.first_name = "Zed".into();
        assert_eq!(s.update(&r.id, &p).unwrap().persona.identity.first_name, "Zed");
        s.delete(&r.id).unwrap();
        assert!(matches!(s.get(&r.id), Err(StoreError::NotFound)));
        assert!(s.journal(&r.id).unwrap().is_empty(), "journal cascades");
    }

    #[test]
    fn faces() {
        let s = Store::in_memory().unwrap();
        s.face_add("abc", "test").unwrap();
        s.face_add("abc", "test").unwrap();
        assert!(s.face_exists("abc").unwrap());
        assert!(!s.face_exists("zzz").unwrap());
    }
}
