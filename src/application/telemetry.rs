//! Per-request chat telemetry signals.
//!
//! Every completed chat request yields exactly one [`ChatRequestSignal`] with
//! the documented fields: provider, model, latency, status, error type,
//! attempt, and the monotonic request count.
//!
//! What a signal deliberately never carries:
//!
//! - **Prompts or message content.** The model name identifies the request's
//!   target; the conversation itself is user data and is masked by omission
//!   (spec 006 clarification: "mask prompts in telemetry").
//! - **Credentials.** No field exists that a provider adapter could fill with
//!   an API key, and [`crate::application::chat::ProviderFailure`] is
//!   fieldless, so an error type names a category and nothing more.
//! - **Request ids.** The gateway's request-id correlation surface is a later
//!   phase; a signal is complete without one.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use crate::application::chat::ProviderFailure;

/// Telemetry state shared by every request through [`super::AppState`].
///
/// The counter is monotonic and per-process: it survives request completion
/// and never resets, which is the semantics a "request count" metric needs.
#[derive(Clone, Debug)]
pub struct ChatTelemetry {
    request_count: Arc<AtomicU64>,
}

impl ChatTelemetry {
    pub fn new() -> Self {
        Self {
            request_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Records one completed chat request and returns its signal.
    ///
    /// `success` is `true` only when the provider produced a usable
    /// completion; a request that failed validation before reaching a
    /// provider is not recorded here, because the application layer rejects
    /// it before this state is consulted.
    pub fn record_chat(
        &self,
        provider: &str,
        model: &str,
        latency_ms: u64,
        success: bool,
        error: Option<ProviderFailure>,
    ) -> ChatRequestSignal {
        // Saturation guards the arithmetic rather than the metric: a counter
        // that overflows at u64::MAX has a more useful failure mode than a
        // wrapped or panicked one.
        let request_count = self.request_count.fetch_add(1, Ordering::Relaxed).saturating_add(1);

        ChatRequestSignal {
            provider: provider.to_owned(),
            model: model.to_owned(),
            latency_ms,
            status: if success { "ok" } else { "error" },
            error_type: error.map(error_type_name),
            attempt: 1,
            request_count,
        }
    }

    /// The number of chat requests recorded so far, for operator visibility.
    pub fn request_count(&self) -> u64 {
        self.request_count.load(Ordering::Relaxed)
    }
}

impl Default for ChatTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

/// The error type a signal reports: a stable category name, never a message.
fn error_type_name(failure: ProviderFailure) -> &'static str {
    match failure {
        ProviderFailure::Unreachable => "unreachable",
        ProviderFailure::Refused => "refused",
        ProviderFailure::DeadlineExceeded => "deadline_exceeded",
        ProviderFailure::UnusableResponse => "unusable_response",
        ProviderFailure::InvalidResponse => "invalid_response",
    }
}

/// One chat request's telemetry signal.
///
/// Rendering (e.g. to a JSON log line) belongs to the emitting layer, which
/// owns serialization; this struct stays plain so the application layer does
/// not depend on a serialization crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRequestSignal {
    /// The provider that served (or attempted to serve) the request.
    pub provider: String,
    /// The model the request targeted.
    pub model: String,
    /// Wall-clock time from just before the provider call to its outcome.
    pub latency_ms: u64,
    /// `"ok"` when the provider produced a usable completion.
    pub status: &'static str,
    /// The failure category when `status` is `"error"`, otherwise `None`.
    pub error_type: Option<&'static str>,
    /// The attempt number. This gateway performs no retries (Phase 4
    /// provider contract §6), so this is always 1 until a retry policy lands.
    pub attempt: u32,
    /// The monotonic request count including this request.
    pub request_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_telemetry_state_has_not_recorded_any_request() {
        let telemetry = ChatTelemetry::new();

        assert_eq!(telemetry.request_count(), 0);
    }

    #[test]
    fn recording_increments_the_request_count_monotonically() {
        let telemetry = ChatTelemetry::new();

        let first = telemetry.record_chat("openrouter", "m", 1, true, None);
        let second = telemetry.record_chat("openrouter", "m", 2, true, None);
        let third = telemetry.record_chat("openrouter", "m", 3, true, None);

        assert_eq!(first.request_count, 1);
        assert_eq!(second.request_count, 2);
        assert_eq!(third.request_count, 3);
        assert_eq!(telemetry.request_count(), 3);
    }

    #[test]
    fn a_shared_state_counts_across_clones() {
        let telemetry = ChatTelemetry::new();
        let shared = telemetry.clone();

        let a = telemetry.record_chat("openrouter", "m", 1, true, None);
        let b = shared.record_chat("openrouter", "m", 2, true, None);

        assert_eq!(a.request_count, 1);
        assert_eq!(b.request_count, 2);
        assert_eq!(telemetry.request_count(), 2);
    }

    #[test]
    fn a_success_signal_carries_the_documented_fields() {
        let telemetry = ChatTelemetry::new();

        let signal = telemetry.record_chat("openrouter", "openai/gpt-4o", 4_321, true, None);

        assert_eq!(signal.provider, "openrouter");
        assert_eq!(signal.model, "openai/gpt-4o");
        assert_eq!(signal.latency_ms, 4_321);
        assert_eq!(signal.status, "ok");
        assert_eq!(signal.error_type, None);
        assert_eq!(signal.attempt, 1);
        assert_eq!(signal.request_count, 1);
    }

    #[test]
    fn each_failure_category_renders_its_stable_name() {
        let telemetry = ChatTelemetry::new();
        let cases = [
            (ProviderFailure::Unreachable, "unreachable"),
            (ProviderFailure::Refused, "refused"),
            (ProviderFailure::DeadlineExceeded, "deadline_exceeded"),
            (ProviderFailure::UnusableResponse, "unusable_response"),
            (ProviderFailure::InvalidResponse, "invalid_response"),
        ];

        for (failure, expected) in cases {
            let signal = telemetry.record_chat("openrouter", "m", 1, false, Some(failure));

            assert_eq!(signal.status, "error", "{failure:?}");
            assert_eq!(signal.error_type, Some(expected), "{failure:?}");
        }
    }

    #[test]
    fn a_signal_cannot_carry_a_prompt_or_a_credential() {
        // The signal's field set is the masking guarantee: provider, model,
        // latency, status, error type, attempt, count. A prompt, message body,
        // or credential has nowhere to live, and nothing that does live in a
        // field may contain one.
        let sentinel_prompt = "SENTINEL_PROMPT_XYZ";
        let sentinel_key = "SENTINEL_KEY_XYZ";

        let signal = ChatRequestSignal {
            provider: "openrouter".to_owned(),
            model: "echo/model".to_owned(),
            latency_ms: 12,
            status: "error",
            error_type: Some("refused"),
            attempt: 1,
            request_count: 7,
        };

        for field in [
            signal.provider.as_str(),
            signal.model.as_str(),
            signal.status,
            signal.error_type.unwrap_or(""),
        ] {
            assert!(!field.contains(sentinel_prompt), "prompt in {field}");
            assert!(!field.contains(sentinel_key), "credential in {field}");
        }
        assert_eq!(signal.attempt, 1);
        assert_eq!(signal.request_count, 7);
    }
}
