use super::*;
use crate::{persistence::Store, providers::*};
use async_trait::async_trait;
use std::{
    fs::OpenOptions,
    sync::{atomic::AtomicUsize, Mutex},
};

struct FakeProvider {
    attempts: AtomicUsize,
    first_limit: bool,
    fail: bool,
}
#[async_trait]
impl AIExecutionProvider for FakeProvider {
    fn id(&self) -> &str {
        "fake"
    }
    fn name(&self) -> &str {
        "Fake test provider"
    }
    fn npm_entry(&self) -> &str {
        ""
    }
    fn installation_url(&self) -> &str {
        ""
    }
    fn capabilities(&self, _: &str) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_non_interactive: true,
            supports_streaming: true,
            ..Default::default()
        }
    }
    fn detect(&self) -> Option<Launch> {
        #[cfg(windows)]
        let launch = Launch {
            program: std::path::PathBuf::from(std::env::var("SystemRoot").unwrap())
                .join("System32/WindowsPowerShell/v1.0/powershell.exe"),
            prefix: vec![
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
            ],
        };
        #[cfg(unix)]
        let launch = Launch {
            program: "/bin/sh".into(),
            prefix: vec!["-c".into()],
        };
        Some(launch)
    }
    fn execution_args(&self, _: &ProviderCapabilities, _: Option<&str>) -> Result<Vec<String>> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        #[cfg(windows)]
        let script = if self.fail {
            "[Console]::In.ReadToEnd() | Out-Null; [Console]::Error.WriteLine('process crashed'); exit 2".into()
        } else if self.first_limit && attempt == 0 {
            "[Console]::In.ReadToEnd() | Out-Null; Set-Content changes.txt 'preserved partial work'; [Console]::Error.WriteLine('usage limit reached'); exit 1".into()
        } else {
            format!("[Console]::In.ReadToEnd() | Out-Null; Add-Content changes.txt 'completed attempt {attempt}'; git -c core.hooksPath=/dev/null add changes.txt; git -c core.hooksPath=/dev/null -c user.name=AiFlowTest -c user.email=test@localhost -c commit.gpgsign=false commit -m 'validated fixture {attempt}'; if ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}; [Console]::Out.WriteLine('done')")
        };
        #[cfg(unix)]
        let script = if self.fail {
            "cat >/dev/null; printf 'process crashed\n' >&2; exit 2".into()
        } else if self.first_limit && attempt == 0 {
            "cat >/dev/null; printf 'preserved partial work\n' >changes.txt; printf 'usage limit reached\n' >&2; exit 1".into()
        } else {
            format!("cat >/dev/null; printf 'completed attempt {attempt}\n' >>changes.txt; git -c core.hooksPath=/dev/null add changes.txt; git -c core.hooksPath=/dev/null -c user.name=AiFlowTest -c user.email=test@localhost -c commit.gpgsign=false commit -m 'validated fixture {attempt}'; printf 'done\n'")
        };
        Ok(vec![script])
    }
    fn auth_status_args(&self) -> Option<Vec<&str>> {
        None
    }
    fn login_args(&self) -> Option<Vec<&str>> {
        None
    }
    fn logout_args(&self) -> Option<Vec<&str>> {
        None
    }
    fn parse_output(&self, line: &str) -> serde_json::Value {
        parsed(line)
    }
    fn classify_error(&self, text: &str) -> ErrorKind {
        if text.contains("usage limit") {
            ErrorKind::UsageLimit
        } else {
            ErrorKind::ProcessError
        }
    }
    async fn health_check(&self) -> ProviderStatus {
        ProviderStatus {
            id: self.id().into(),
            name: self.name().into(),
            status: "AVAILABLE".into(),
            version: Some("fixture".into()),
            authentication: "CONNECTED".into(),
            capabilities: self.capabilities(""),
            checked_at: now(),
            diagnostics: "Test-only provider".into(),
            installation_url: String::new(),
        }
    }
}
pub(crate) async fn fixture(
    first_limit: bool,
    fail: bool,
) -> (tempfile::TempDir, Arc<AppState>, Project) {
    let temp = tempfile::tempdir().unwrap();
    let work = temp.path().join("repo");
    std::fs::create_dir(&work).unwrap();
    let path = work.to_str().unwrap();
    git::git(path, &["init", "--initial-branch=main"])
        .await
        .unwrap();
    std::fs::write(work.join("base.txt"), "initial").unwrap();
    git::git(path, &["add", "base.txt"]).await.unwrap();
    git::git(
        path,
        &[
            "-c",
            "user.name=AiFlowTest",
            "-c",
            "user.email=test@localhost",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "initial",
        ],
    )
    .await
    .unwrap();
    let policy = ProjectPolicy {
        allowed_providers: vec!["fake".into()],
        default_provider: "fake".into(),
        test_commands: vec![vec![
            "git".into(),
            "rev-parse".into(),
            "--verify".into(),
            "HEAD".into(),
        ]],
        ..Default::default()
    };
    let project = Project {
        id: "p".into(),
        name: "Test".into(),
        repository: "fixture/repo".into(),
        path: path.into(),
        branch: "main".into(),
        policy,
    };
    let store = Store::open(&temp.path().join("state.db")).unwrap();
    store.save_project(&project).unwrap();
    for n in 1..=2 {
        let issue = Issue {
            number: n,
            title: format!("Fixture {n}"),
            body: "Inspect test repository and implement fixture".into(),
            state: "OPEN".into(),
            labels: vec![],
            assignees: vec![],
            milestone: None,
            url: String::new(),
            updated_at: now(),
            comments: vec![],
        };
        store
            .sql(
                "INSERT INTO issues_cache VALUES('p',?,?)",
                params![n as i64, serde_json::to_string(&issue).unwrap()],
            )
            .unwrap();
        store
            .sql(
                "INSERT INTO queue_items VALUES(?,'p',?,'fake',?,'QUEUED',0)",
                params![format!("q{n}"), n as i64, n as i64],
            )
            .unwrap();
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(temp.path().join("instance"))
        .unwrap();
    let app = Arc::new(AppState {
        store,
        providers: ProviderManager::fixture(Arc::new(FakeProvider {
            attempts: AtomicUsize::new(0),
            first_limit,
            fail,
        })),
        github: crate::github::GitHubService::fixture(),
        statuses: Mutex::new(vec![]),
        active: Mutex::new(std::collections::HashMap::from([(
            "p".into(),
            Arc::new(AtomicBool::new(false)),
        )])),
        handle: Mutex::new(None),
        _instance: file,
    });
    (temp, app, project)
}
#[tokio::test]
async fn queue_advances_only_after_independent_validation() {
    let (_temp, app, p) = fixture(false, false).await;
    step(app.clone(), p.clone()).await.unwrap();
    assert_eq!(app.store.queue().unwrap()[0].state, "SUCCESS");
    assert_eq!(app.store.queue().unwrap()[1].state, "QUEUED");
    step(app.clone(), p).await.unwrap();
    assert!(app
        .store
        .queue()
        .unwrap()
        .iter()
        .all(|q| q.state == "SUCCESS"));
    for e in app.store.executions().unwrap() {
        let events = app.store.events(&e.id, 0).unwrap();
        let validation = events
            .iter()
            .position(|e| e.r#type == "VALIDATION_FINISHED")
            .unwrap();
        let success = events
            .iter()
            .position(|e| e.payload["state"] == "SUCCESS")
            .unwrap();
        assert!(validation < success);
    }
}
#[tokio::test]
async fn quota_resume_preserves_original_head_and_partial_work() {
    let (_temp, app, p) = fixture(true, false).await;
    step(app.clone(), p.clone()).await.unwrap();
    let prior = app.store.executions().unwrap()[0].clone();
    assert_eq!(prior.state, "WAITING_FOR_PROVIDER");
    assert_eq!(app.store.usage().unwrap()[0].reset_at, None);
    app.store
        .sql(
            "UPDATE executions SET retry_at='2020-01-01T00:00:00Z' WHERE id=?",
            [&prior.id],
        )
        .unwrap();
    step(app.clone(), p.clone()).await.unwrap();
    let e = app.store.executions().unwrap()[0].clone();
    assert_eq!(e.state, "SUCCESS");
    assert_eq!(e.provider_id, "fake");
    assert_eq!(e.attempt, 2);
    assert_eq!(e.before_git.unwrap().head, prior.before_git.unwrap().head);
    assert!(
        std::fs::read_to_string(std::path::Path::new(&p.path).join("changes.txt"))
            .unwrap()
            .contains("preserved partial work")
    );
}
#[tokio::test]
async fn failure_and_production_gate_never_advance_silently() {
    let (_temp, app, p) = fixture(false, true).await;
    step(app.clone(), p.clone()).await.unwrap();
    assert_eq!(app.store.queue().unwrap()[0].state, "FAILED");
    step(app.clone(), p.clone()).await.unwrap();
    assert_eq!(app.store.executions().unwrap().len(), 1);
    assert_eq!(app.store.queue().unwrap()[1].state, "QUEUED");
    let mut issue = app.store.issues("p").unwrap()[0].clone();
    issue.labels = vec!["production".into()];
    app.store
        .sql(
            "UPDATE issues_cache SET data=? WHERE project_id='p' AND number=1",
            [serde_json::to_string(&issue).unwrap()],
        )
        .unwrap();
    app.store
        .sql("UPDATE queue_items SET state='QUEUED' WHERE id='q1'", [])
        .unwrap();
    step(app.clone(), p).await.unwrap();
    assert_eq!(app.store.queue().unwrap()[0].state, "BLOCKED");
}

#[tokio::test]
async fn github_report_must_be_confirmed_before_releasing_next_issue() {
    let (_temp, app, p) = fixture(false, false).await;
    app.github
        .fixture_write_failure
        .store(true, Ordering::SeqCst);
    assert!(step(app.clone(), p).await.is_err());
    assert_eq!(app.store.queue().unwrap()[0].state, "VALIDATING");
    assert_eq!(app.store.queue().unwrap()[1].state, "QUEUED");
    assert!(!app
        .store
        .executions()
        .unwrap()
        .iter()
        .any(|e| e.state == "SUCCESS"));
}
#[tokio::test]
async fn failed_validation_reports_blocker_without_closing_or_advancing() {
    let (_temp, app, mut p) = fixture(false, false).await;
    p.policy.test_commands = vec![vec![
        "git".into(),
        "rev-parse".into(),
        "--verify".into(),
        "refs/heads/does-not-exist".into(),
    ]];
    p.policy.close_issue = true;
    step(app.clone(), p).await.unwrap();
    assert_eq!(app.store.queue().unwrap()[0].state, "HUMAN_REVIEW");
    assert_eq!(app.store.queue().unwrap()[1].state, "QUEUED");
    let writes = app.github.fixture_writes.lock().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0]["method"], "POST");
    assert!(writes[0]["body"]["body"]
        .as_str()
        .unwrap()
        .contains("validation blocked"));
}

#[tokio::test]
async fn successful_validation_reports_then_closes_github_before_success() {
    let (_temp, app, mut p) = fixture(false, false).await;
    p.policy.close_issue = true;
    step(app.clone(), p).await.unwrap();
    let writes = app.github.fixture_writes.lock().unwrap();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0]["method"], "POST");
    assert_eq!(writes[1]["method"], "PATCH");
    assert_eq!(app.store.issues("p").unwrap()[0].state, "CLOSED");
    assert_eq!(app.store.queue().unwrap()[0].state, "SUCCESS");
    assert_eq!(app.store.queue().unwrap()[1].state, "QUEUED");
}
