use super::*;
pub struct CodexCliProvider;
#[async_trait]
impl AIExecutionProvider for CodexCliProvider {
    fn id(&self) -> &str {
        "codex"
    }
    fn name(&self) -> &str {
        "OpenAI Codex"
    }
    fn npm_entry(&self) -> &str {
        "@openai/codex/bin/codex.js"
    }
    fn installation_url(&self) -> &str {
        "https://developers.openai.com/codex/cli"
    }
    fn help_args(&self) -> Vec<&str> {
        vec!["exec", "--help"]
    }
    fn capabilities(&self, h: &str) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_non_interactive: h.contains("codex exec") && h.contains("--sandbox"),
            supports_streaming: h.contains("--json"),
            supports_structured_output: h.contains("--json"),
            supports_resume: h.contains("resume"),
            supports_session_id: h.contains("resume"),
            supports_working_directory: h.contains("--cd"),
            ..Default::default()
        }
    }
    fn execution_args(
        &self,
        c: &ProviderCapabilities,
        session: Option<&str>,
    ) -> Result<Vec<String>> {
        if !c.supports_streaming || !c.supports_non_interactive {
            return Err("CLI_INCOMPATIBLE".into());
        }
        let mut a = vec!["exec".into(), "--sandbox".into(), "workspace-write".into()];
        if let Some(s) = session {
            if !c.supports_resume {
                return Err("CLI resume unsupported".into());
            }
            a.extend(["resume".into(), s.into()]);
        }
        a.extend(["--json".into(), "-".into()]);
        Ok(a)
    }
    fn auth_status_args(&self) -> Option<Vec<&str>> {
        Some(vec!["login", "status"])
    }
    fn login_args(&self) -> Option<Vec<&str>> {
        Some(vec!["login"])
    }
    fn logout_args(&self) -> Option<Vec<&str>> {
        Some(vec!["logout"])
    }
    fn parse_output(&self, line: &str) -> serde_json::Value {
        let mut v = parsed(line);
        if v["type"] == "turn.failed" || v["type"] == "error" {
            v["failed"] = true.into();
        }
        if let Some(t) = text_at(
            &v,
            &[
                "/item/text",
                "/item/aggregated_output",
                "/error/message",
                "/message",
            ],
        ) {
            v["display"] = t.into();
        }
        if let Some(s) = text_at(&v, &["/thread_id"]) {
            v["sessionId"] = s.into();
        }
        v
    }
    fn classify_error(&self, text: &str) -> ErrorKind {
        let t = text.to_lowercase();
        if t.contains("usage limit")
            || t.contains("rate_limit_exceeded")
            || t.contains("quota exceeded")
        {
            ErrorKind::UsageLimit
        } else if t.contains("unauthorized") || t.contains("authentication") || t.contains("401") {
            ErrorKind::AuthFailure
        } else if t.contains("context window") {
            ErrorKind::ContextLimit
        } else if t.contains("approval") || t.contains("permission denied") {
            ErrorKind::PermissionRequired
        } else if t.contains("connection") || t.contains("timed out") {
            ErrorKind::TransientNetworkError
        } else {
            ErrorKind::ProcessError
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn probes_flags_and_parses_quota() {
        let p = CodexCliProvider;
        assert!(!p.capabilities("removed flags").supports_streaming);
        assert!(matches!(
            p.classify_error("usage limit reached"),
            ErrorKind::UsageLimit
        ));
        assert!(p.parse_quota_information("usage limit reached").is_none());
        assert_eq!(
            p.parse_quota_information(r#"{"error":{"retry_after":120}}"#)
                .unwrap()
                .retry_after,
            Some(120)
        );
    }
}
