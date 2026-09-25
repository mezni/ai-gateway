use axum::extract::rejection::JsonRejection;
use axum::{Json, extract::State};

use crate::{
    api::{
        dto::{
            AssistantMessageDto, ChatCompletionChoiceDto, ChatCompletionRequestDto,
            ChatCompletionResponseDto,
        },
        error::ApiError,
    },
    application::{
        AppState,
        chat::{CompleteChatCommand, IncomingMessage},
    },
};

pub async fn chat_completions(
    State(state): State<AppState>,
    payload: Result<Json<ChatCompletionRequestDto>, JsonRejection>,
) -> Result<Json<ChatCompletionResponseDto>, ApiError> {
    let Json(request) = payload.map_err(ApiError::from_json_rejection)?;
    let ChatCompletionRequestDto {
        model,
        messages,
        stream,
    } = request;

    let command = CompleteChatCommand {
        model,
        messages: messages
            .into_iter()
            .map(|message| IncomingMessage {
                role: message.role,
                content: message.content,
            })
            .collect(),
    };

    let validated = state
        .chat_service
        .validate(command)
        .map_err(|_| ApiError::InvalidRequest)?;

    if stream {
        return Err(ApiError::UnsupportedFeature);
    }

    let model = validated.model().to_owned();
    let completion = state.chat_service.complete(validated);

    Ok(Json(ChatCompletionResponseDto {
        id: "chat_mock".to_owned(),
        object: "chat.completion".to_owned(),
        model,
        choices: vec![ChatCompletionChoiceDto {
            index: 0,
            message: AssistantMessageDto {
                role: "assistant".to_owned(),
                content: completion.content,
            },
            finish_reason: "stop".to_owned(),
        }],
    }))
}
