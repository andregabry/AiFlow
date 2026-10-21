mod account_auth;
mod archive;
mod execution;
mod git;
mod github;
mod model;
mod persistence;
mod providers;
mod security;
mod system;
use fs2::FileExt;
use model::*;
use persistence::Store;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::Emitter;
use tauri::Manager;

pub struct AppState {
    store: Store,
    providers: providers::ProviderManager,
    github: github::GitHubService,
    statuses: Mutex<Vec<ProviderStatus>>,
    active: Mutex<HashMap<String, Arc<AtomicBool>>>,
    handle: Mutex<Option<tauri::AppHandle>>,
    _instance: File,
}
impl AppState {
    fn changed(&self) {
        if let Some(h) = self.handle.lock().unwrap().as_ref() {
            let _ = h.emit("aiflow:changed", ());
        }
    }
    fn emit(&self, e: &Execution, kind: &str, payload: Value) -> Result<ExecutionEvent> {
        let event = self.store.event(&e.id, e.attempt, kind, payload)?;
        if let Some(h) = self.handle.lock().unwrap().as_ref() {
            h.emit("aiflow:event", &event).map_err(|e| e.to_string())?;
        }
        Ok(event)
    }
    fn notify(&self, title: &str, body: &str) {
        use tauri_plugin_notification::NotificationExt;
        if let Some(h) = self.handle.lock().unwrap().as_ref() {
            let _ = h.notification().builder().title(title).body(body).show();
        }
    }
}
#[tauri::command]
async fn snapshot(state: tauri::State<'_, Arc<AppState>>) -> Result<Snapshot> {
    let projects = state.store.projects()?;
    let issues = projects
        .iter()
        .map(|p| {
            Ok(ProjectIssues {
                project_id: p.id.clone(),
                issues: state.store.issues(&p.id)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let running_projects =
        state
            .store
            .query("SELECT project_id FROM queues WHERE running=1", [], |r| {
                r.get(0)
            })?;
    let github_account = state.github.account().await;
    Ok(Snapshot {
        projects,
        issues,
        queue: state.store.queue()?,
        executions: state.store.executions()?,
        providers: state.statuses.lock().unwrap().clone(),
        usage: state.store.usage()?,
        running_projects,
        github_account,
    })
}
#[tauri::command]
async fn scan_providers(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<ProviderStatus>> {
    let statuses = state.providers.scan().await;
    for s in &statuses {
        state.store.sql(
            "INSERT OR REPLACE INTO providers VALUES(?,?)",
            rusqlite::params![s.id, serde_json::to_string(s).unwrap()],
        )?;
    }
    *state.statuses.lock().unwrap() = statuses.clone();
    state.changed();
    Ok(statuses)
}
#[tauri::command]
async fn save_project(
    state: tauri::State<'_, Arc<AppState>>,
    mut project: Project,
) -> Result<Project> {
    if !security::safe_repository(&project.repository) {
        return Err("Repository must be owner/name".into());
    }
    if project.name.trim().is_empty() {
        return Err("Project name is required".into());
    }
    if !project.id.is_empty() && state.active.lock().unwrap().contains_key(&project.id) {
        return Err(
            "Pause the queue and wait for its active execution before changing project policy"
                .into(),
        );
    }
    if project.policy.default_provider == "auto" {
        return Err("Automatic routing needs a concrete project default".into());
    }
    state
        .providers
        .select(&project.policy.default_provider, &project.policy)?;
    for cmd in &project.policy.test_commands {
        if cmd.is_empty() || cmd.iter().any(|a| a.contains('\0')) {
            return Err("Invalid validation command arguments".into());
        }
    }
    let s = git::snapshot(&project.path).await?;
    project.path = std::path::Path::new(&project.path)
        .canonicalize()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .into();
    if project.branch.is_empty() {
        project.branch = s.branch;
    }
    if project.id.is_empty() {
        project.id = id();
    }
    state.store.save_project(&project)?;
    state.store.audit(
        "PROJECT_SAVED",
        json!({"project":project.id,"policy":project.policy}),
    )?;
    state.changed();
    Ok(project)
}
#[tauri::command]
async fn inspect_repository(path: String) -> Result<GitSnapshot> {
    git::snapshot(&path).await
}
fn provider_login_origin(provider: &str, url: &tauri::Url) -> bool {
    if url.scheme() != "https" || url.port_or_known_default() != Some(443) {
        return false;
    }
    match provider {
        "codex" => matches!(url.host_str(), Some("auth.openai.com" | "chatgpt.com")),
        "claude" => matches!(
            url.host_str(),
            Some("claude.ai" | "console.anthropic.com" | "platform.claude.com")
        ),
        _ => false,
    }
}
#[tauri::command]
async fn provider_auth_window_open(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    provider_id: String,
    url: String,
) -> Result<()> {
    if window.label() != "main" {
        return Err("Only the main window can open sign-in".into());
    }
    let url: tauri::Url = url
        .parse()
        .map_err(|_| "Invalid official sign-in address")?;
    if !provider_login_origin(&provider_id, &url) {
        return Err("This provider requires its official system-browser sign-in".into());
    }
    let callback = url
        .query_pairs()
        .find(|(k, _)| k == "redirect_uri")
        .and_then(|(_, v)| v.parse::<tauri::Url>().ok())
        .filter(|u| {
            u.scheme() == "http" && matches!(u.host_str(), Some("localhost" | "127.0.0.1"))
        });
    if let Some(auth) = app.get_webview_window("provider-sign-in") {
        auth.close().map_err(|e| e.to_string())?;
    }
    tauri::WebviewWindowBuilder::new(&app, "provider-sign-in", tauri::WebviewUrl::External(url))
        .title(format!("AiFlow | Official {provider_id} CLI sign-in"))
        .inner_size(960.0, 760.0)
        .center()
        .parent(&window)
        .map_err(|e| e.to_string())?
        .incognito(true)
        .on_navigation(move |u| {
            provider_login_origin(&provider_id, u)
                || callback
                    .as_ref()
                    .is_some_and(|c| u.origin() == c.origin() && u.path() == c.path())
        })
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
fn provider_auth_window_close(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<()> {
    if window.label() != "main" {
        return Err("Only the main window can close sign-in".into());
    }
    if let Some(auth) = app.get_webview_window("provider-sign-in") {
        auth.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
async fn github_auth_window_open(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    code: String,
) -> Result<()> {
    if window.label() != "main" {
        return Err("Only the main window can open sign-in".into());
    }
    if !valid_github_user_code(&code) {
        return Err("Invalid GitHub sign-in code".into());
    }
    let title = format!("AiFlow | GitHub | Code: {code}");
    if let Some(auth) = app.get_webview_window("github-sign-in") {
        auth.set_title(&title).map_err(|e| e.to_string())?;
        auth.show().map_err(|e| e.to_string())?;
        auth.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        &app,
        "github-sign-in",
        tauri::WebviewUrl::External("https://github.com/login/device".parse().unwrap()),
    )
    .title(title)
    .inner_size(960.0, 760.0)
    .min_inner_size(640.0, 520.0)
    .center()
    .parent(&window)
    .map_err(|e| e.to_string())?
    .incognito(true)
    // Remote content receives no AiFlow capabilities or injected scripts.
    .on_navigation(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("github.com")
            && url.port_or_known_default() == Some(443)
    })
    .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn valid_github_user_code(code: &str) -> bool {
    code.len() == 9
        && code.as_bytes()[4] == b'-'
        && code
            .bytes()
            .enumerate()
            .all(|(i, b)| i == 4 || b.is_ascii_alphanumeric())
}
#[tauri::command]
fn github_auth_window_close(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<()> {
    if window.label() != "main" {
        return Err("Only the main window can close sign-in".into());
    }
    if let Some(auth) = app.get_webview_window("github-sign-in") {
        auth.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
async fn github_device_start(state: tauri::State<'_, Arc<AppState>>) -> Result<Value> {
    if let Some(client_id) =
        option_env!("AIFLOW_GITHUB_CLIENT_ID").filter(|id| !id.trim().is_empty())
    {
        state.github.device_start(client_id.to_string()).await
    } else {
        github::GitHubService::cli_launch()?;
        Ok(json!({"method":"cli"}))
    }
}
#[tauri::command]
async fn github_cli_start(state: tauri::State<'_, Arc<AppState>>) -> Result<Value> {
    github::GitHubService::cli_launch()?;
    if state
        .github
        .connect_cli(&AtomicBool::new(false))
        .await
        .is_ok()
    {
        state.changed();
        return Ok(json!({"method":"existing"}));
    }
    Ok(json!({"method":"cli"}))
}
#[tauri::command]
async fn github_cli_login(
    state: tauri::State<'_, Arc<AppState>>,
    execution_id: String,
) -> Result<()> {
    if !execution_id.starts_with("auth-") || !security::valid_session_id(&execution_id) {
        return Err("Invalid authentication session".into());
    }
    let launch = github::GitHubService::cli_launch()?;
    let key = format!("auth:{execution_id}");
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut active = state.active.lock().unwrap();
        if !active.is_empty() {
            return Err("Wait for active executions before connecting GitHub".into());
        }
        active.insert(key.clone(), cancel.clone());
    }
    let e = Execution {
        id: execution_id,
        item_id: String::new(),
        project_id: String::new(),
        provider_id: "github".into(),
        state: "RUNNING".into(),
        started_at: now(),
        finished_at: None,
        reason: None,
        before_git: None,
        after_git: None,
        retry_at: None,
        attempt: 1,
        session_id: None,
    };
    let args = [
        "auth",
        "login",
        "--hostname",
        "github.com",
        "--web",
        "--git-protocol",
        "https",
        "--skip-ssh-key",
    ]
    .map(str::to_owned);
    let result = execution::process::run_auth(&launch, &args, cancel.clone(), |stream, raw, _| {
        state
            .emit(
                &e,
                "OUTPUT_RECEIVED",
                json!({"stream":stream,"content":raw}),
            )
            .map(|_| ())
    })
    .await;
    let result = match result {
        Ok(r) if !r.cancelled && !cancel.load(Ordering::SeqCst) && r.code == Some(0) => {
            state.github.connect_cli(&cancel).await
        }
        Ok(r) if r.cancelled || cancel.load(Ordering::SeqCst) => {
            Err("GitHub connection cancelled".into())
        }
        Ok(_) => {
            Err("GitHub sign-in failed. Review the official client output and try again.".into())
        }
        Err(e) => Err(e),
    };
    state.active.lock().unwrap().remove(&key);
    state.changed();
    result
}
#[tauri::command]
async fn github_device_poll(state: tauri::State<'_, Arc<AppState>>) -> Result<Value> {
    state.github.device_poll().await
}
#[tauri::command]
fn github_device_cancel(state: tauri::State<'_, Arc<AppState>>) -> Result<()> {
    state.github.device_cancel()
}
#[tauri::command]
async fn github_repositories(state: tauri::State<'_, Arc<AppState>>) -> Result<Value> {
    state.github.repositories().await
}
#[tauri::command]
fn github_logout(state: tauri::State<'_, Arc<AppState>>) -> Result<()> {
    state.github.logout()?;
    state.changed();
    Ok(())
}
#[tauri::command]
async fn sync_issues(state: tauri::State<'_, Arc<AppState>>, project_id: String) -> Result<usize> {
    let p = state.store.project(&project_id)?;
    let n = state.github.sync(&state.store, &p).await?;
    state.changed();
    Ok(n)
}
#[tauri::command]
fn enqueue(
    state: tauri::State<'_, Arc<AppState>>,
    project_id: String,
    number: u64,
    provider_id: String,
) -> Result<()> {
    let p = state.store.project(&project_id)?;
    state.providers.select(&provider_id, &p.policy)?;
    let i = state
        .store
        .issues(&p.id)?
        .into_iter()
        .find(|i| i.number == number)
        .ok_or("Issue not found")?;
    if i.state != "OPEN" {
        return Err("Reopen this issue on GitHub before adding it to the queue".into());
    }
    state.store.sql("INSERT INTO queue_items(id,project_id,issue_number,provider_id,position,state) VALUES(?,?,?,?,(SELECT COALESCE(MAX(position),0)+1 FROM queue_items),'QUEUED')",rusqlite::params![id(),p.id,number as i64,provider_id]).map_err(|_|"Issue is already in the queue".to_string())?;
    state.changed();
    Ok(())
}
#[tauri::command]
fn queue_control(
    state: tauri::State<'_, Arc<AppState>>,
    project_id: String,
    action: String,
) -> Result<()> {
    state.store.project(&project_id)?;
    match action.as_str() {
        "start" => {
            state.store.sql(
                "UPDATE queues SET running=1,stop_after_current=0 WHERE project_id=?",
                [&project_id],
            )?;
        }
        "pause" => {
            state.store.sql(
                "UPDATE queues SET running=0 WHERE project_id=?",
                [&project_id],
            )?;
        }
        "stop-after" => {
            state.store.sql(
                "UPDATE queues SET stop_after_current=1 WHERE project_id=?",
                [&project_id],
            )?;
        }
        "cancel" => {
            state.store.sql(
                "UPDATE queues SET running=0 WHERE project_id=?",
                [&project_id],
            )?;
            if let Some(c) = state.active.lock().unwrap().get(&project_id) {
                c.store(true, Ordering::SeqCst);
            }
        }
        _ => return Err("Unknown queue action".into()),
    }
    state.store.audit(
        "QUEUE_CONTROL",
        json!({"project":project_id,"action":action}),
    )?;
    state.changed();
    Ok(())
}
#[tauri::command]
fn item_control(
    state: tauri::State<'_, Arc<AppState>>,
    item_id: String,
    action: String,
    provider_id: Option<String>,
) -> Result<()> {
    let item = state
        .store
        .queue()?
        .into_iter()
        .find(|i| i.id == item_id)
        .ok_or("Queue item not found")?;
    if state.active.lock().unwrap().contains_key(&item.project_id) {
        return Err(
            "Pause the queue and wait for the active execution before changing queue items".into(),
        );
    }
    match action.as_str() {
        "retry" => {
            if !execution::state::allowed(&item.state, "QUEUED") {
                return Err("This item is not retryable".into());
            }
            state.store.sql(
                "UPDATE queue_items SET state='QUEUED' WHERE id=?",
                [&item_id],
            )?;
        }
        "approve" => {
            state.store.sql(
                "UPDATE queue_items SET approved=1,state='QUEUED' WHERE id=?",
                [&item_id],
            )?;
        }
        "skip" => {
            state.store.sql(
                "UPDATE queue_items SET state='SKIPPED' WHERE id=?",
                [&item_id],
            )?;
        }
        "remove" => {
            if item.state != "QUEUED" {
                return Err("Only unstarted queue items can be removed".into());
            }
            state
                .store
                .sql("DELETE FROM queue_items WHERE id=?", [&item_id])?;
        }
        "provider" => {
            let p = state.store.project(&item.project_id)?;
            let requested = provider_id.ok_or("Select provider")?;
            state.providers.select(&requested, &p.policy)?;
            state.store.sql(
                "UPDATE queue_items SET provider_id=?,state='QUEUED',approved=0 WHERE id=?",
                rusqlite::params![requested, item_id],
            )?;
            state.store.audit("PROVIDER_CHANGED",json!({"item":item_id,"previous":item.provider_id,"next":requested,"requiresCleanTree":true}))?;
        }
        _ => return Err("Unknown item action".into()),
    }
    state
        .store
        .audit("ITEM_CONTROL", json!({"item":item_id,"action":action}))?;
    state.changed();
    Ok(())
}
#[tauri::command]
fn reorder_queue(
    state: tauri::State<'_, Arc<AppState>>,
    project_id: String,
    ids: Vec<String>,
) -> Result<()> {
    if state.active.lock().unwrap().contains_key(&project_id) {
        return Err("Pause the queue and wait before reordering".into());
    }
    let expected: std::collections::HashSet<_> = state
        .store
        .queue()?
        .into_iter()
        .filter(|i| i.project_id == project_id)
        .map(|i| i.id)
        .collect();
    if ids.len() != expected.len()
        || ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len()
        || ids.iter().any(|id| !expected.contains(id))
    {
        return Err("Queue changed; refresh before reordering".into());
    }
    let c = state.store.0.lock().map_err(|e| e.to_string())?;
    c.execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| e.to_string())?;
    for (n, id) in ids.iter().enumerate() {
        if let Err(err) = c.execute(
            "UPDATE queue_items SET position=? WHERE id=?",
            rusqlite::params![n as i64, id],
        ) {
            let _ = c.execute_batch("ROLLBACK");
            return Err(err.to_string());
        }
    }
    c.execute_batch("COMMIT").map_err(|e| e.to_string())?;
    drop(c);
    state.changed();
    Ok(())
}
#[tauri::command]
fn execution_events(
    state: tauri::State<'_, Arc<AppState>>,
    execution_id: String,
    after: i64,
) -> Result<Vec<ExecutionEvent>> {
    state.store.events(&execution_id, after)
}
#[tauri::command]
fn execution_event_window(
    state: tauri::State<'_, Arc<AppState>>,
    execution_id: String,
    before: Option<i64>,
) -> Result<Vec<ExecutionEvent>> {
    state.store.event_window(&execution_id, before)
}
#[tauri::command]
async fn dry_run(
    state: tauri::State<'_, Arc<AppState>>,
    project_id: String,
    number: u64,
    provider_id: String,
) -> Result<Value> {
    let p = state.store.project(&project_id)?;
    let provider = state.providers.select(&provider_id, &p.policy)?;
    let git = git::validate(&p).await?;
    let i = state
        .store
        .issues(&p.id)?
        .into_iter()
        .find(|i| i.number == number)
        .ok_or("Sync issues first")?;
    let health = provider.health_check().await;
    Ok(
        json!({"provider":health,"git":git,"prompt":execution::prompt::build(&p,&i,None),"wouldCloseIssue":p.policy.close_issue,"validationCommands":p.policy.test_commands,"manualApprovalRequired":i.labels.iter().any(|l|p.policy.manual_approval_labels.contains(l))}),
    )
}
#[tauri::command]
async fn provider_auth(
    state: tauri::State<'_, Arc<AppState>>,
    provider_id: String,
    action: String,
    execution_id: Option<String>,
) -> Result<String> {
    let p = state.providers.get(&provider_id)?;
    let launch = p
        .detect()
        .ok_or("Install the official CLI before connecting this account")?;
    if action == "login" && p.get_authentication_status(&launch).await == "CONNECTED" {
        return Ok("Existing official CLI session verified".into());
    }
    let args = if provider_id == "gemini" && action == "login" {
        vec![]
    } else {
        match action.as_str() { "login"=>p.login_args(), "logout"=>p.logout_args(),_=>None }
        .ok_or("This CLI does not advertise this authentication operation. See its official account instructions.")?
    };
    let execution_id = execution_id.unwrap_or_else(|| format!("auth-{}", id()));
    if !execution_id.starts_with("auth-") || !security::valid_session_id(&execution_id) {
        return Err("Invalid authentication session".into());
    }
    let key = format!("auth:{execution_id}");
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut active = state.active.lock().unwrap();
        if !active.is_empty() {
            return Err(
                "Wait for active executions before changing provider authentication".into(),
            );
        }
        active.insert(key.clone(), cancel.clone());
    }
    let e = Execution {
        id: execution_id,
        item_id: String::new(),
        project_id: String::new(),
        provider_id: p.id().into(),
        state: "RUNNING".into(),
        started_at: now(),
        finished_at: None,
        reason: None,
        before_git: None,
        after_git: None,
        retry_at: None,
        attempt: 1,
        session_id: None,
    };
    let current = std::env::temp_dir().to_string_lossy().into_owned();
    let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
    if provider_id == "gemini" {
        let result = account_auth::gemini(&launch, cancel, |raw| {
            state
                .emit(
                    &e,
                    "OUTPUT_RECEIVED",
                    json!({"stream":"stderr","content":raw}),
                )
                .map(|_| ())
        })
        .await;
        state.active.lock().unwrap().remove(&key);
        result?;
        state.providers.confirm_auth("gemini");
        return Ok("Google account verified by the official Gemini CLI".into());
    }
    let result =
        execution::process::run(&launch, &args, &current, "", p, cancel, |stream, raw, _| {
            state
                .emit(
                    &e,
                    "OUTPUT_RECEIVED",
                    json!({"stream":stream,"content":raw}),
                )
                .map(|_| ())
        })
        .await;
    state.active.lock().unwrap().remove(&key);
    let result = result?;
    if result.cancelled {
        return Err("Account connection cancelled".into());
    }
    if result.code != Some(0) {
        return Err(account_auth::authentication_error(&result.tail));
    }
    Ok("Official authentication command completed".into())
}
#[tauri::command]
async fn cancel_account_connection(
    state: tauri::State<'_, Arc<AppState>>,
    execution_id: String,
) -> Result<()> {
    let key = format!("auth:{execution_id}");
    if let Some(flag) = state.active.lock().unwrap().get(&key) {
        flag.store(true, Ordering::SeqCst);
    }
    for _ in 0..100 {
        if !state.active.lock().unwrap().contains_key(&key) {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Err("The sign-in process has not stopped yet. Wait a moment and try cancelling again.".into())
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let path = app.path().app_data_dir()?;
            std::fs::create_dir_all(&path)?;
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(path.join("instance.lock"))?;
            file.try_lock_exclusive()
                .map_err(|_| "AiFlow is already running. Open its existing window.")?;
            let store = Store::open(&path.join("AiFlow.db"))?;
            store.recover()?;
            let statuses = store.json_rows("SELECT data FROM providers", [])?;
            let state = Arc::new(AppState {
                store,
                providers: providers::ProviderManager::default(),
                github: github::GitHubService::new(path.join("github-cli-connected")),
                statuses: Mutex::new(statuses),
                active: Mutex::new(HashMap::new()),
                handle: Mutex::new(Some(app.handle().clone())),
                _instance: file,
            });
            app.manage(state.clone());
            execution::scheduler(state.clone());
            let scan = state.clone();
            tauri::async_runtime::spawn(async move {
                let statuses = scan.providers.scan().await;
                *scan.statuses.lock().unwrap() = statuses;
                scan.changed();
            });
            let menu = tauri::menu::Menu::with_items(
                app,
                &[
                    &tauri::menu::MenuItem::with_id(
                        app,
                        "open",
                        "Open AiFlow",
                        true,
                        None::<&str>,
                    )?,
                    &tauri::menu::MenuItem::with_id(
                        app,
                        "pause",
                        "Pause all queues",
                        true,
                        None::<&str>,
                    )?,
                    &tauri::menu::MenuItem::with_id(
                        app,
                        "quit",
                        "Quit AiFlow",
                        true,
                        None::<&str>,
                    )?,
                ],
            )?;
            let mut tray = tauri::tray::TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("AiFlow · AI development orchestrator")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "pause" => {
                        let state = app.state::<Arc<AppState>>();
                        let _ = state.store.sql("UPDATE queues SET running=0", []);
                        state.changed();
                    }
                    "quit" => {
                        let state = app.state::<Arc<AppState>>();
                        if state.active.lock().unwrap().is_empty() {
                            app.exit(0);
                        } else {
                            state.notify(
                                "Execution active",
                                "Cancel or finish the current execution before quitting.",
                            );
                        }
                    }
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let s = window.state::<Arc<AppState>>();
                if !s.active.lock().unwrap().is_empty() {
                    api.prevent_close();
                    let _ = window.hide();
                    s.notify(
                        "AiFlow is running in the tray",
                        "Open AiFlow from the tray to view or cancel active work.",
                    );
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            scan_providers,
            save_project,
            inspect_repository,
            provider_auth_window_open,
            provider_auth_window_close,
            github_auth_window_open,
            github_auth_window_close,
            github_device_start,
            github_cli_start,
            github_cli_login,
            github_device_poll,
            github_device_cancel,
            cancel_account_connection,
            github_repositories,
            github_logout,
            sync_issues,
            enqueue,
            queue_control,
            item_control,
            reorder_queue,
            execution_events,
            execution_event_window,
            dry_run,
            provider_auth
        ])
        .run(tauri::generate_context!())
        .expect("Unable to run AiFlow");
}

#[cfg(test)]
mod auth_window_tests {
    #[test]
    fn provider_windows_allow_only_the_selected_official_origin() {
        assert!(super::provider_login_origin(
            "codex",
            &"https://auth.openai.com/oauth/authorize".parse().unwrap()
        ));
        assert!(super::provider_login_origin(
            "claude",
            &"https://claude.ai/oauth/authorize".parse().unwrap()
        ));
        for url in [
            "https://auth.openai.com.attacker.example/",
            "http://auth.openai.com/",
            "https://auth.openai.com:444/",
            "https://accounts.google.com/",
        ] {
            assert!(!super::provider_login_origin(
                "codex",
                &url.parse().unwrap()
            ));
        }
        assert!(!super::provider_login_origin(
            "gemini",
            &"https://accounts.google.com/".parse().unwrap()
        ));
    }
    #[test]
    fn github_window_title_accepts_only_device_user_codes() {
        assert!(super::valid_github_user_code("ABCD-1234"));
        for invalid in [
            "",
            "ABCD1234",
            "ABCD-12345",
            "ABCD-12\n4",
            "ABCD-12/4",
            "éBCD-1234",
        ] {
            assert!(!super::valid_github_user_code(invalid));
        }
    }
}
