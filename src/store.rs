use super::Result;
use rusqlite::{params, Connection, TransactionBehavior};
use serde_json::{json, Value};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};

pub struct Store(Connection);

pub fn directory() -> Result<PathBuf> {
    if let Some(path) = env::var_os("STAINLESS_HOME") {
        if path.is_empty() {
            return Err("STAINLESS_HOME is empty".into());
        }
        return Ok(PathBuf::from(path));
    }
    let base = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state")))
        .ok_or("set STAINLESS_HOME to a private directory")?;
    Ok(base.join("stainless"))
}

fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty() || name.len() > 256 {
        return Err("name must contain 1–256 bytes".into());
    }
    Ok(())
}
pub fn verify_backup(path: &Path) -> Result<()> {
    let db = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let good: String = db.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let broken: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check)",
        [],
        |r| r.get(0),
    )?;
    if good != "ok" || !(1..=3).contains(&version) || broken {
        return Err("invalid or unsupported StainlessSteel snapshot".into());
    }
    db.prepare("SELECT s.id,s.name,s.workspace,e.seq,e.session,e.kind,e.body,e.at,t.id,t.session,t.model,t.status,t.reply FROM sessions s,events e,turns t LIMIT 0")?;
    Ok(())
}
impl Store {
    pub fn open() -> Result<Self> {
        let dir = directory()?;
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        builder.mode(0o700);
        builder.recursive(true).create(&dir)?;
        let meta = fs::symlink_metadata(&dir)?;
        if !meta.is_dir() {
            return Err("state path must be a private directory".into());
        }
        #[cfg(unix)]
        if meta.permissions().mode() & 0o077 != 0 {
            return Err("state directory must have mode 0700".into());
        }
        let mut db = Connection::open(dir.join("sessions.sqlite3"))?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;",
        )?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version == 0 {
            tx.execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY, name TEXT NOT NULL, workspace TEXT NOT NULL);
                CREATE TABLE events(seq INTEGER PRIMARY KEY, session TEXT NOT NULL REFERENCES sessions(id), kind TEXT NOT NULL, body TEXT NOT NULL, at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
                CREATE INDEX session_events ON events(session, seq);
                CREATE TABLE turns(id INTEGER PRIMARY KEY REFERENCES events(seq), session TEXT NOT NULL REFERENCES sessions(id), model TEXT NOT NULL, status TEXT NOT NULL, reply TEXT);
                CREATE UNIQUE INDEX one_active ON turns(session) WHERE status='running';
                PRAGMA user_version=1;")?;
            version = 1;
        }
        if version == 1 {
            tx.execute_batch(
                "ALTER TABLE sessions ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
                PRAGMA user_version=2;",
            )?;
            version = 2;
        }
        if version == 2 {
            tx.execute_batch("CREATE TABLE agents(id TEXT PRIMARY KEY, session TEXT NOT NULL REFERENCES sessions(id), turn INTEGER NOT NULL, pid INTEGER NOT NULL, prompt TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'running', log TEXT NOT NULL, created TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
                PRAGMA user_version=3;")?;
            version = 3;
        }
        if version != 3 {
            return Err(
                "unsupported journal schema; use a matching version of StainlessSteel".into(),
            );
        }
        tx.commit()?;
        Ok(Self(db))
    }
    pub fn new_session(&mut self, name: &str, workspace: &Path) -> Result<String> {
        validate_name(name)?;
        let ws = workspace.canonicalize()?;
        if !ws.is_dir() {
            return Err("workspace must be a directory".into());
        }
        let ws = ws.to_str().ok_or("workspace path must be UTF-8")?;
        let tx = self.0.transaction()?;
        let id: String = tx.query_row(
            "INSERT INTO sessions(id,name,workspace) VALUES('ses_' || lower(hex(randomblob(16))),?1,?2) RETURNING id",
            params![name, ws],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO events(session,kind,body) VALUES(?1,'created',?2)",
            params![id, name],
        )?;
        tx.commit()?;
        Ok(id)
    }
    pub fn rename(&mut self, id: &str, name: &str) -> Result<()> {
        validate_name(name)?;
        let tx = self.0.transaction()?;
        tx.execute("UPDATE sessions SET name=?1 WHERE id=?2", params![name, id])?;
        tx.execute(
            "INSERT INTO events(session,kind,body) VALUES(?1,'renamed',?2)",
            params![id, name],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn backup(&self, path: &Path) -> Result<()> {
        use std::fs::OpenOptions;
        #[cfg(unix)]
        use std::os::unix::fs::OpenOptionsExt;
        let nonce: String = self
            .0
            .query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))?;
        let tmp = path.with_file_name(format!(".stainless-{nonce}.tmp"));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let file = options.open(&tmp)?;
        let result = (|| -> Result<()> {
            self.0.execute(
                "VACUUM INTO ?1",
                [tmp.to_str().ok_or("backup path must be UTF-8")?],
            )?;
            verify_backup(&tmp)?;
            file.sync_all()?;
            fs::hard_link(&tmp, path)?; // Publish a complete snapshot without replacing any existing file.
            Ok(())
        })();
        fs::remove_file(tmp)?;
        result
    }
    pub fn preview(&self, id: &str) -> Result<Vec<(String, String)>> {
        let mut q=self.0.prepare("SELECT kind,substr(body,1,1200) FROM events WHERE session=?1 ORDER BY seq DESC LIMIT 20")?;
        let rows = q.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let mut events: Vec<_> = rows.collect::<rusqlite::Result<_>>()?;
        events.reverse();
        Ok(events)
    }
    pub fn list(&self) -> Result<Vec<Value>> {
        self.list_filtered(false)
    }
    pub fn list_all(&self) -> Result<Vec<Value>> {
        self.list_filtered(true)
    }
    fn list_filtered(&self, all: bool) -> Result<Vec<Value>> {
        let sql = if all {
            "SELECT s.id,s.name,s.workspace,s.archived,max(e.seq) AS latest FROM sessions s JOIN events e ON s.id=e.session GROUP BY s.id ORDER BY latest DESC"
        } else {
            "SELECT s.id,s.name,s.workspace,s.archived,max(e.seq) AS latest FROM sessions s JOIN events e ON s.id=e.session WHERE s.archived=0 GROUP BY s.id ORDER BY latest DESC"
        };
        let mut q = self.0.prepare(sql)?;
        let rows = q.query_map([], |r| {
            Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"workspace":r.get::<_,String>(2)?,"archived":r.get::<_,i64>(3)?}))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
    pub fn archive(&mut self, id: &str) -> Result<()> {
        let tx = self.0.transaction()?;
        let n = tx.execute("UPDATE sessions SET archived=1 WHERE id=?1", [id])?;
        if n != 1 {
            return Err("session not found".into());
        }
        tx.execute(
            "INSERT INTO events(session,kind,body) VALUES(?1,'archived','')",
            [id],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn fork(&mut self, id: &str, at: Option<i64>) -> Result<String> {
        let (name, workspace): (String, String) = self
            .0
            .query_row(
                "SELECT name,workspace FROM sessions WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| -> Box<dyn std::error::Error> { "session not found".into() })?;
        let tx = self.0.transaction()?;
        let new_id: String = tx.query_row(
            "INSERT INTO sessions(id,name,workspace) VALUES('ses_' || lower(hex(randomblob(16))),?1,?2) RETURNING id",
            params![format!("{name} (fork)"), workspace],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO events(session,kind,body,at) SELECT ?1,kind,body,at FROM events WHERE session=?2 AND (?3 IS NULL OR seq<=?3) ORDER BY seq",
            params![new_id, id, at],
        )?;
        tx.execute(
            "INSERT INTO events(session,kind,body) VALUES(?1,'forked',?2)",
            params![new_id, id],
        )?;
        tx.commit()?;
        Ok(new_id)
    }
    pub fn import(&mut self, name: &str, workspace: &Path, doc: &Value) -> Result<String> {
        if doc["schema"] != 1 {
            return Err("unsupported session export schema".into());
        }
        let source = doc["session"].as_str().unwrap_or("unknown").to_owned();
        let events = doc["events"].as_array().ok_or("export has no events")?;
        let ws = workspace.canonicalize()?;
        if !ws.is_dir() {
            return Err("workspace must be a directory".into());
        }
        let tx = self.0.transaction()?;
        let new_id: String = tx.query_row(
            "INSERT INTO sessions(id,name,workspace) VALUES('ses_' || lower(hex(randomblob(16))),?1,?2) RETURNING id",
            params![name, ws.to_str().ok_or("workspace path must be UTF-8")?],
            |r| r.get(0),
        )?;
        for e in events {
            let kind = e["kind"].as_str().ok_or("export event missing kind")?;
            let body = e["body"].as_str().ok_or("export event missing body")?;
            tx.execute(
                "INSERT INTO events(session,kind,body) VALUES(?1,?2,?3)",
                params![new_id, kind, body],
            )?;
        }
        tx.execute(
            "INSERT INTO events(session,kind,body) VALUES(?1,'imported',?2)",
            params![new_id, source],
        )?;
        tx.commit()?;
        Ok(new_id)
    }
    pub fn resolve(&self, selector: &str) -> Result<String> {
        let sql = if selector == "last" {
            "SELECT session FROM events ORDER BY seq DESC LIMIT 1"
        } else {
            "SELECT id FROM sessions WHERE id=?1"
        };
        use rusqlite::OptionalExtension;
        let mut q = self.0.prepare(sql)?;
        let p: Vec<&str> = if selector == "last" {
            vec![]
        } else {
            vec![selector]
        };
        q.query_row(rusqlite::params_from_iter(p), |r| r.get(0))
            .optional()?
            .ok_or_else(|| "session not found".into())
    }
    pub fn events(&self, id: &str) -> Result<Vec<Value>> {
        let mut q = self
            .0
            .prepare("SELECT seq,kind,body,at FROM events WHERE session=?1 ORDER BY seq")?;
        let rows=q.query_map([id],|r|Ok(json!({"seq":r.get::<_,i64>(0)?,"kind":r.get::<_,String>(1)?,"body":r.get::<_,String>(2)?,"at":r.get::<_,String>(3)?})))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
    pub fn new_agent(&self, session: &str, prompt: &str) -> Result<String> {
        let id: String = self.0.query_row(
            "INSERT INTO agents(id,session,turn,pid,prompt,log) VALUES('agt_' || lower(hex(randomblob(16))),?1,-1,-1,?2,'') RETURNING id",
            params![session, prompt],
            |r| r.get(0),
        )?;
        Ok(id)
    }
    pub fn set_agent_launch(&self, id: &str, turn: i64, pid: i64, log: &str) -> Result<()> {
        let n = self.0.execute(
            "UPDATE agents SET turn=?1,pid=?2,log=?3 WHERE id=?4 AND status='running'",
            params![turn, pid, log, id],
        )?;
        if n != 1 {
            return Err("agent is no longer running".into());
        }
        Ok(())
    }
    pub fn set_agent_status(&self, id: &str, status: &str) -> Result<()> {
        let n = self.0.execute(
            "UPDATE agents SET status=?1 WHERE id=?2",
            params![status, id],
        )?;
        if n != 1 {
            return Err("agent not found".into());
        }
        Ok(())
    }
    fn agent_row(r: &rusqlite::Row) -> rusqlite::Result<Value> {
        Ok(
            json!({"id":r.get::<_,String>(0)?,"session":r.get::<_,String>(1)?,"turn":r.get::<_,i64>(2)?,"pid":r.get::<_,i64>(3)?,"prompt":r.get::<_,String>(4)?,"status":r.get::<_,String>(5)?,"log":r.get::<_,String>(6)?}),
        )
    }
    pub fn agent(&self, id: &str) -> Result<Value> {
        use rusqlite::OptionalExtension;
        self.0
            .query_row(
                "SELECT id,session,turn,pid,prompt,status,log FROM agents WHERE id=?1",
                [id],
                Self::agent_row,
            )
            .optional()?
            .ok_or_else(|| "agent not found".into())
    }
    pub fn agents(&self, session: Option<&str>) -> Result<Vec<Value>> {
        let sql = match session {
            Some(_) => "SELECT id,session,turn,pid,prompt,status,log FROM agents WHERE session=?1 ORDER BY created",
            None => "SELECT id,session,turn,pid,prompt,status,log FROM agents ORDER BY created",
        };
        let mut q = self.0.prepare(sql)?;
        let rows = match session {
            Some(s) => q.query_map([s], Self::agent_row)?,
            None => q.query_map([], Self::agent_row)?,
        };
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
    pub fn running_agent_count(&self) -> Result<i64> {
        Ok(self.0.query_row(
            "SELECT COUNT(*) FROM agents WHERE status='running'",
            [],
            |r| r.get(0),
        )?)
    }
    pub fn context_stats(&self, id: &str) -> Result<Value> {
        let mut kinds = serde_json::Map::new();
        {
            let mut q = self
                .0
                .prepare("SELECT kind,COUNT(*) FROM events WHERE session=?1 GROUP BY kind")?;
            let rows = q.query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
            for r in rows {
                let (kind, count) = r?;
                kinds.insert(kind, json!(count));
            }
        }
        let mut turns = serde_json::Map::new();
        {
            let mut q = self
                .0
                .prepare("SELECT status,COUNT(*) FROM turns WHERE session=?1 GROUP BY status")?;
            let rows = q.query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
            for r in rows {
                let (status, count) = r?;
                turns.insert(status, json!(count));
            }
        }
        for missing in ["complete", "interrupted", "running"] {
            turns.entry(missing.to_owned()).or_insert(json!(0));
        }
        let events: i64 =
            self.0
                .query_row("SELECT COUNT(*) FROM events WHERE session=?1", [id], |r| {
                    r.get(0)
                })?;
        let bytes: i64 = self
            .0
            .query_row(
                "SELECT COALESCE(SUM(LENGTH(body)),0) FROM events WHERE session=?1",
                [id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        Ok(
            json!({"session":id,"events":events,"bytes":bytes,"budget":262144,"by_kind":kinds,"turns":turns}),
        )
    }
    pub fn checkpoint(&self, id: &str) -> Result<i64> {
        self.0.execute(
            "INSERT INTO events(session,kind,body) VALUES(?1,'checkpoint','')",
            [id],
        )?;
        Ok(self.0.last_insert_rowid())
    }
    pub fn begin_turn(&mut self, id: &str, prompt: &str, model: &str) -> Result<(i64, Vec<Value>)> {
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM turns WHERE session=?1 AND status='running')",
            [id],
            |r| r.get::<_, bool>(0),
        )? {
            return Err("session already has an active turn; inspect it before recovery".into());
        }
        let mut hist: Vec<(String, String)> = Vec::new();
        let mut used = prompt.len();
        {
            let mut q=tx.prepare("SELECT e.body,t.reply FROM turns t JOIN events e ON t.id=e.seq WHERE t.session=?1 AND t.status='complete' ORDER BY t.id DESC LIMIT 32")?;
            let mut rows = q.query([id])?;
            while let Some(r) = rows.next()? {
                let (a, b): (String, String) = (r.get(0)?, r.get(1)?);
                used += a.len() + b.len();
                if used > 262144 {
                    break;
                }
                hist.push((a, b));
            }
        }
        let mut msgs = vec![];
        for (a, b) in hist.into_iter().rev() {
            msgs.push(json!({"role":"user","content":a}));
            msgs.push(json!({"role":"assistant","content":b}));
        }
        msgs.push(json!({"role":"user","content":prompt}));
        tx.execute(
            "INSERT INTO events(session,kind,body) VALUES(?1,'user',?2)",
            params![id, prompt],
        )?;
        let n = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO turns VALUES(?1,?2,?3,'running',NULL)",
            params![n, id, model],
        )?;
        tx.commit()?;
        Ok((n, msgs))
    }
    pub fn finish(&mut self, id: &str, turn: i64, reply: Option<&str>) -> Result<()> {
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (status, kind) = if reply.is_some() {
            ("complete", "assistant")
        } else {
            ("interrupted", "interrupted")
        };
        let n = tx.execute(
            "UPDATE turns SET status=?1, reply=?2 WHERE session=?3 AND id=?4 AND status='running'",
            params![status, reply, id, turn],
        )?;
        if n != 1 {
            return Err("turn is no longer active".into());
        }
        tx.execute(
            "INSERT INTO events(session,kind,body) VALUES(?1,?2,?3)",
            params![
                id,
                kind,
                reply.unwrap_or("Turn interrupted. No automatic retry.")
            ],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn turn_status(&self, session: &str, turn: i64) -> Result<Option<String>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .0
            .query_row(
                "SELECT status FROM turns WHERE session=?1 AND id=?2",
                params![session, turn],
                |r| r.get(0),
            )
            .optional()?)
    }
    fn complete_history(&self, session: &str) -> Result<(Vec<Value>, usize)> {
        let mut hist: Vec<(String, String)> = Vec::new();
        let mut used = 0usize;
        {
            let mut q=self.0.prepare("SELECT e.body,t.reply FROM turns t JOIN events e ON t.id=e.seq WHERE t.session=?1 AND t.status='complete' ORDER BY t.id DESC LIMIT 32")?;
            let mut rows = q.query([session])?;
            while let Some(r) = rows.next()? {
                let (a, b): (String, String) = (r.get(0)?, r.get(1)?);
                used += a.len() + b.len();
                if used > 262144 {
                    break;
                }
                hist.push((a, b));
            }
        }
        let mut msgs = vec![];
        for (a, b) in hist.into_iter().rev() {
            msgs.push(json!({"role":"user","content":a}));
            msgs.push(json!({"role":"assistant","content":b}));
        }
        Ok((msgs, used))
    }
    /// Rebuild the exact model input for an active turn: completed history
    /// plus the prompt recorded when the turn began. Fails when the turn is
    /// not active, so a supervisor can never replay a finished turn.
    pub fn turn_messages(&self, session: &str, turn: i64) -> Result<Vec<Value>> {
        let prompt: String = self
            .0
            .query_row(
                "SELECT e.body FROM turns t JOIN events e ON t.id=e.seq WHERE t.session=?1 AND t.id=?2 AND t.status='running'",
                params![session, turn],
                |r| r.get(0),
            )
            .map_err(|_| -> Box<dyn std::error::Error> { "turn is not active".into() })?;
        let (mut msgs, _) = self.complete_history(session)?;
        msgs.push(json!({"role":"user","content":prompt}));
        Ok(msgs)
    }
    pub fn pending(&self, id: &str) -> Result<Option<i64>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .0
            .query_row(
                "SELECT id FROM turns WHERE session=?1 AND status='running'",
                [id],
                |r| r.get(0),
            )
            .optional()?)
    }
}
