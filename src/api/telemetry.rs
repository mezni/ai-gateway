//! Rendering of chat telemetry signals into structured log lines.
//!
//! The API layer owns the transport surface, so it also owns the act of
//! rendering the application-layer signal into a log line. Rendering is a
//! pure function so it is directly testable; the thin `emit` wrapper performs
//! the actual write to standard output.
//!
//! The rendered line is a single JSON object and carries exactly the signal's
//! fields — provider, model, latency, status, error type, attempt, request
//! count. It never contains prompt content, message bodies, or credentials.

use crate::application::telemetry::ChatRequestSignal;

/// The event name every chat-request signal renders under.
pub const CHAT_REQUEST_EVENT: &str = "chat_request";

/// Renders a signal as a single JSON object, without a trailing newline.
///
/// Pure and total: every signal renders to a valid JSON object with exactly
/// the documented keys. `error_type` is `null` on a successful request.
pub fn chat_signal_line(signal: &ChatRequestSignal) -> String {
    serde_json::json!({
        "event": CHAT_REQUEST_EVENT,
        "provider": signal.provider,
        "model": signal.model,
        "latency_ms": signal.latency_ms,
        "status": signal.status,
        "error_type": signal.error_type,
        "attempt": signal.attempt,
        "request_count": signal.request_count,
    })
    .to_string()
}

/// Writes a signal to standard output as a single JSON line.
pub fn emit_chat_signal(signal: &ChatRequestSignal) {
    println!("{}", chat_signal_line(signal));
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    fn signal(status: &'static str, error_type: Option<&'static str>) -> ChatRequestSignal {
        ChatRequestSignal {
            provider: "openrouter".to_owned(),
            model: "openai/gpt-4o".to_owned(),
            latency_ms: 4321,
            status,
            error_type,
            attempt: 1,
            request_count: 7,
        }
    }

    #[test]
    fn a_success_line_has_exactly_the_documented_keys() {
        let value: Value = serde_json::from_str(&chat_signal_line(&signal("ok", None))).unwrap();
        let object = value.as_object().unwrap();

        let expected: [&str; 8] = [
            "event",
            "provider",
            "model",
            "latency_ms",
            "status",
            "error_type",
            "attempt",
            "request_count",
        ];
        assert_eq!(object.len(), expected.len());
        for key in expected {
            assert!(object.contains_key(key), "missing {key}");
        }
        assert_eq!(object["event"], "chat_request");
        assert_eq!(object["provider"], "openrouter");
        assert_eq!(object["model"], "openai/gpt-4o");
        assert_eq!(object["latency_ms"], 4321);
        assert_eq!(object["status"], "ok");
        assert!(object["error_type"].is_null());
        assert_eq!(object["attempt"], 1);
        assert_eq!(object["request_count"], 7);
    }

    #[test]
    fn an_error_line_carries_the_error_type() {
        let value: Value =
            serde_json::from_str(&chat_signal_line(&signal("error", Some("refused")))).unwrap();

        assert_eq!(value["status"], "error");
        assert_eq!(value["error_type"], "refused");
    }

    #[test]
    fn the_renderer_has_no_field_of_its_own_beyond_the_signal() {
        // The renderer copies the signal's fields verbatim and adds only the
        // fixed event name: provider, model, latency, status, error type,
        // attempt, request count. Prompt content and credentials have no path
        // into the line because no field of either struct can hold them.
        let line = chat_signal_line(&signal("ok", None));

        for needle in ["SENTINEL_PROMPT", "SENTINEL_KEY", "Bearer", "sk-or-"] {
            assert!(!line.contains(needle), "leaked {needle:?}");
        }
        assert!(!line.contains('\n'), "a signal is one line");
    }

    #[test]
    fn a_model_with_json_special_characters_still_renders_one_valid_line() {
        let signal = ChatRequestSignal {
            provider: "openrouter".to_owned(),
            model: "weird/\"model\"\u{1f428}\\new".to_owned(),
            latency_ms: 1,
            status: "ok",
            error_type: None,
            attempt: 1,
            request_count: 1,
        };

        let line = chat_signal_line(&signal);
        assert!(!line.contains('\n'));

        let value: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["model"], "weird/\"model\"\u{1f428}\\new");
    }

    #[test]
    fn every_line_is_valid_json() {
        for status in ["ok", "error"] {
            for error_type in [None, Some("unreachable")] {
                let line = chat_signal_line(&signal(status, error_type));
                assert!(serde_json::from_str::<Value>(&line).is_ok(), "{line}");
            }
        }
    }
}
