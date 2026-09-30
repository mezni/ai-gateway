use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponseDto {
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessResponseDto {
    pub status: String,
    /// Id of the provider currently serving completions, so an operator can
    /// confirm which selection is live without a separate endpoint (FR-008).
    pub provider: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiErrorDto {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatCompletionRequestDto {
    pub model: String,
    pub messages: Vec<MessageDto>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub stream: bool,
}

/// A request field the client sent that this gateway cannot accept.
///
/// The rendered text names neither the field nor the offending value, so a
/// `serde_json` error can never leak a submitted value or a parser detail into
/// a client response. `Display` exists only to satisfy the deserializer contract.
#[derive(Debug)]
struct MalformedField;

impl de::Error for MalformedField {
    fn custom<T: fmt::Display>(_: T) -> Self {
        MalformedField
    }
}

impl fmt::Display for MalformedField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the chat request is invalid")
    }
}

impl std::error::Error for MalformedField {}

impl<'de> Deserialize<'de> for ChatCompletionRequestDto {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ChatCompletionRequestVisitor)
    }
}

/// Reads the request map, tracking which keys were seen within this one call.
///
/// Seen keys live in this visitor's own state, so a duplicate is refused without
/// any state shared between requests.
struct ChatCompletionRequestVisitor;

impl<'de> Visitor<'de> for ChatCompletionRequestVisitor {
    type Value = ChatCompletionRequestDto;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a chat completion request object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut model: Option<String> = None;
        let mut messages: Option<Vec<MessageDto>> = None;
        let mut temperature: Option<f64> = None;
        let mut max_tokens: Option<u32> = None;
        let mut stream: Option<bool> = None;
        let mut seen_temperature = false;
        let mut seen_max_tokens = false;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "model" => {
                    model = Some(map.next_value::<String>()?);
                }
                "messages" => {
                    messages = Some(map.next_value::<Vec<MessageDto>>()?);
                }
                "temperature" => {
                    if seen_temperature {
                        return Err(de::Error::custom(MalformedField));
                    }
                    seen_temperature = true;
                    temperature = Some(
                        read_temperature(&map.next_value::<Value>()?).map_err(de::Error::custom)?,
                    );
                }
                "max_tokens" => {
                    if seen_max_tokens {
                        return Err(de::Error::custom(MalformedField));
                    }
                    seen_max_tokens = true;
                    max_tokens = Some(
                        read_max_tokens(&map.next_value::<Value>()?).map_err(de::Error::custom)?,
                    );
                }
                "stream" => {
                    stream = Some(map.next_value::<bool>()?);
                }
                _ => {
                    map.next_value::<de::IgnoredAny>()?;
                }
            }
        }

        Ok(ChatCompletionRequestDto {
            model: model.ok_or_else(|| de::Error::custom(MalformedField))?,
            messages: messages.ok_or_else(|| de::Error::custom(MalformedField))?,
            temperature,
            max_tokens,
            stream: stream.unwrap_or(false),
        })
    }
}

/// Accepts only a JSON number for `temperature`.
///
/// A string, boolean, `null`, object, or array is refused rather than being
/// treated as absent. The number's value is not range-checked here; that is the
/// validation stage's job, so out-of-range numbers reach the application layer.
fn read_temperature(value: &Value) -> Result<f64, MalformedField> {
    value.as_f64().ok_or(MalformedField)
}

