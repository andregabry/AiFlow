use super::*;
pub struct GeminiCliProvider;
#[async_trait]
impl AIExecutionProvider for GeminiCliProvider {
    fn id(&self) -> &str {
        "gemini"
    }
    fn name(&self) -> &str {
        "Google Gemini"
    }
    fn npm_entry(&self) -> &str {
        "@google/gemini-cli/dist/index.js"
    }
    fn installation_url(&self) -> &str {
        "https://geminicli.com/docs/get-started/installation/"
    }
    fn capabilities(&self, h: &str) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_non_interactive: h.contains("--prompt"),
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
        // Prompt body is piped through stdin; this short instruction activates headless mode.
        let mut a = vec![
            "--prompt".into(),
            "Execute the task provided on stdin according to its policies.".into(),
            "--output-format".into(),
            "stream-json".into(),
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
        None
    }
    fn login_args(&self) -> Option<Vec<&str>> {
        None
    }
    fn logout_args(&self) -> Option<Vec<&str>> {
        None
    }
    fn parse_output(&self, line: &str) -> serde_json::Value {
        let mut v = parsed(line);
        if v["type"] == "error" || (v["type"] == "result" && v["status"] == "error") {
            v["failed"] = true.into();
        }
        if let Some(t) = text_at(&v, &["/content", "/error/message", "/message"]) {
            v["display"] = t.into();
        }
        if let Some(s) = text_at(&v, &["/session_id"]) {
            v["sessionId"] = s.into();
        }
        v
    }
    fn classify_error(&self, text: &str) -> ErrorKind {
        let t = text.to_lowercase();
        if t.contains("resource_exhausted") || t.contains("quota") || t.contains("429") {
            ErrorKind::UsageLimit
        } else if t.contains("unauthenticated")
            || t.contains("authentication")
            || t.contains("invalid_grant")
        {
            ErrorKind::AuthFailure
        } else if t.contains("permission") {
            ErrorKind::PermissionRequired
        } else if t.contains("503") {
            ErrorKind::ProviderUnavailable
        } else if t.contains("network") {
            ErrorKind::TransientNetworkError
        } else {
            ErrorKind::ProcessError
        }
    }
}
