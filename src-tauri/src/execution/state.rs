pub fn allowed(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("QUEUED", "PREPARING")
            | (
                "PREPARING",
                "RUNNING"
                    | "BLOCKED"
                    | "HUMAN_REVIEW"
                    | "WAITING_FOR_PROVIDER"
                    | "FAILED"
                    | "CANCELLED"
            )
            | (
                "RUNNING",
                "VALIDATING"
                    | "WAITING_FOR_PROVIDER"
                    | "WAITING_FOR_QUOTA"
                    | "FAILED"
                    | "HUMAN_REVIEW"
                    | "CANCELLED"
                    | "RETRY_WAIT"
            )
            | (
                "VALIDATING",
                "COMMITTING" | "SUCCESS" | "HUMAN_REVIEW" | "FAILED" | "CANCELLED"
            )
            | ("COMMITTING", "SUCCESS" | "HUMAN_REVIEW" | "FAILED")
            | (
                "WAITING_FOR_PROVIDER"
                    | "WAITING_FOR_QUOTA"
                    | "RETRY_WAIT"
                    | "PAUSED"
                    | "FAILED"
                    | "BLOCKED"
                    | "HUMAN_REVIEW"
                    | "CANCELLED",
                "QUEUED"
            )
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn success_requires_validation() {
        assert!(!allowed("RUNNING", "SUCCESS"));
        assert!(allowed("RUNNING", "VALIDATING"));
        assert!(allowed("VALIDATING", "SUCCESS"));
    }
    #[test]
    fn waiting_and_retry_are_explicit() {
        for s in [
            "WAITING_FOR_QUOTA",
            "WAITING_FOR_PROVIDER",
            "FAILED",
            "BLOCKED",
        ] {
            assert!(allowed("RUNNING", s) || s == "BLOCKED");
            assert!(allowed(s, "QUEUED"));
        }
        assert!(!allowed("SUCCESS", "RUNNING"));
    }
}
