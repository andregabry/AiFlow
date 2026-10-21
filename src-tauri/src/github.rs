use crate::{model::*, persistence::Store};
use reqwest::{header, Client};
use serde_json::{json, Value};
use std::sync::Mutex;

pub struct GitHubService {
    client: Client,
    cli_marker: Option<std::path::PathBuf>,
    pending: Mutex<Option<DeviceCode>>,
    account_cache: Mutex<Option<(std::time::Instant, Option<String>)>>,
    #[cfg(test)]
    fixture_comments: bool,
}
struct DeviceCode {
    code: String,
    client_id: String,
    expires: chrono::DateTime<chrono::Utc>,
    next_poll: chrono::DateTime<chrono::Utc>,
    interval: i64,
}
impl Default for GitHubService {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .user_agent("AiFlow/0.1")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("HTTP client"),
            cli_marker: None,
            pending: Mutex::new(None),
            account_cache: Mutex::new(None),
            #[cfg(test)]
            fixture_comments: false,
        }
    }
}
impl GitHubService {
    pub fn new(marker: std::path::PathBuf) -> Self {
        Self {
            cli_marker: Some(marker),
            ..Self::default()
        }
    }
    fn uses_cli(&self) -> bool {
        self.cli_marker.as_ref().is_some_and(|p| p.is_file())
    }
    pub fn cli_launch() -> Result<crate::providers::Launch> {
        let program = which::which("gh").ok();
        #[cfg(windows)]
        let program = program.or_else(|| {
            std::env::var_os("ProgramFiles")
                .map(std::path::PathBuf::from)
                .map(|p| p.join("GitHub CLI/gh.exe"))
                .filter(|p| p.is_file())
        });
        Ok(crate::providers::Launch {
            program: program.ok_or(
                "Install the official GitHub CLI from https://cli.github.com, then try again.",
            )?,
            prefix: vec![],
        })
    }
    async fn cli_api(path: &str, close: bool) -> Result<Value> {
        let mut command = Self::cli_launch()?.command();
        command.args([
            "api",
            "--hostname",
            "github.com",
            "--method",
            if close { "PATCH" } else { "GET" },
            "--header",
            "X-GitHub-Api-Version: 2022-11-28",
        ]);
        if close {
            command.args(["--raw-field", "state=closed"]);
        }
        command
            .arg(path)
            .env("GH_PROMPT_DISABLED", "1")
            .env_remove("GH_DEBUG");
        let output = tokio::time::timeout(std::time::Duration::from_secs(30), command.output())
            .await
            .map_err(|_| "GitHub CLI request timed out")?
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "GitHub CLI: {}",
                crate::security::redact(&String::from_utf8_lossy(&output.stderr))
            ));
        }
        serde_json::from_slice(&output.stdout)
            .map_err(|_| "GitHub CLI returned an invalid API response".into())
    }
    pub async fn connect_cli(&self, cancel: &std::sync::atomic::AtomicBool) -> Result<()> {
        let user = Self::cli_api("/user", false).await?;
        let login = user["login"]
            .as_str()
            .ok_or("GitHub did not confirm the signed-in account")?;
        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("GitHub connection cancelled".into());
        }
        if let Ok(credential) = Self::credential() {
            let _ = credential.delete_credential();
        }
        std::fs::write(
            self.cli_marker
                .as_ref()
                .ok_or("Missing account configuration path")?,
            b"official-github-cli",
        )
        .map_err(|e| e.to_string())?;
        *self.account_cache.lock().map_err(|e| e.to_string())? =
            Some((std::time::Instant::now(), Some(login.into())));
        Ok(())
    }
    #[cfg(test)]
    pub fn fixture() -> Self {
        Self {
            fixture_comments: true,
            ..Default::default()
        }
    }
    fn credential() -> Result<keyring::Entry> {
        keyring::Entry::new("AiFlow", "github.com").map_err(|e| e.to_string())
    }
    fn token() -> Result<String> {
        Self::credential()?
            .get_password()
            .map_err(|_| "GitHub is not connected. Connect with Device Flow in Accounts.".into())
    }
    pub async fn device_start(&self, client_id: String) -> Result<Value> {
        if client_id.len() < 5 || !client_id.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(
                "Enter the public client ID of a GitHub OAuth App with Device Flow enabled".into(),
            );
        }
        let response = self
            .client
            .post("https://github.com/login/device/code")
            .header(header::ACCEPT, "application/json")
            .form(&[
                ("client_id", client_id.as_str()),
                ("scope", "repo read:org"),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let value: Value = response
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let code = value["device_code"]
            .as_str()
            .ok_or("GitHub did not issue a device code. Check OAuth App settings.")?;
        let interval = value["interval"].as_i64().unwrap_or(5).max(5);
        *self.pending.lock().map_err(|e| e.to_string())? = Some(DeviceCode {
            code: code.into(),
            client_id,
            expires: chrono::Utc::now()
                + chrono::Duration::seconds(value["expires_in"].as_i64().unwrap_or(900)),
            next_poll: chrono::Utc::now() + chrono::Duration::seconds(interval),
            interval,
        });
        Ok(
            json!({"userCode":value["user_code"],"verificationUri":value["verification_uri"],"interval":interval,"expiresIn":value["expires_in"]}),
        )
    }
    pub async fn device_poll(&self) -> Result<Value> {
        let (code, client) = {
            let mut pending = self.pending.lock().map_err(|e| e.to_string())?;
            let p = pending.as_mut().ok_or("Start GitHub Device Flow first")?;
            if chrono::Utc::now() > p.expires {
                *pending = None;
                return Err("GitHub device code expired".into());
            }
            if chrono::Utc::now() < p.next_poll {
                return Ok(json!({"status":"authorization_pending"}));
            }
            p.next_poll = chrono::Utc::now() + chrono::Duration::seconds(p.interval);
            (p.code.clone(), p.client_id.clone())
        };
        let v: Value = self
            .client
            .post("https://github.com/login/oauth/access_token")
            .header(header::ACCEPT, "application/json")
            .form(&[
                ("client_id", client),
                ("device_code", code.clone()),
                (
                    "grant_type",
                    "urn:ietf:params:oauth:grant-type:device_code".into(),
                ),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let mut pending = self.pending.lock().map_err(|e| e.to_string())?;
        if pending.as_ref().map(|p| p.code.as_str()) != Some(code.as_str()) {
            return Err("GitHub connection cancelled".into());
        }
        if let Some(token) = v["access_token"].as_str() {
            Self::credential()?
                .set_password(token)
                .map_err(|e| e.to_string())?;
            if let Some(marker) = &self.cli_marker {
                let _ = std::fs::remove_file(marker);
            }
            *self.account_cache.lock().map_err(|e| e.to_string())? = None;
            *pending = None;
            return Ok(json!({"status":"connected"}));
        }
        let error = v["error"].as_str().unwrap_or("unknown_error");
        if error == "slow_down" {
            if let Some(p) = pending.as_mut() {
                p.interval += 5;
                p.next_poll = chrono::Utc::now() + chrono::Duration::seconds(p.interval);
            }
        }
        if !matches!(error, "authorization_pending" | "slow_down") {
            *pending = None;
            return Err(format!("GitHub Device Flow: {error}"));
        }
        Ok(json!({"status":error}))
    }
    pub fn logout(&self) -> Result<()> {
        *self.account_cache.lock().map_err(|e| e.to_string())? = None;
        if self.uses_cli() {
            std::fs::remove_file(self.cli_marker.as_ref().unwrap()).map_err(|e| e.to_string())?;
            return Ok(());
        }
        Self::credential()?
            .delete_credential()
            .map_err(|e| e.to_string())
    }
    pub fn device_cancel(&self) -> Result<()> {
        *self.pending.lock().map_err(|e| e.to_string())? = None;
        Ok(())
    }
    pub async fn account(&self) -> Option<String> {
        if let Some((at, account)) = self.account_cache.lock().ok()?.as_ref() {
            if at.elapsed() < std::time::Duration::from_secs(120) {
                return account.clone();
            }
        }
        let account = self
            .get("/user")
            .await
            .ok()
            .and_then(|v| v["login"].as_str().map(str::to_owned));
        if let Ok(mut cache) = self.account_cache.lock() {
            *cache = Some((std::time::Instant::now(), account.clone()));
        }
        account
    }
    async fn get(&self, path: &str) -> Result<Value> {
        if self.uses_cli() {
            return Self::cli_api(path, false).await;
        }
        self.client
            .get(format!("https://api.github.com{path}"))
            .bearer_auth(Self::token()?)
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| format!("GITHUB_ERROR: {e}"))?
            .json()
            .await
            .map_err(|e| e.to_string())
    }
    pub async fn repositories(&self) -> Result<Value> {
        let mut list = vec![];
        for page in 1..=100 {
            let v = self
                .get(&format!(
                    "/user/repos?per_page=100&sort=updated&page={page}"
                ))
                .await?;
            let rows = v.as_array().ok_or("Invalid GitHub repository response")?;
            list.extend(rows.iter().map(|r|json!({"fullName":r["full_name"],"private":r["private"],"defaultBranch":r["default_branch"]})));
            if rows.len() < 100 {
                break;
            }
        }
        Ok(list.into())
    }
    pub async fn sync(&self, store: &Store, project: &Project) -> Result<usize> {
        if !crate::security::safe_repository(&project.repository) {
            return Err("Invalid repository name".into());
        }
        let mut issues = vec![];
        for page in 1..=100 {
            let url = format!(
                "https://api.github.com/repos/{}/issues?state=all&per_page=100&page={page}",
                project.repository
            );
            let cached = store
                .query(
                    "SELECT etag,data FROM github_cache WHERE url=?",
                    [&url],
                    |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, String>(1)?)),
                )?
                .pop();
            let cli = self.uses_cli();
            let mut req = self
                .client
                .get(&url)
                .header("X-GitHub-Api-Version", "2022-11-28");
            if !cli {
                req = req.bearer_auth(Self::token()?);
            }
            if let Some((Some(tag), _)) = &cached {
                req = req.header(header::IF_NONE_MATCH, tag);
            }
            let rows: Vec<Value> = if cli {
                serde_json::from_value(
                    Self::cli_api(url.trim_start_matches("https://api.github.com"), false).await?,
                )
                .map_err(|e| e.to_string())?
            } else {
                let response = req.send().await.map_err(|e| e.to_string())?;
                if response.status() == 304 {
                    serde_json::from_str(&cached.ok_or("Missing ETag cache")?.1)
                        .map_err(|e| e.to_string())?
                } else {
                    let response = response
                        .error_for_status()
                        .map_err(|e| format!("GITHUB_ERROR: {e}"))?;
                    let tag = response
                        .headers()
                        .get(header::ETAG)
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_owned);
                    let data = response
                        .json::<Vec<Value>>()
                        .await
                        .map_err(|e| e.to_string())?;
                    store.sql(
                        "INSERT OR REPLACE INTO github_cache VALUES(?,?,?)",
                        rusqlite::params![url, tag, serde_json::to_string(&data).unwrap()],
                    )?;
                    data
                }
            };
            let count = rows.len();
            for r in rows {
                if r.get("pull_request").is_some() {
                    continue;
                }
                issues.push(Issue {
                    number: r["number"].as_u64().ok_or("Missing issue number")?,
                    title: r["title"].as_str().unwrap_or_default().into(),
                    body: r["body"].as_str().unwrap_or_default().into(),
                    state: r["state"].as_str().unwrap_or("open").to_uppercase(),
                    labels: r["labels"]
                        .as_array()
                        .map(|v| {
                            v.iter()
                                .filter_map(|x| x["name"].as_str().map(str::to_owned))
                                .collect()
                        })
                        .unwrap_or_default(),
                    assignees: r["assignees"]
                        .as_array()
                        .map(|v| {
                            v.iter()
                                .filter_map(|x| x["login"].as_str().map(str::to_owned))
                                .collect()
                        })
                        .unwrap_or_default(),
                    milestone: r["milestone"]["title"].as_str().map(str::to_owned),
                    url: r["html_url"].as_str().unwrap_or_default().into(),
                    updated_at: r["updated_at"].as_str().unwrap_or_default().into(),
                    comments: vec![],
                });
            }
            if count < 100 {
                break;
            }
        }
        // Upsert the remote cache only. Operational task state and attempt history are independent.
        for i in &issues {
            store.sql(
                "INSERT OR REPLACE INTO issues_cache VALUES(?,?,?)",
                rusqlite::params![
                    project.id,
                    i.number as i64,
                    serde_json::to_string(i).unwrap()
                ],
            )?;
        }
        Ok(issues.len())
    }
    pub async fn comments(&self, p: &Project, number: u64) -> Result<Vec<String>> {
        #[cfg(test)]
        if self.fixture_comments {
            return Ok(vec!["Fixture comment; untrusted task content".into()]);
        }
        let mut out = vec![];
        for page in 1..=100 {
            let v = self
                .get(&format!(
                    "/repos/{}/issues/{number}/comments?per_page=100&page={page}",
                    p.repository
                ))
                .await?;
            let rows = v.as_array().ok_or("Invalid comments response")?;
            out.extend(
                rows.iter()
                    .filter_map(|r| r["body"].as_str().map(str::to_owned)),
            );
            if rows.len() < 100 {
                break;
            }
        }
        Ok(out)
    }
    pub async fn refresh_issue(&self, store: &Store, p: &Project, number: u64) -> Result<Issue> {
        #[cfg(test)]
        if self.fixture_comments {
            return store
                .issues(&p.id)?
                .into_iter()
                .find(|i| i.number == number)
                .ok_or("Missing fixture issue".into());
        }
        let r = self
            .get(&format!("/repos/{}/issues/{number}", p.repository))
            .await?;
        let issue = Issue {
            number: r["number"].as_u64().ok_or("Missing issue number")?,
            title: r["title"].as_str().unwrap_or_default().into(),
            body: r["body"].as_str().unwrap_or_default().into(),
            state: r["state"].as_str().unwrap_or("open").to_uppercase(),
            labels: r["labels"]
                .as_array()
                .map(|v| {
                    v.iter()
                        .filter_map(|x| x["name"].as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            assignees: r["assignees"]
                .as_array()
                .map(|v| {
                    v.iter()
                        .filter_map(|x| x["login"].as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            milestone: r["milestone"]["title"].as_str().map(str::to_owned),
            url: r["html_url"].as_str().unwrap_or_default().into(),
            updated_at: r["updated_at"].as_str().unwrap_or_default().into(),
            comments: vec![],
        };
        store.sql(
            "INSERT OR REPLACE INTO issues_cache VALUES(?,?,?)",
            rusqlite::params![
                p.id,
                issue.number as i64,
                serde_json::to_string(&issue).unwrap()
            ],
        )?;
        Ok(issue)
    }
    pub async fn close(&self, p: &Project, number: u64) -> Result<()> {
        if self.uses_cli() {
            Self::cli_api(&format!("/repos/{}/issues/{number}", p.repository), true).await?;
            return Ok(());
        }
        self.client
            .patch(format!(
                "https://api.github.com/repos/{}/issues/{number}",
                p.repository
            ))
            .bearer_auth(Self::token()?)
            .json(&json!({"state":"closed"}))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| format!("GITHUB_ERROR: {e}"))?;
        Ok(())
    }
    pub async fn ci_passed(&self, p: &Project, head: &str) -> Result<bool> {
        let v = self
            .get(&format!(
                "/repos/{}/commits/{head}/check-runs?per_page=100",
                p.repository
            ))
            .await?;
        let rows = v["check_runs"]
            .as_array()
            .ok_or("CI check-runs unavailable")?;
        Ok(!rows.is_empty()
            && v["total_count"].as_u64().unwrap_or(0) <= 100
            && rows.iter().all(|r| {
                r["status"] == "completed"
                    && matches!(
                        r["conclusion"].as_str(),
                        Some("success" | "skipped" | "neutral")
                    )
            }))
    }
}
