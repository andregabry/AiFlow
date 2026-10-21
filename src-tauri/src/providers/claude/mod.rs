use super::*;
pub struct ClaudeCodeProvider;
#[async_trait]
impl AIExecutionProvider for ClaudeCodeProvider {
    fn id(&self) -> &str {
        "claude"
    }
    fn name(&self) -> &str {
        "Anthropic Claude"
    }
    fn npm_entry(&self) -> &str {
        "@anthropic-ai/claude-code/cli.js"
    }
    fn installation_url(&self) -> &str {
        "https://code.claude.com/docs/en/setup"
    }
    fn capabilities(&self, h: &str) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_non_interactive: h.contains("--print") && h.contains("--verbose"),
            supports_streaming: h.contains("--output-format") && h.contains("stream-json"),
            supports_structured_output: h.contains("--output-format"),
            supports_resume: h.contains("--resume"),
            supports_session_id: h.contains("--resume"),
            supports_working_directory: true,
            ..Default::default()
        }
    }
    fn execution_args(
        &self,
        c: &ProviderCapabilities,
        session: Option<&str>,
    ) -> Result<Vec<String>> {
        if !c.supports_non_interactive || !c.supports_streaming {
            return Err("CLI_INCOMPATIBLE".into());
        }
        let mut a = vec![
            "--print".into(),
            "--output-format".into(),
            "stream-json".into(),
            "--verbose".into(),
        ];
        if let Some(s) = session {
            if !c.supports_resume {
                return Err("CLI resume unsupported".into());
            }
            a.extend(["--resume".into(), s.into()]);
        }
        Ok(a)
    }
    fn auth_status_args(&self) -> Option<Vec<&str>> {
        Some(vec!["auth", "status"])
    }
    fn login_args(&self) -> Option<Vec<&str>> {
        Some(vec!["auth", "login"])
    }
    fn logout_args(&self) -> Option<Vec<&str>> {
        Some(vec!["auth", "logout"])
    }
    fn parse_output(&self, line: &str) -> serde_json::Value {
        let mut v = parsed(line);
        if v["is_error"] == true {
            v["failed"] = true.into();
        }
        if let Some(t) = text_at(
            &v,
            &["/message/content/0/text", "/result", "/error/message"],
        ) {
            v["display"] = t.into();
        }
        if let Some(s) = text_at(&v, &["/session_id"]) {
            v["sessionId"] = s.into();
        }
        v
    }
    fn classify_error(&self, text: &str) -> ErrorKind {
        let t = text.to_lowercase();
        if t.contains("rate_limit") || t.contains("usage limit") || t.contains("hit your limit") {
            ErrorKind::UsageLimit
        } else if t.contains("authentication")
            || t.contains("invalid_api_key")
            || t.contains("not logged in")
        {
            ErrorKind::AuthFailure
        } else if t.contains("permission") {
            ErrorKind::PermissionRequired
        } else if t.contains("overloaded") {
            ErrorKind::ProviderUnavailable
        } else if t.contains("connection") {
            ErrorKind::TransientNetworkError
        } else {
            ErrorKind::ProcessError
        }
    }
}
