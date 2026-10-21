use regex::Regex;
use std::sync::LazyLock;

static SECRETS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
    Regex::new(r"(?i)(bearer\s+)[A-Za-z0-9._~+/=-]+").unwrap(),
    Regex::new(r"(?:gh[pousr]_[A-Za-z0-9_]+|github_pat_[A-Za-z0-9_]+|sk-[A-Za-z0-9_-]{12,})").unwrap(),
    Regex::new(r#"(?i)((?:api[_-]?key|access[_-]?token|refresh[_-]?token|session[_-]?token|password|cookie|authorization)[\s"']*[:=][\s"']*)[^\s,}"']+"#).unwrap(),
]
});
pub fn redact(value: &str) -> String {
    let mut out = value.to_string();
    for rule in SECRETS.iter() {
        out = rule.replace_all(&out, "[REDACTED]").into_owned();
    }
    // Never expose a PEM payload, even in the raw view.
    if out.contains("PRIVATE KEY") {
        return "[REDACTED PRIVATE KEY OUTPUT]".into();
    }
    out
}
pub fn safe_repository(value: &str) -> bool {
    let parts: Vec<_> = value.split('/').collect();
    parts.len() == 2
        && parts.iter().all(|s| {
            !s.is_empty()
                && !s.starts_with('.')
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        })
}

pub fn valid_session_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() < 200
        && value
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-".contains(c))
}
#[derive(Default)]
pub struct StreamRedactor {
    private_key: bool,
}
impl StreamRedactor {
    pub fn apply(&mut self, value: &str) -> String {
        if value.contains("-----BEGIN") && value.contains("PRIVATE KEY") {
            self.private_key = true;
        }
        if self.private_key {
            if value.contains("-----END") && value.contains("PRIVATE KEY") {
                self.private_key = false;
            }
            return "[REDACTED PRIVATE KEY OUTPUT]\n".into();
        }
        redact(value)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multiline_private_keys_and_session_option_injection() {
        let mut redactor = StreamRedactor::default();
        for line in [
            "-----BEGIN PRIVATE KEY-----\n",
            "topsecretbase64\n",
            "-----END PRIVATE KEY-----\n",
        ] {
            assert!(redactor.apply(line).contains("REDACTED"));
        }
        assert_eq!(redactor.apply("normal output"), "normal output");
        assert!(!valid_session_id("--dangerously-bypass"));
        assert!(valid_session_id("550e8400-e29b-41d4-a716-446655440000"));
    }
    #[test]
    fn secrets_are_redacted() {
        for s in [
            "Bearer abc.def.ghi",
            "ghp_1234567890secret",
            "password=secret",
            "access_token: secret",
            "sk-1234567890secret",
        ] {
            assert!(!redact(s).contains("secret"));
        }
    }
    #[test]
    fn repository_is_not_a_url_or_option() {
        assert!(safe_repository("owner/repo"));
        for s in [
            "--help",
            "../repo",
            "https://github.com/a/b",
            "a/b/c",
            "a/b?x=y",
        ] {
            assert!(!safe_repository(s));
        }
    }
}