/// Accepts only a JSON integer for `max_tokens`.
///
/// `100.5` and `1e3` are refused because `as_u64` declines them, and `u32::try_from`
/// refuses anything that does not fit. Range checking stays with the validation stage.
fn read_max_tokens(value: &Value) -> Result<u32, MalformedField> {
    let integer = value.as_u64().ok_or(MalformedField)?;

    u32::try_from(integer).map_err(|_| MalformedField)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageDto {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatCompletionResponseDto {
    pub id: String,
    pub object: String,
    pub model: String,
    pub choices: Vec<ChatCompletionChoiceDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatCompletionChoiceDto {
    pub index: u32,
    pub message: AssistantMessageDto,
    pub finish_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistantMessageDto {
    pub role: String,
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn health_response_serializes_exact_status() {
        let response = HealthResponseDto {
            status: "ok".to_owned(),
        };

        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value, json!({"status": "ok"}));
    }

    #[test]
    fn readiness_response_serializes_ready_status() {
        let response = ReadinessResponseDto {
            status: "ready".to_owned(),
            provider: "deterministic".to_owned(),
        };

        let value = serde_json::to_value(response).unwrap();

        assert_eq!(
            value,
            json!({"status": "ready", "provider": "deterministic"})
        );
    }

    #[test]
    fn readiness_response_serializes_not_ready_status() {
        let response = ReadinessResponseDto {
            status: "not_ready".to_owned(),
            provider: "deterministic".to_owned(),
        };

        let value = serde_json::to_value(response).unwrap();

        assert_eq!(
            value,
            json!({"status": "not_ready", "provider": "deterministic"})
        );
    }

    #[test]
    fn api_error_serializes_exact_code_and_message_fields() {
        let response = ApiErrorDto {
            code: "invalid_request".to_owned(),
            message: "The chat request is invalid.".to_owned(),
        };

        let value = serde_json::to_value(response).unwrap();
        let object = value.as_object().unwrap();

        assert_eq!(
            value,
            json!({
                "code": "invalid_request",
                "message": "The chat request is invalid."
            })
        );
        assert_eq!(object.len(), 2);
        assert!(!object.contains_key("details"));
    }

    #[test]
    fn api_error_deserializes_code_and_message() {
        let response: ApiErrorDto = serde_json::from_str(
            r#"{"code":"internal_error","message":"The gateway could not complete the request."}"#,
        )
        .unwrap();

        assert_eq!(response.code, "internal_error");
        assert_eq!(
            response.message,
            "The gateway could not complete the request."
        );
    }

    #[test]
    fn chat_completion_request_refuses_non_numeric_temperature() {
        for rejected in [r#""0.5""#, "true", "null", "{}", "[]", r#""""#] {
            let body = format!(
                r#"{{"model":"mock-model","messages":[{{"role":"user","content":"Hello"}}],"temperature":{rejected}}}"#
            );

            assert!(
                parse_request(&body).is_err(),
                "temperature {rejected} should be refused"
            );
        }
    }

    #[test]
    fn chat_completion_request_refuses_malformed_max_tokens() {
        for rejected in ["true", "null", "[500]", "100.5", "1e3", r#""500""#, "{}"] {
            let body = format!(
                r#"{{"model":"mock-model","messages":[{{"role":"user","content":"Hello"}}],"max_tokens":{rejected}}}"#
            );

            assert!(
                parse_request(&body).is_err(),
                "max_tokens {rejected} should be refused"
            );
        }
    }

    #[test]
    fn chat_completion_request_refuses_a_repeated_control() {
        for body in [
            r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":0.5,"temperature":1.0}"#,
            r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"max_tokens":10,"max_tokens":20}"#,
        ] {
            assert!(
                parse_request(body).is_err(),
                "a repeated control should be refused: {body}"
            );
        }
    }

    #[test]
    fn chat_completion_request_refuses_a_repeated_control_even_when_both_are_well_formed() {
        // Both copies are individually acceptable; the duplicate is the breach.
        let body = r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":0.5,"temperature":0.5}"#;

        assert!(parse_request(body).is_err());
    }

    #[test]
    fn chat_completion_request_keeps_ignoring_unknown_fields() {
        let response = parse_request(
            r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello","name":"n","extra":1}],"temperature":0.5,"max_tokens":8,"n":4,"top_p":0.9,"stream":false,"future":{"nested":[1,2,{"deep":null}]}}"#,
        )
        .unwrap();

        assert_eq!(response.temperature, Some(0.5));
        assert_eq!(response.max_tokens, Some(8));
    }

    #[test]
    fn chat_completion_request_out_of_range_numbers_still_deserialize() {
        // Range checking belongs to the validation stage, so these must parse.
        let response = parse_request(
            r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":9.0,"max_tokens":99999}"#,
        )
        .unwrap();

        assert_eq!(response.temperature, Some(9.0));
        assert_eq!(response.max_tokens, Some(99_999));
    }

    #[test]
    fn chat_completion_request_still_requires_model_and_messages() {
        for body in [
            r#"{"messages":[{"role":"user","content":"Hello"}]}"#,
            r#"{"model":"mock-model"}"#,
        ] {
            assert!(parse_request(body).is_err(), "{body} should be refused");
        }
    }

    #[test]
    fn deserialization_errors_never_name_the_field_or_the_value() {
        // Only bodies that genuinely fail to deserialize belong here; an
        // out-of-range but well-formed control parses and is refused later by
        // the validation stage, which renders its own fixed message.
        //
        // `serde_json` appends its own positional suffix ("at line N column M")
        // to any custom error. That text is discarded at the API boundary by
        // `ApiError::from_json_rejection`, which replaces it with the fixed
        // client message; the integration tests assert that no parser detail
        // reaches a client. So only field names and submitted values are
        // checked here.
        for body in [
            r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":"0.5"}"#,
            r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"max_tokens":1e3}"#,
            r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"max_tokens":100.5}"#,
            r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":null}"#,
            r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":0.5,"temperature":1.0}"#,
            r#"{"messages":[{"role":"user","content":"Hello"}]}"#,
            r#"{"model":"mock-model"}"#,
        ] {
            let error = parse_request(body).unwrap_err().to_string();

            for needle in [
                "model",
                "messages",
                "role",
                "content",
                "temperature",
                "max_tokens",
                "stream",
                "0.5",
                "100.5",
                "1000",
            ] {
                assert!(!error.contains(needle), "leaked {needle:?} in {error:?}");
            }
        }
    }

    fn parse_request(body: &str) -> Result<ChatCompletionRequestDto, serde_json::Error> {
        serde_json::from_str(body)
    }

    #[test]
    fn chat_completion_request_defaults_stream_to_false_and_ignores_unknown_fields() {
        let response: ChatCompletionRequestDto = serde_json::from_str(
            r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello","name":"ignored"}],"temperature":0.5}"#,
        )
        .unwrap();

        assert_eq!(response.model, "mock-model");
        assert_eq!(response.messages.len(), 1);
        assert_eq!(response.messages[0].role, "user");
        assert_eq!(response.messages[0].content, "Hello");
        assert_eq!(response.temperature, Some(0.5));
        assert_eq!(response.max_tokens, None);
        assert!(!response.stream);
    }

    #[test]
    fn chat_completion_request_defaults_both_controls_to_none_when_absent() {
        let response: ChatCompletionRequestDto = serde_json::from_str(
            r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}]}"#,
        )
        .unwrap();

        assert_eq!(response.temperature, None);
        assert_eq!(response.max_tokens, None);
    }

    #[test]
    fn chat_completion_request_deserializes_both_controls_when_present() {
        let response: ChatCompletionRequestDto = serde_json::from_str(
            r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":0.7,"max_tokens":500}"#,
        )
        .unwrap();

        assert_eq!(response.temperature, Some(0.7));
        assert_eq!(response.max_tokens, Some(500));
    }

    #[test]
    fn chat_completion_request_accepts_control_boundaries() {
        for (temperature, max_tokens) in [(0.0, 1), (2.0, 4096)] {
            let body = format!(
                r#"{{"model":"mock-model","messages":[{{"role":"user","content":"Hello"}}],"temperature":{temperature},"max_tokens":{max_tokens}}}"#
            );
            let response: ChatCompletionRequestDto = serde_json::from_str(&body).unwrap();

            assert_eq!(response.temperature, Some(temperature));
            assert_eq!(response.max_tokens, Some(max_tokens));
        }
    }

    #[test]
    fn chat_completion_request_ignores_unknown_fields_while_reading_controls() {
        let response: ChatCompletionRequestDto = serde_json::from_str(
            r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":0.25,"max_tokens":8,"n":4,"top_p":0.9,"future":{"nested":[1,2]}}"#,
        )
        .unwrap();

        assert_eq!(response.temperature, Some(0.25));
        assert_eq!(response.max_tokens, Some(8));
    }

    #[test]
    fn chat_completion_response_serializes_exact_envelope_without_usage_or_created() {
        let response = ChatCompletionResponseDto {
            id: "chat_mock".to_owned(),
            object: "chat.completion".to_owned(),
            model: "mock-model".to_owned(),
            choices: vec![ChatCompletionChoiceDto {
                index: 0,
                message: AssistantMessageDto {
                    role: "assistant".to_owned(),
                    content: "This is a mocked chat completion.".to_owned(),
                },
                finish_reason: "stop".to_owned(),
            }],
        };

        let value = serde_json::to_value(&response).unwrap();
        let object = value.as_object().unwrap();

        assert_eq!(
            value,
            json!({
                "id": "chat_mock",
                "object": "chat.completion",
                "model": "mock-model",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "This is a mocked chat completion."
                    },
                    "finish_reason": "stop"
                }]
            })
        );
        assert_eq!(object.len(), 4);
        assert!(!object.contains_key("usage"));
        assert!(!object.contains_key("created"));
    }
}
