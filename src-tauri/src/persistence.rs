use crate::model::*;
use rusqlite::{params, Connection};
use std::{path::Path, sync::Mutex};

pub struct Store(pub Mutex<Connection>, crate::archive::OutputArchive);
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let c = Connection::open(path).map_err(|e| e.to_string())?;
        c.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")
            .map_err(|e| e.to_string())?;
        c.execute_batch(include_str!("../migrations/001_initial.sql"))
            .map_err(|e| e.to_string())?;
        let archive = crate::archive::OutputArchive::new(
            path.parent().unwrap_or(Path::new(".")).join("output"),
        )?;
        Ok(Self(Mutex::new(c), archive))
    }
    pub fn sql(&self, sql: &str, args: impl rusqlite::Params) -> Result<usize> {
        self.0
            .lock()
            .map_err(|e| e.to_string())?
            .execute(sql, args)
            .map_err(|e| e.to_string())
    }
    pub fn query<T>(
        &self,
        sql: &str,
        args: impl rusqlite::Params,
        mut f: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>> {
        let c = self.0.lock().map_err(|e| e.to_string())?;
        let mut st = c.prepare(sql).map_err(|e| e.to_string())?;
        let rows = st.query_map(args, |r| f(r)).map_err(|e| e.to_string())?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
    pub fn projects(&self) -> Result<Vec<Project>> {
        self.json_rows("SELECT data FROM projects", [])
    }
    pub fn project(&self, id: &str) -> Result<Project> {
        self.json_rows("SELECT data FROM projects WHERE id=?", [id])?
            .pop()
            .ok_or("Project not found".into())
    }
    pub fn json_rows<T: serde::de::DeserializeOwned>(
        &self,
        sql: &str,
        args: impl rusqlite::Params,
    ) -> Result<Vec<T>> {
        self.query(sql, args, |r| r.get::<_, String>(0))?
            .iter()
            .map(|s| serde_json::from_str(s).map_err(|e| e.to_string()))
            .collect()
    }
    pub fn save_project(&self, p: &Project) -> Result<()> {
        self.sql(
            "INSERT OR REPLACE INTO projects VALUES(?,?)",
            params![p.id, serde_json::to_string(p).unwrap()],
        )?;
        self.sql(
            "INSERT OR IGNORE INTO queues(project_id) VALUES(?)",
            [&p.id],
        )?;
        Ok(())
    }
    pub fn issues(&self, p: &str) -> Result<Vec<Issue>> {
        self.json_rows(
            "SELECT data FROM issues_cache WHERE project_id=? ORDER BY number",
            [p],
        )
    }
    pub fn queue(&self) -> Result<Vec<QueueItem>> {
        self.query("SELECT id,project_id,issue_number,provider_id,position,state,approved FROM queue_items ORDER BY position",[],|r|Ok(QueueItem{id:r.get(0)?,project_id:r.get(1)?,issue_number:r.get::<_,i64>(2)? as u64,provider_id:r.get(3)?,position:r.get(4)?,state:r.get(5)?,approved:r.get(6)?}))
    }
    pub fn executions(&self) -> Result<Vec<Execution>> {
        self.query("SELECT id,item_id,project_id,provider_id,state,started_at,finished_at,reason,before_git,after_git,retry_at,attempt,session_id FROM executions ORDER BY started_at DESC",[],|r|Ok(Execution{id:r.get(0)?,item_id:r.get(1)?,project_id:r.get(2)?,provider_id:r.get(3)?,state:r.get(4)?,started_at:r.get(5)?,finished_at:r.get(6)?,reason:r.get(7)?,before_git:r.get::<_,Option<String>>(8)?.and_then(|s|serde_json::from_str(&s).ok()),after_git:r.get::<_,Option<String>>(9)?.and_then(|s|serde_json::from_str(&s).ok()),retry_at:r.get(10)?,attempt:r.get(11)?,session_id:r.get(12)?}))
    }
    pub fn save_execution(&self, e: &Execution) -> Result<()> {
        let c = self.0.lock().map_err(|e| e.to_string())?;
        c.execute_batch("BEGIN IMMEDIATE")
            .map_err(|e| e.to_string())?;
        let result = (|| {
            c.execute(
                "INSERT OR REPLACE INTO executions VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)",
                params![
                    e.id,
                    e.item_id,
                    e.project_id,
                    e.provider_id,
                    e.state,
                    e.started_at,
                    e.finished_at,
                    e.reason,
                    e.before_git
                        .as_ref()
                        .map(|v| serde_json::to_string(v).unwrap()),
                    e.after_git
                        .as_ref()
                        .map(|v| serde_json::to_string(v).unwrap()),
                    e.retry_at,
                    e.attempt,
                    e.session_id
                ],
            )?;
            c.execute(
                "UPDATE queue_items SET state=? WHERE id=?",
                params![e.state, e.item_id],
            )?;
            c.execute(
                "INSERT OR REPLACE INTO execution_attempts VALUES(?,?)",
                params![e.id, serde_json::to_string(e).unwrap()],
            )?;
            Ok::<_, rusqlite::Error>(())
        })();
        if result.is_ok() {
            c.execute_batch("COMMIT")
        } else {
            c.execute_batch("ROLLBACK")
        }
        .map_err(|e| e.to_string())?;
        result.map_err(|e| e.to_string())
    }
    pub fn event(
        &self,
        eid: &str,
        attempt: u32,
        kind: &str,
        payload: serde_json::Value,
    ) -> Result<ExecutionEvent> {
        let mut e = ExecutionEvent {
            event_id: id(),
            execution_id: eid.into(),
            attempt_id: format!("{eid}:{attempt}"),
            sequence: 0,
            timestamp: now(),
            r#type: kind.into(),
            payload,
        };
        let c = self.0.lock().map_err(|e| e.to_string())?;
        let stored = if kind == "OUTPUT_RECEIVED" {
            self.1.append(eid, &e.payload)?
        } else {
            e.payload.clone()
        };
        c.execute("INSERT INTO events(event_id,execution_id,attempt_id,timestamp,type,payload) VALUES(?,?,?,?,?,?)",params![e.event_id,e.execution_id,e.attempt_id,e.timestamp,e.r#type,stored.to_string()]).map_err(|e|e.to_string())?;
        e.sequence = c.last_insert_rowid();
        Ok(e)
    }
    pub fn events(&self, eid: &str, after: i64) -> Result<Vec<ExecutionEvent>> {
        let mut events=self.query("SELECT event_id,execution_id,attempt_id,sequence,timestamp,type,payload FROM events WHERE execution_id=? AND sequence>? ORDER BY sequence LIMIT 2000",params![eid,after],|r|Ok(ExecutionEvent{event_id:r.get(0)?,execution_id:r.get(1)?,attempt_id:r.get(2)?,sequence:r.get(3)?,timestamp:r.get(4)?,r#type:r.get(5)?,payload:serde_json::from_str(&r.get::<_,String>(6)?).unwrap_or_default()}))?;
        for e in &mut events {
            e.payload = self.1.hydrate(e.payload.clone())?;
        }
        Ok(events)
    }
    pub fn audit(&self, action: &str, data: serde_json::Value) -> Result<()> {
        self.sql(
            "INSERT INTO audit_events(timestamp,action,data) VALUES(?,?,?)",
            params![now(), action, crate::security::redact(&data.to_string())],
        )?;
        Ok(())
    }
    pub fn event_window(&self, eid: &str, before: Option<i64>) -> Result<Vec<ExecutionEvent>> {
        let cutoff = before.unwrap_or(i64::MAX);
        let mut events=self.query("SELECT event_id,execution_id,attempt_id,sequence,timestamp,type,payload FROM events WHERE execution_id=? AND (sequence IN (SELECT sequence FROM events WHERE execution_id=? AND sequence<? AND type='OUTPUT_RECEIVED' ORDER BY sequence DESC LIMIT 2000) OR type!='OUTPUT_RECEIVED') ORDER BY sequence",params![eid,eid,cutoff],|r|Ok(ExecutionEvent{event_id:r.get(0)?,execution_id:r.get(1)?,attempt_id:r.get(2)?,sequence:r.get(3)?,timestamp:r.get(4)?,r#type:r.get(5)?,payload:serde_json::from_str(&r.get::<_,String>(6)?).unwrap_or_default()}))?;
        for e in &mut events {
            e.payload = self.1.hydrate(e.payload.clone())?;
        }
        Ok(events)
    }
    pub fn usage(&self) -> Result<Vec<ProviderUsage>> {
        self.query("SELECT provider_id,detected_at,reset_at,retry_after,source,confidence FROM provider_usage",[],|r|Ok(ProviderUsage{provider_id:r.get(0)?,detected_at:r.get(1)?,reset_at:r.get(2)?,retry_after:r.get(3)?,source:r.get(4)?,confidence:r.get(5)?}))
    }
    pub fn save_usage(&self, u: &ProviderUsage) -> Result<()> {
        self.sql(
            "INSERT OR REPLACE INTO provider_usage VALUES(?,?,?,?,?,?)",
            params![
                u.provider_id,
                u.detected_at,
                u.reset_at,
                u.retry_after,
                u.source,
                u.confidence
            ],
        )?;
        Ok(())
    }
    pub fn recover(&self) -> Result<()> {
        // OS-level process/worktree leases are acquired before recovery by the app instance.
        self.sql("UPDATE queues SET running=0", [])?;
        self.sql("UPDATE executions SET state='HUMAN_REVIEW', reason='AiFlow stopped during execution. Inspect worktree and partial changes before retrying.' WHERE state IN ('PREPARING','RUNNING','VALIDATING','COMMITTING')",[])?;
        self.sql("UPDATE queue_items SET state='HUMAN_REVIEW' WHERE state IN ('PREPARING','RUNNING','VALIDATING','COMMITTING')",[])?;
        self.audit(
            "SESSION_RECOVERY",
            serde_json::json!({"action":"queues paused; interrupted attempts require review"}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn execution(item: &str, state: &str) -> Execution {
        Execution {
            id: id(),
            item_id: item.into(),
            project_id: "p".into(),
            provider_id: "codex".into(),
            state: state.into(),
            started_at: now(),
            finished_at: None,
            reason: None,
            before_git: None,
            after_git: None,
            retry_at: None,
            attempt: 1,
            session_id: None,
        }
    }
    #[test]
    fn recovery_preserves_waits_and_does_not_claim_completion() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("test.db");
        let store = Store::open(&path).unwrap();
        for (n, state) in ["RUNNING", "WAITING_FOR_PROVIDER", "SUCCESS"]
            .iter()
            .enumerate()
        {
            let item = format!("item{n}");
            store
                .sql(
                    "INSERT INTO queue_items VALUES(?,'p',?,'codex',?,?,0)",
                    params![item, n as i64, n as i64, state],
                )
                .unwrap();
            let mut e = execution(&item, state);
            e.retry_at = Some("2026-10-22T19:00:00Z".into());
            store.save_execution(&e).unwrap();
        }
        drop(store);
        let store = Store::open(&path).unwrap();
        store.recover().unwrap();
        let rows = store.queue().unwrap();
        assert_eq!(rows[0].state, "HUMAN_REVIEW");
        assert_eq!(rows[1].state, "WAITING_FOR_PROVIDER");
        assert_eq!(rows[2].state, "SUCCESS");
        assert_eq!(
            store
                .executions()
                .unwrap()
                .iter()
                .find(|e| e.item_id == "item1")
                .unwrap()
                .retry_at
                .as_deref(),
            Some("2026-10-22T19:00:00Z")
        );
    }
    #[test]
    fn archived_output_replays_exactly_and_sequence_survives_restart() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("test.db");
        let store = Store::open(&path).unwrap();
        let one = store
            .event(
                "execution-1",
                1,
                "OUTPUT_RECEIVED",
                serde_json::json!({"stream":"stderr","content":"line 1\nPortuguês 日本語"}),
            )
            .unwrap();
        let stored = store
            .query("SELECT payload FROM events", [], |r| r.get::<_, String>(0))
            .unwrap();
        assert!(!stored[0].contains("Português"));
        assert!(stored[0].contains("archive"));
        drop(store);
        let store = Store::open(&path).unwrap();
        let two = store
            .event(
                "execution-1",
                2,
                "STATE_CHANGED",
                serde_json::json!({"state":"VALIDATING"}),
            )
            .unwrap();
        assert!(two.sequence > one.sequence);
        let replay = store.events("execution-1", 0).unwrap();
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].payload, one.payload);
        assert_eq!(replay[1].attempt_id, "execution-1:2");
    }
    #[test]
    fn github_refresh_preserves_operational_history() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(&temp.path().join("test.db")).unwrap();
        store
            .sql(
                "INSERT INTO queue_items VALUES('q','p',7,'claude',0,'RUNNING',0)",
                [],
            )
            .unwrap();
        let e = execution("q", "RUNNING");
        store.save_execution(&e).unwrap();
        let mut i = Issue {
            number: 7,
            title: "Task".into(),
            body: String::new(),
            state: "OPEN".into(),
            labels: vec![],
            assignees: vec![],
            milestone: None,
            url: String::new(),
            updated_at: now(),
            comments: vec![],
        };
        for remote in ["OPEN", "CLOSED"] {
            i.state = remote.into();
            store
                .sql(
                    "INSERT OR REPLACE INTO issues_cache VALUES('p',7,?)",
                    [serde_json::to_string(&i).unwrap()],
                )
                .unwrap();
            assert_eq!(store.queue().unwrap()[0].state, "RUNNING");
            assert_eq!(store.executions().unwrap()[0].id, e.id);
        }
    }
    #[test]
    fn independent_provider_quotas_persist() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("test.db");
        let store = Store::open(&path).unwrap();
        for provider in ["codex", "claude", "gemini"] {
            store
                .save_usage(&ProviderUsage {
                    provider_id: provider.into(),
                    detected_at: now(),
                    reset_at: None,
                    retry_after: Some(900),
                    source: "fixture".into(),
                    confidence: "unknown".into(),
                })
                .unwrap();
        }
        drop(store);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.usage().unwrap().len(), 3);
    }
}
