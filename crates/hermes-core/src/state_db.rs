// 1:1 port of hermes_state.py SessionDB
use rusqlite::{Connection, params, Result as SqlResult};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use chrono::Utc;
use uuid::Uuid;

pub struct SessionDB {
    conn: Mutex<Connection>,
    db_path: PathBuf,
}

impl SessionDB {
    pub fn open(path: impl Into<PathBuf>) -> SqlResult<Self> {
        let path: PathBuf = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
        let db = Self { conn: Mutex::new(conn), db_path: path };
        db.init_schema()?;
        Ok(db)
    }

    pub fn db_path(&self) -> &Path { &self.db_path }

    fn init_schema(&self) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL UNIQUE,
                source TEXT NOT NULL DEFAULT 'cli',
                model TEXT,
                system_prompt TEXT,
                title TEXT,
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                ended_at TEXT,
                end_reason TEXT,
                turn_count INTEGER DEFAULT 0,
                prompt_tokens INTEGER DEFAULT 0,
                completion_tokens INTEGER DEFAULT 0,
                total_tokens INTEGER DEFAULT 0,
                metadata TEXT DEFAULT '{}'
            );
            CREATE INDEX IF NOT EXISTS idx_sessions_created ON sessions(created_at);
            CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);
            CREATE INDEX IF NOT EXISTS idx_sessions_title ON sessions(title);
        ")?;
        Ok(())
    }

    pub fn create_session(&self, session_id: &str, source: &str) -> SqlResult<String> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO sessions (id, session_id, source, status, created_at, updated_at) VALUES (?1, ?2, ?3, 'active', ?4, ?4)",
            params![id, session_id, source, now],
        )?;
        Ok(id)
    }

    pub fn end_session(&self, session_id: &str, reason: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sessions SET status='ended', ended_at=?1, end_reason=?2, updated_at=?1 WHERE session_id=?3",
            params![now, reason, session_id],
        )?;
        Ok(())
    }

    pub fn update_tokens(&self, session_id: &str, prompt: u64, completion: u64) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE sessions SET prompt_tokens=prompt_tokens+?1, completion_tokens=completion_tokens+?2, total_tokens=total_tokens+?1+?2 WHERE session_id=?3",
            params![prompt as u32, completion as u32, session_id],
        )?;
        Ok(())
    }

    pub fn set_title(&self, session_id: &str, title: &str) -> SqlResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE sessions SET title=?1, updated_at=?2 WHERE session_id=?3",
            params![title, Utc::now().to_rfc3339(), session_id],
        )?;
        Ok(rows > 0)
    }

    pub fn get_session(&self, session_id: &str) -> SqlResult<Option<SessionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, source, model, title, status, created_at, updated_at, turn_count, prompt_tokens, completion_tokens, total_tokens FROM sessions WHERE session_id=?1"
        )?;
        let mut rows = stmt.query_map(params![session_id], |row| {
            Ok(SessionRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                source: row.get(2)?,
                model: row.get(3)?,
                title: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                turn_count: row.get(8)?,
                prompt_tokens: row.get(9)?,
                completion_tokens: row.get(10)?,
                total_tokens: row.get(11)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    pub fn list_sessions(&self, limit: i64, offset: i64) -> SqlResult<Vec<SessionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, source, model, title, status, created_at, updated_at, turn_count, prompt_tokens, completion_tokens, total_tokens FROM sessions ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
        )?;
        let rows = stmt.query_map(params![limit, offset], |row| {
            Ok(SessionRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                source: row.get(2)?,
                model: row.get(3)?,
                title: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                turn_count: row.get(8)?,
                prompt_tokens: row.get(9)?,
                completion_tokens: row.get(10)?,
                total_tokens: row.get(11)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows { if let Ok(r) = row { result.push(r); } }
        Ok(result)
    }

    pub fn search_sessions(&self, query: &str) -> SqlResult<Vec<SessionRow>> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT id, session_id, source, model, title, status, created_at, updated_at, turn_count, prompt_tokens, completion_tokens, total_tokens FROM sessions WHERE title LIKE ?1 OR session_id LIKE ?1 ORDER BY created_at DESC LIMIT 50"
        )?;
        let rows = stmt.query_map(params![pattern], |row| {
            Ok(SessionRow {
                id: row.get(0)?, session_id: row.get(1)?, source: row.get(2)?,
                model: row.get(3)?, title: row.get(4)?, status: row.get(5)?,
                created_at: row.get(6)?, updated_at: row.get(7)?,
                turn_count: row.get(8)?, prompt_tokens: row.get(9)?,
                completion_tokens: row.get(10)?, total_tokens: row.get(11)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows { if let Ok(r) = row { result.push(r); } }
        Ok(result)
    }

    pub fn prune_empty_sessions(&self) -> SqlResult<u32> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "DELETE FROM sessions WHERE turn_count = 0 AND status = 'active' AND created_at < datetime('now', '-7 days')",
            [],
        )?;
        Ok(count as u32)
    }

    pub fn stats(&self) -> SqlResult<SessionStats> {
        let conn = self.conn.lock().unwrap();
        let total: u64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?;
        let active: u64 = conn.query_row("SELECT COUNT(*) FROM sessions WHERE status='active'", [], |r| r.get(0))?;
        let tokens: u64 = conn.query_row("SELECT COALESCE(SUM(total_tokens),0) FROM sessions", [], |r| r.get(0))?;
        Ok(SessionStats { total, active, total_tokens: tokens })
    }
}

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: String, pub session_id: String, pub source: String,
    pub model: Option<String>, pub title: Option<String>, pub status: String,
    pub created_at: String, pub updated_at: String,
    pub turn_count: Option<i64>, pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>, pub total_tokens: Option<i64>,
}

#[derive(Debug)]
pub struct SessionStats {
    pub total: u64, pub active: u64, pub total_tokens: u64,
}
