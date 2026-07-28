use serde::{Deserialize, Serialize};

/// Reason why a reply finished.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyFinishedReason {
    Completed,
    Interrupted,
    ExceedMaxIters,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reply_finished_reason_serialization() {
        assert_eq!(
            serde_json::to_string(&ReplyFinishedReason::Completed).unwrap(),
            r#""completed""#
        );
        assert_eq!(
            serde_json::to_string(&ReplyFinishedReason::Interrupted).unwrap(),
            r#""interrupted""#
        );
        assert_eq!(
            serde_json::to_string(&ReplyFinishedReason::ExceedMaxIters).unwrap(),
            r#""exceed_max_iters""#
        );
        assert_eq!(
            serde_json::to_string(&ReplyFinishedReason::Error).unwrap(),
            r#""error""#
        );
    }

    #[test]
    fn test_reply_finished_reason_roundtrip() {
        let variants = vec![
            ReplyFinishedReason::Completed,
            ReplyFinishedReason::Interrupted,
            ReplyFinishedReason::ExceedMaxIters,
            ReplyFinishedReason::Error,
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let restored: ReplyFinishedReason = serde_json::from_str(&json).unwrap();
            assert_eq!(v, restored);
        }
    }
}
