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
        chat::{ChatValidator, CompleteChatCommand, IncomingMessage, call_provider_bounded},
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
        temperature,
        max_tokens,
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
        temperature,
        max_tokens,
        stream,
    };

    // Validation first: a request that breaks a rule is rejected without any
    // provider being contacted, so an invalid request can never surface as a
    // provider failure.
    let validated = ChatValidator::validate(command).map_err(ApiError::from_validation_error)?;

    let model = validated.model().to_owned();
    let response = call_provider_bounded(
        state.chat_service.as_ref(),
        validated.request(),
        state.provider_config.deadline_ms,
    )
    .await
    .map_err(ApiError::from_provider_failure)?;

    Ok(Json(ChatCompletionResponseDto {
        id: "chat_mock".to_owned(),
        object: "chat.completion".to_owned(),
        model,
        choices: vec![ChatCompletionChoiceDto {
            index: 0,
            message: AssistantMessageDto {
                role: "assistant".to_owned(),
                content: response.content,
            },
            finish_reason: "stop".to_owned(),
        }],
    }))
}
