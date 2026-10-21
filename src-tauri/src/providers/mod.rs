pub mod claude;
pub mod codex;
pub mod gemini;

use crate::{git::command, model::*};
use async_trait::async_trait;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::process::Command;

#[derive(Clone)]
pub struct Launch {
    pub program: PathBuf,
    pub prefix: Vec<String>,
}
impl Launch {
    pub fn command(&self) -> Command {
        let mut cmd = command(self.program.to_str().unwrap_or_default());
        cmd.args(&self.prefix);
        #[cfg(unix)]
        cmd.process_group(0);
        cmd
    }
}
#[async_trait]
pub trait AIExecutionProvider: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn installation_url(&self) -> &str;
    fn npm_entry(&self) -> &str;
    fn help_args(&self) -> Vec<&str> {
        vec!["--help"]
    }
    fn capabilities(&self, help: &str) -> ProviderCapabilities;
    fn execution_args(
        &self,
        caps: &ProviderCapabilities,
        session: Option<&str>,
    ) -> Result<Vec<String>>;
    fn auth_status_args(&self) -> Option<Vec<&str>>;
    fn login_args(&self) -> Option<Vec<&str>>;
    fn logout_args(&self) -> Option<Vec<&str>>;
    fn parse_output(&self, line: &str) -> serde_json::Value;
    fn classify_error(&self, text: &str) -> ErrorKind;
    fn parse_quota_information(&self, text: &str) -> Option<ProviderUsage> {
        quota(self.id(), text)
    }
    fn detect(&self) -> Option<Launch> {
        let path = which::which(self.id()).ok()?;
        if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("cmd" | "ps1" | "bat")
        ) {
            let entry = path.parent()?.join("node_modules").join(self.npm_entry());
            if !entry.is_file() {
                return None;
            }
            Some(Launch {
                program: which::which("node").ok()?,
                prefix: vec![entry.to_str()?.into()],
            })
        } else {
            Some(Launch {
                program: path,
                prefix: vec![],
            })
        }
    }
    async fn get_authentication_status(&self, launch: &Launch) -> String {
        let Some(args) = self.auth_status_args() else {
            return "UNKNOWN".into();
        };
        match tokio::time::timeout(
            std::time::Duration::from_secs(15),
            launch.command().args(args).output(),
        )
        .await
        {
            Ok(Ok(o)) if o.status.success() => "CONNECTED".into(),
            Ok(Ok(o)) if o.status.code() == Some(1) => "DISCONNECTED".into(),
            _ => "UNKNOWN".into(),
        }
    }
    async fn health_check(&self) -> ProviderStatus {
        let mut s = ProviderStatus {
            id: self.id().into(),
            name: self.name().into(),
            status: "NOT_INSTALLED".into(),
            version: None,
            authentication: "UNKNOWN".into(),
            capabilities: ProviderCapabilities::default(),
            checked_at: now(),
            diagnostics: "Official CLI was not found on PATH".into(),
            installation_url: self.installation_url().into(),
        };
        let Some(launch) = self.detect() else {
            return s;
        };
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            launch.command().args(self.help_args()).output(),
        )
        .await;
        let Ok(Ok(help)) = output else {
            s.status = "UNAVAILABLE".into();
            s.diagnostics = "CLI help did not complete within 15 seconds".into();
            return s;
        };
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&help.stdout),
            String::from_utf8_lossy(&help.stderr)
        );
        s.capabilities = self.capabilities(&text);
        if let Ok(Ok(v)) = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            launch.command().arg("--version").output(),
        )
        .await
        {
            s.version = Some(crate::security::redact(
                String::from_utf8_lossy(&v.stdout).trim(),
            ));
        }
        s.authentication = self.get_authentication_status(&launch).await;
        s.status = if !help.status.success()
            || !s.capabilities.supports_non_interactive
            || !s.capabilities.supports_streaming
        {
            "INCOMPATIBLE"
        } else if s.authentication == "DISCONNECTED" {
            "AUTH_REQUIRED"
        } else {
            "AVAILABLE"
        }
        .into();
        s.diagnostics=if s.status=="INCOMPATIBLE" { "Installed CLI does not advertise the required noninteractive streaming capabilities" } else if s.authentication=="UNKNOWN" { "Official noninteractive authentication status is unavailable. Authentication is checked by the official CLI at execution time." } else { "CLI capability probe and independent authentication check completed" }.into();
        s
    }
}
pub struct ProviderManager {
    registry: HashMap<String, Arc<dyn AIExecutionProvider>>,
    recent_auth: std::sync::Mutex<HashMap<String, std::time::Instant>>,
}
impl Default for ProviderManager {
    fn default() -> Self {
        let list: Vec<Arc<dyn AIExecutionProvider>> = vec![
            Arc::new(codex::CodexCliProvider),
            Arc::new(claude::ClaudeCodeProvider),
            Arc::new(gemini::GeminiCliProvider),
        ];
        Self {
            recent_auth: Default::default(),
            registry: list.into_iter().map(|p| (p.id().to_string(), p)).collect(),
        }
    }
}
impl ProviderManager {
    #[cfg(test)]
    pub fn fixture(provider: Arc<dyn AIExecutionProvider>) -> Self {
        Self {
            recent_auth: Default::default(),
            registry: HashMap::from([(provider.id().to_string(), provider)]),
        }
    }
    pub fn get(&self, id: &str) -> Result<Arc<dyn AIExecutionProvider>> {
        self.registry
            .get(id)
            .cloned()
            .ok_or_else(|| format!("Unknown provider: {id}"))
    }
    pub fn confirm_auth(&self, id: &str) {
        self.recent_auth
            .lock()
            .unwrap()
            .insert(id.into(), std::time::Instant::now());
    }
    pub async fn scan(&self) -> Vec<ProviderStatus> {
        let mut statuses = vec![];
        let mut ids = self.registry.keys().collect::<Vec<_>>();
        ids.sort();
        for id in ids {
            if let Ok(p) = self.get(id) {
                let mut status = p.health_check().await;
                if id == "gemini"
                    && status.authentication == "UNKNOWN"
                    && self
                        .recent_auth
                        .lock()
                        .unwrap()
                        .get(id)
                        .is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(120))
                {
                    status.authentication = "CONNECTED".into();
                    status.diagnostics="Google account verified by the official Gemini CLI ACP authentication response within the last two minutes.".into();
                }
                statuses.push(status);
            }
        }
        statuses
    }
    pub fn select(
        &self,
        requested: &str,
        policy: &ProjectPolicy,
    ) -> Result<Arc<dyn AIExecutionProvider>> {
        let selected = if requested == "auto" {
            &policy.default_provider
        } else {
            requested
        };
        if !policy.allowed_providers.iter().any(|p| p == selected) {
            return Err("POLICY_VIOLATION: provider is not allowed for this project".into());
        }
        self.get(selected)
    }
}
pub fn parsed(line: &str) -> serde_json::Value {
    serde_json::from_str(line).unwrap_or_else(|_| serde_json::json!({"type":"text","text":line}))
}
pub fn text_at(v: &serde_json::Value, paths: &[&str]) -> Option<String> {
    paths
        .iter()
        .find_map(|p| v.pointer(p).and_then(|x| x.as_str()).map(str::to_owned))
}
pub fn quota(provider: &str, text: &str) -> Option<ProviderUsage> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    // Only structured, provider-reported values; never infer a reset from local clock or prose.
    let error = value.get("error").unwrap_or(&value);
    let reset = error
        .get("reset_at")
        .and_then(|v| v.as_str())
        .filter(|s| chrono::DateTime::parse_from_rfc3339(s).is_ok())
        .map(str::to_owned);
    let retry = error
        .get("retry_after")
        .and_then(|v| v.as_i64())
        .filter(|v| *v > 0 && *v < 604800);
    if reset.is_none() && retry.is_none() {
        return None;
    }
    Some(ProviderUsage {
        provider_id: provider.into(),
        detected_at: now(),
        reset_at: reset,
        retry_after: retry,
        source: "structured CLI output".into(),
        confidence: "provider-reported".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_provider_normalizes_limits_independently() {
        let providers: Vec<(Box<dyn AIExecutionProvider>, &str)> = vec![
            (Box::new(codex::CodexCliProvider), "usage limit reached"),
            (Box::new(claude::ClaudeCodeProvider), "hit your limit"),
            (
                Box::new(gemini::GeminiCliProvider),
                "RESOURCE_EXHAUSTED: quota",
            ),
        ];
        for (p, fixture) in providers {
            assert!(matches!(p.classify_error(fixture), ErrorKind::UsageLimit));
            assert!(p.parse_quota_information(fixture).is_none());
            let usage = p
                .parse_quota_information(
                    r#"{"error":{"reset_at":"2026-10-22T18:00:00Z","retry_after":60}}"#,
                )
                .unwrap();
            assert_eq!(usage.provider_id, p.id());
            assert_eq!(usage.reset_at.as_deref(), Some("2026-10-22T18:00:00Z"));
            assert!(p
                .parse_quota_information(r#"{"reset_at":"tomorrow","retry_after":-1}"#)
                .is_none());
            assert_eq!(
                p.parse_output("malformed provider output")["text"],
                "malformed provider output"
            );
        }
    }
    #[test]
    fn auto_routing_obeys_allowlist_and_concrete_default() {
        let manager = ProviderManager::default();
        let mut policy = ProjectPolicy::default();
        assert_eq!(manager.select("auto", &policy).unwrap().id(), "codex");
        policy.allowed_providers = vec!["claude".into()];
        assert!(manager.select("auto", &policy).is_err());
        assert!(manager.select("gemini", &policy).is_err());
        assert_eq!(manager.select("claude", &policy).unwrap().id(), "claude");
    }
}
