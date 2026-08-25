//! Shared helpers for command-like tools.

/// Timeout unit accepted by a command-facing API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutUnit {
    Milliseconds,
    Seconds,
}

/// Parsed timeout ready for backend execution and user-facing diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandTimeout {
    pub seconds: f64,
    pub display: String,
}

/// Clamp a user-supplied timeout into a bounded command timeout.
///
/// `default_ms` and `max_ms` are the canonical millisecond values. When
/// `unit` is [`TimeoutUnit::Seconds`], the input/default/max are interpreted
/// and displayed in seconds while the returned backend value remains seconds.
#[must_use]
pub fn command_timeout(
    input: Option<i64>,
    unit: TimeoutUnit,
    default_ms: i64,
    max_ms: i64,
) -> CommandTimeout {
    match unit {
        TimeoutUnit::Milliseconds => {
            let timeout_ms = input.unwrap_or(default_ms).clamp(0, max_ms);
            CommandTimeout {
                seconds: timeout_ms as f64 / 1000.0,
                display: format!("{timeout_ms}ms"),
            }
        }
        TimeoutUnit::Seconds => {
            let default_secs = default_ms / 1000;
            let max_secs = max_ms / 1000;
            let timeout_secs = input.unwrap_or(default_secs).clamp(0, max_secs);
            CommandTimeout {
                seconds: timeout_secs as f64,
                display: format!("{timeout_secs}s"),
            }
        }
    }
}

/// Decode stdout/stderr as UTF-8 (lossy), normalize CRLF, merge stderr after
/// stdout, and truncate to `max_chars`.
#[must_use]
pub fn format_command_output(stdout: &[u8], stderr: &[u8], max_chars: usize) -> String {
    let stdout = String::from_utf8_lossy(stdout).replace("\r\n", "\n");
    let stderr = String::from_utf8_lossy(stderr).replace("\r\n", "\n");
    let mut output = stdout;
    if !stderr.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&stderr);
    }
    truncate_chars(output, max_chars, "\n... (output truncated)")
}

/// Truncate `text` to `max_chars` Unicode scalar values and append `marker`.
#[must_use]
pub fn truncate_chars(text: String, max_chars: usize, marker: &str) -> String {
    if text.chars().count() > max_chars {
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{truncated}{marker}")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_timeout_supports_milliseconds_and_seconds() {
        assert_eq!(
            command_timeout(None, TimeoutUnit::Milliseconds, 120_000, 600_000),
            CommandTimeout {
                seconds: 120.0,
                display: "120000ms".into()
            }
        );
        assert_eq!(
            command_timeout(Some(900), TimeoutUnit::Seconds, 120_000, 600_000),
            CommandTimeout {
                seconds: 600.0,
                display: "600s".into()
            }
        );
    }

    #[test]
    fn format_command_output_merges_and_truncates() {
        assert_eq!(
            format_command_output(b"a\r\nb", b"err", 10),
            "a\nb\nerr".to_string()
        );
        assert_eq!(
            format_command_output("abcdef".as_bytes(), b"", 3),
            "abc\n... (output truncated)".to_string()
        );
    }
}
