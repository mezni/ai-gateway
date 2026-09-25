use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponseDto {
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessResponseDto {
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiErrorDto {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatCompletionRequestDto {
    pub model: String,
    pub messages: Vec<MessageDto>,
    #[serde(default)]
    pub stream: bool,
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
        };

        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value, json!({"status": "ready"}));
    }

    #[test]
    fn readiness_response_serializes_not_ready_status() {
        let response = ReadinessResponseDto {
            status: "not_ready".to_owned(),
        };

        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value, json!({"status": "not_ready"}));
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
    fn chat_completion_request_defaults_stream_to_false_and_ignores_unknown_fields() {
        let response: ChatCompletionRequestDto = serde_json::from_str(
            r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello","name":"ignored"}],"temperature":0.5}"#,
        )
        .unwrap();

        assert_eq!(response.model, "mock-model");
        assert_eq!(response.messages.len(), 1);
        assert_eq!(response.messages[0].role, "user");
        assert_eq!(response.messages[0].content, "Hello");
        assert!(!response.stream);
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
