use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, String>;
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub repository: String,
    pub path: String,
    pub branch: String,
    pub policy: ProjectPolicy,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectPolicy {
    pub allowed_providers: Vec<String>,
    pub default_provider: String,
    pub failover: String,
    pub require_clean_tree: bool,
    pub require_commit: bool,
    pub require_tests: bool,
    pub require_ci: bool,
    pub close_issue: bool,
    pub manual_approval_labels: Vec<String>,
    pub test_commands: Vec<Vec<String>>,
    pub max_retries: u32,
}
impl Default for ProjectPolicy {
    fn default() -> Self {
        Self {
            allowed_providers: vec!["codex".into(), "claude".into(), "gemini".into()],
            default_provider: "codex".into(),
            failover: "ask".into(),
            require_clean_tree: true,
            require_commit: true,
            require_tests: true,
            require_ci: false,
            close_issue: false,
            manual_approval_labels: vec![
                "production".into(),
                "deployment".into(),
                "manual-approval".into(),
            ],
            test_commands: vec![],
            max_retries: 3,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub milestone: Option<String>,
    pub url: String,
    pub updated_at: String,
    pub comments: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub id: String,
    pub project_id: String,
    pub issue_number: u64,
    pub provider_id: String,
    pub position: i64,
    pub state: String,
    pub approved: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    pub id: String,
    pub item_id: String,
    pub project_id: String,
    pub provider_id: String,
    pub state: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub reason: Option<String>,
    pub before_git: Option<GitSnapshot>,
    pub after_git: Option<GitSnapshot>,
    pub retry_at: Option<String>,
    pub attempt: u32,
    pub session_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GitSnapshot {
    pub branch: String,
    pub head: String,
    pub status: String,
    pub root: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionEvent {
    pub event_id: String,
    pub execution_id: String,
    pub attempt_id: String,
    pub sequence: i64,
    pub timestamp: String,
    pub r#type: String,
    pub payload: serde_json::Value,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub supports_resume: bool,
    pub supports_structured_output: bool,
    pub supports_streaming: bool,
    pub supports_usage_information: bool,
    pub supports_session_id: bool,
    pub supports_non_interactive: bool,
    pub supports_working_directory: bool,
    pub supports_auto_approval: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub id: String,
    pub name: String,
    pub status: String,
    pub version: Option<String>,
    pub authentication: String,
    pub capabilities: ProviderCapabilities,
    pub checked_at: String,
    pub diagnostics: String,
    pub installation_url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsage {
    pub provider_id: String,
    pub detected_at: String,
    pub reset_at: Option<String>,
    pub retry_after: Option<i64>,
    pub source: String,
    pub confidence: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorKind {
    UsageLimit,
    AuthFailure,
    TransientProviderError,
    TransientNetworkError,
    ProviderUnavailable,
    CliIncompatible,
    ProcessError,
    ContextLimit,
    PermissionRequired,
    Unknown,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub projects: Vec<Project>,
    pub issues: Vec<ProjectIssues>,
    pub queue: Vec<QueueItem>,
    pub executions: Vec<Execution>,
    pub providers: Vec<ProviderStatus>,
    pub usage: Vec<ProviderUsage>,
    pub running_projects: Vec<String>,
    pub github_account: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIssues {
    pub project_id: String,
    pub issues: Vec<Issue>,
}
