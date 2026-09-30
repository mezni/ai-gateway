use crate::api::dto::ApiErrorDto;
use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    #[error("The chat request is invalid.")]
    InvalidRequest,
    #[error("Streaming is not supported.")]
    UnsupportedFeature,
    #[error("The request media type is not supported.")]
    UnsupportedMediaType,
    #[error("The request method is not allowed for this path.")]
    MethodNotAllowed,
    #[error("The requested path was not found.")]
    NotFound,
    #[error("The gateway is not accepting new chat requests.")]
    NotReady,
    #[error("The request payload is too large.")]
    PayloadTooLarge,
    #[error("The gateway could not complete the request.")]
    InternalError,
    #[error("The upstream provider is unavailable.")]
    ProviderUnavailable,
    #[error("The upstream provider did not respond in time.")]
    ProviderTimeout,
}

impl ApiError {
    /// Maps an application-layer validation failure to the client error.
    ///
    /// Every field-level failure collapses to the single documented
    /// `invalid_request` row, so the offending field is never disclosed. A
    /// request for an unimplemented stream keeps its own documented row.
    pub fn from_validation_error(error: crate::application::chat::ValidationError) -> Self {
        use crate::application::chat::ValidationFailure;

        match error.failure {
            ValidationFailure::StreamingRequested => Self::UnsupportedFeature,
            ValidationFailure::InvalidModel
            | ValidationFailure::InvalidMessages
            | ValidationFailure::InvalidRole
            | ValidationFailure::InvalidContent
            | ValidationFailure::TemperatureOutOfRange
            | ValidationFailure::MaxTokensOutOfRange => Self::InvalidRequest,
        }
    }

    pub fn from_json_rejection(rejection: JsonRejection) -> Self {
        match rejection {
            JsonRejection::MissingJsonContentType(_) => Self::UnsupportedMediaType,
            JsonRejection::JsonSyntaxError(_) | JsonRejection::JsonDataError(_) => {
                Self::InvalidRequest
            }
            JsonRejection::BytesRejection(_) => Self::InvalidRequest,
            _ => Self::InternalError,
        }
    }

    /// Maps a provider failure category to the client-facing error row.
    ///
    /// The mapping is exhaustive over the five categories and is the *only*
    /// place a provider outcome becomes a response. `ProviderFailure` is
    /// fieldless, so there is no provider detail here to redact: the category
    /// alone selects the row, and the row's message is a fixed string.
    ///
    /// No category maps to a validation row. A client that sent a bad request
    /// must keep seeing `invalid_request`; a provider that failed later is a
    /// gateway-side problem (FR-012, FR-013).
    pub fn from_provider_failure(failure: crate::application::chat::ProviderFailure) -> Self {
        use crate::application::chat::ProviderFailure;

        match failure {
            ProviderFailure::Unreachable | ProviderFailure::Refused => Self::ProviderUnavailable,
            ProviderFailure::DeadlineExceeded => Self::ProviderTimeout,
            ProviderFailure::UnusableResponse | ProviderFailure::InvalidResponse => {
                Self::InternalError
            }
        }
    }

    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidRequest | Self::UnsupportedFeature => StatusCode::BAD_REQUEST,
            Self::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::NotReady => StatusCode::SERVICE_UNAVAILABLE,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ProviderUnavailable => StatusCode::BAD_GATEWAY,
            Self::ProviderTimeout => StatusCode::GATEWAY_TIMEOUT,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::UnsupportedFeature => "unsupported_feature",
            Self::UnsupportedMediaType => "unsupported_media_type",
            Self::MethodNotAllowed => "method_not_allowed",
            Self::NotFound => "not_found",
            Self::NotReady => "not_ready",
            Self::PayloadTooLarge => "payload_too_large",
            Self::InternalError => "internal_error",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::ProviderTimeout => "provider_timeout",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::InvalidRequest => "The chat request is invalid.",
            Self::UnsupportedFeature => "Streaming is not supported.",
            Self::UnsupportedMediaType => "The request media type is not supported.",
            Self::MethodNotAllowed => "The request method is not allowed for this path.",
            Self::NotFound => "The requested path was not found.",
            Self::NotReady => "The gateway is not accepting new chat requests.",
            Self::PayloadTooLarge => "The request payload is too large.",
            Self::InternalError => "The gateway could not complete the request.",
            Self::ProviderUnavailable => "The upstream provider is unavailable.",
            Self::ProviderTimeout => "The upstream provider did not respond in time.",
        }
    }

    pub fn to_dto(&self) -> ApiErrorDto {
        ApiErrorDto {
            code: self.code().to_owned(),
            message: self.message().to_owned(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status_code(), Json(self.to_dto())).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use axum::http::header::CONTENT_TYPE;
    use axum::response::IntoResponse;
    use serde_json::json;

    async fn assert_error_contract(
        error: ApiError,
        expected_status: StatusCode,
        expected_code: &str,
        expected_message: &str,
    ) {
        assert_eq!(error.status_code(), expected_status);
        assert_eq!(error.code(), expected_code);
        assert_eq!(error.message(), expected_message);
        assert_eq!(error.to_string(), expected_message);
        assert_eq!(error.to_dto().code, expected_code);
        assert_eq!(error.to_dto().message, expected_message);

        let response = error.into_response();

        assert_eq!(response.status(), expected_status);
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "application/json"
        );

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let object = value.as_object().unwrap();

        assert_eq!(
            value,
            json!({
                "code": expected_code,
                "message": expected_message,
            })
        );
        assert_eq!(object.len(), 2);
        assert!(!object.contains_key("details"));
        assert!(!object.contains_key("source"));
        assert!(!object.contains_key("diagnostic"));
        assert!(!object.contains_key("usage"));
    }

    #[tokio::test]
    async fn invalid_request_returns_400_contract() {
        assert_error_contract(
            ApiError::InvalidRequest,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The chat request is invalid.",
        )
        .await;
    }

    #[tokio::test]
    async fn unsupported_feature_returns_400_contract() {
        assert_error_contract(
            ApiError::UnsupportedFeature,
            StatusCode::BAD_REQUEST,
            "unsupported_feature",
            "Streaming is not supported.",
        )
        .await;
    }

    #[tokio::test]
    async fn unsupported_media_type_returns_415_contract() {
        assert_error_contract(
            ApiError::UnsupportedMediaType,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "The request media type is not supported.",
        )
        .await;
    }

    #[tokio::test]
    async fn method_not_allowed_returns_405_contract() {
        assert_error_contract(
            ApiError::MethodNotAllowed,
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "The request method is not allowed for this path.",
        )
        .await;
    }

    #[tokio::test]
    async fn not_found_returns_404_contract() {
        assert_error_contract(
            ApiError::NotFound,
            StatusCode::NOT_FOUND,
            "not_found",
            "The requested path was not found.",
        )
        .await;
    }

    #[tokio::test]
    async fn payload_too_large_returns_413_contract() {
        assert_error_contract(
            ApiError::PayloadTooLarge,
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "The request payload is too large.",
        )
        .await;
    }

    #[tokio::test]
    async fn payload_too_large_message_never_discloses_a_size_or_limit() {
        let message = ApiError::PayloadTooLarge.message();

        for needle in [
            "1048576",
            "1_048_576",
            "1MB",
            "1 MB",
            "limit",
            "byte",
            "size",
            "length",
            "MAX_",
        ] {
            assert!(
                !message.contains(needle),
                "leaked {needle:?} in {message:?}"
            );
        }
    }

    #[tokio::test]
    async fn not_ready_returns_503_contract() {
        assert_error_contract(
            ApiError::NotReady,
            StatusCode::SERVICE_UNAVAILABLE,
            "not_ready",
            "The gateway is not accepting new chat requests.",
        )
        .await;
    }

    #[tokio::test]
    async fn internal_error_returns_500_contract() {
        assert_error_contract(
            ApiError::InternalError,
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "The gateway could not complete the request.",
        )
        .await;
    }

    use crate::api::dto::ChatCompletionRequestDto;
    use axum::extract::rejection::{JsonRejection, MissingJsonContentType};

    const SYNTAX_PAYLOAD: &[u8] = b"{";
    const EMPTY_PAYLOAD: &[u8] = b"";
    const TRAILING_PAYLOAD: &[u8] = br#"{"model":"m","messages":[]} trailing"#;
    const DATA_PAYLOAD: &[u8] = br#"{"model":1}"#;
    const MISSING_CONTENT_PAYLOAD: &[u8] = br#"{"model":"m","messages":[{"role":"u"}]}"#;
    const SENTINEL_CONTENT_PAYLOAD: &[u8] =
        br#"{"model":987654321,"messages":[{"role":"u","content":"SENTINEL_CONTENT_VALUE"}]}"#;
    const SENTINEL_TYPE_PAYLOAD: &[u8] =
        br#"{"model":"SENTINEL_MODEL","messages":[],"stream":"SENTINEL_CONTENT_VALUE"}"#;

    fn missing_media_type_rejection() -> JsonRejection {
        JsonRejection::from(MissingJsonContentType::default())
    }

    fn rejection_from(payload: &[u8]) -> JsonRejection {
        match Json::<ChatCompletionRequestDto>::from_bytes(payload) {
            Ok(_) => panic!("expected a JSON rejection for the given payload"),
            Err(rejection) => rejection,
        }
    }

    fn assert_mapped(error: ApiError, status: StatusCode, code: &str, message: &str) {
        assert_eq!(error.status_code(), status);
        assert_eq!(error.code(), code);
        assert_eq!(error.message(), message);
        assert_eq!(error.to_string(), message);
        assert_eq!(error.to_dto().code, code);
        assert_eq!(error.to_dto().message, message);
    }

    async fn assert_mapped_body(
        error: ApiError,
        status: StatusCode,
        code: &str,
        message: &str,
    ) -> String {
        assert_mapped(error, status, code, message);
        assert_error_contract(error, status, code, message).await;

        let response = error.into_response();
        assert_eq!(response.status(), status);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let object = value.as_object().unwrap();

        assert_eq!(value, json!({ "code": code, "message": message }));
        assert_eq!(object.len(), 2);
        assert!(!object.contains_key("details"));
        assert!(!object.contains_key("source"));
        assert!(!object.contains_key("diagnostic"));

        text
    }

    #[tokio::test]
    async fn missing_media_type_rejection_maps_to_unsupported_media_type() {
        let rejection = missing_media_type_rejection();
        let rejection_text = rejection.body_text();

        assert_eq!(rejection.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert!(rejection_text.contains("Content-Type: application/json"));

        let error = ApiError::from_json_rejection(rejection);
        let body = assert_mapped_body(
            error,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "The request media type is not supported.",
        )
        .await;

        assert_ne!(error.message(), rejection_text);
        assert!(!body.contains("Content-Type: application/json"));
    }

    #[tokio::test]
    async fn json_syntax_rejection_maps_to_invalid_request() {
        let rejection = rejection_from(SYNTAX_PAYLOAD);
        let rejection_text = rejection.body_text();

        assert_eq!(rejection.status(), StatusCode::BAD_REQUEST);
        assert!(!rejection_text.is_empty());
        assert!(!rejection.to_string().is_empty());
        assert!(rejection_text.contains("Failed to parse the request body as JSON"));

        let error = ApiError::from_json_rejection(rejection);
        let body = assert_mapped_body(
            error,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The chat request is invalid.",
        )
        .await;

        assert_ne!(error.message(), rejection_text);
        assert!(
            !error
                .message()
                .contains("Failed to parse the request body as JSON")
        );
        assert!(!body.contains(&rejection_text));
        assert!(!body.contains("Failed to parse the request body as JSON"));
    }

    #[tokio::test]
    async fn trailing_content_rejection_maps_to_invalid_request() {
        let rejection = rejection_from(TRAILING_PAYLOAD);
        let rejection_text = rejection.body_text();

        assert!(rejection_text.contains("trailing characters"));

        let body = assert_mapped_body(
            ApiError::from_json_rejection(rejection),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The chat request is invalid.",
        )
        .await;

        assert!(!body.contains("trailing characters"));
        assert!(!body.contains(&rejection_text));
    }

    #[tokio::test]
    async fn json_data_rejection_maps_to_invalid_request() {
        let missing_content = rejection_from(MISSING_CONTENT_PAYLOAD);
        let missing_content_text = missing_content.body_text();

        assert_eq!(missing_content.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(missing_content_text.contains("missing field `content`"));

        let missing_content_body = assert_mapped_body(
            ApiError::from_json_rejection(missing_content),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The chat request is invalid.",
        )
        .await;

        assert!(!missing_content_body.contains(&missing_content_text));
        assert!(!missing_content_body.contains("missing field"));
        assert!(!missing_content_body.contains("content"));

        let wrong_model = rejection_from(SENTINEL_CONTENT_PAYLOAD);
        let wrong_model_text = wrong_model.body_text();

        assert_eq!(wrong_model.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(wrong_model_text.contains("invalid type: integer `987654321`"));

        let wrong_model_body = assert_mapped_body(
            ApiError::from_json_rejection(wrong_model),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The chat request is invalid.",
        )
        .await;

        assert!(!wrong_model_body.contains(&wrong_model_text));
        assert!(!wrong_model_body.contains("SENTINEL_CONTENT_VALUE"));
        assert!(!wrong_model_body.contains("987654321"));
        assert!(!wrong_model_body.contains("invalid type"));
        assert!(!wrong_model_body.contains("model"));

        let body = assert_mapped_body(
            ApiError::from_json_rejection(rejection_from(SENTINEL_TYPE_PAYLOAD)),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The chat request is invalid.",
        )
        .await;

        for leaked in [
            &missing_content_text,
            &wrong_model_text,
            "SENTINEL_CONTENT_VALUE",
            "SENTINEL_MODEL",
            "987654321",
            "missing field",
            "invalid type",
            "model",
            "messages",
            "content",
            "role",
            "stream",
        ] {
            assert!(!body.contains(leaked), "leaked {leaked:?} in {body}");
        }
    }

    #[tokio::test]
    async fn empty_body_rejection_maps_to_invalid_request() {
        let rejection = rejection_from(EMPTY_PAYLOAD);
        let rejection_text = rejection.body_text();

        assert!(rejection_text.contains("Failed to parse the request body as JSON"));

        let body = assert_mapped_body(
            ApiError::from_json_rejection(rejection),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The chat request is invalid.",
        )
        .await;

        assert!(!body.contains(&rejection_text));
    }

    #[tokio::test]
    async fn mapped_errors_never_expose_parser_or_source_diagnostics() {
        let forbidden = [
            "Failed to",
            "expected",
            "serde",
            "line ",
            "column",
            "source",
            "diagnostic",
            "JsonRejection",
            "SENTINEL_CONTENT_VALUE",
            "SENTINEL_MODEL",
            "987654321",
            "trailing",
            "content-type",
            "application/json",
        ];

        let cases: Vec<(JsonRejection, StatusCode, &str, &str)> = vec![
            (
                missing_media_type_rejection(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
                "The request media type is not supported.",
            ),
            (
                rejection_from(SYNTAX_PAYLOAD),
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "The chat request is invalid.",
            ),
            (
                rejection_from(DATA_PAYLOAD),
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "The chat request is invalid.",
            ),
            (
                rejection_from(TRAILING_PAYLOAD),
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "The chat request is invalid.",
            ),
            (
                rejection_from(SENTINEL_TYPE_PAYLOAD),
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "The chat request is invalid.",
            ),
        ];

        for (rejection, expected_status, expected_code, expected_message) in cases {
            let error = ApiError::from_json_rejection(rejection);

            assert_eq!(error.to_string(), expected_message);

            let body =
                assert_mapped_body(error, expected_status, expected_code, expected_message).await;

            for needle in forbidden {
                assert!(
                    !body.to_lowercase().contains(needle),
                    "leaked {needle:?} in {body}"
                );
            }
        }
    }

    #[test]
    fn mapped_error_status_matches_contract_for_each_rejection_class() {
        let cases: Vec<(JsonRejection, StatusCode, &str)> = vec![
            (
                missing_media_type_rejection(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
            ),
            (
                rejection_from(SYNTAX_PAYLOAD),
                StatusCode::BAD_REQUEST,
                "invalid_request",
            ),
            (
                rejection_from(DATA_PAYLOAD),
                StatusCode::BAD_REQUEST,
                "invalid_request",
            ),
            (
                rejection_from(TRAILING_PAYLOAD),
                StatusCode::BAD_REQUEST,
                "invalid_request",
            ),
        ];

        for (rejection, expected_status, expected_code) in cases {
            let error = ApiError::from_json_rejection(rejection);

            assert_eq!(error.status_code(), expected_status);
            assert_eq!(error.status_code().as_u16(), expected_status.as_u16());
            assert_eq!(error.code(), expected_code);
            assert_eq!(error.into_response().status(), expected_status);
        }
    }

    /// Every documented row, pinned together. Adding a row means adding a case
    /// here, and changing a frozen row means changing the table below.
    const ALL_ROWS: [(&str, StatusCode, &str, &str); 10] = [
        (
            "InvalidRequest",
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The chat request is invalid.",
        ),
        (
            "UnsupportedFeature",
            StatusCode::BAD_REQUEST,
            "unsupported_feature",
            "Streaming is not supported.",
        ),
        (
            "UnsupportedMediaType",
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "The request media type is not supported.",
        ),
        (
            "MethodNotAllowed",
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "The request method is not allowed for this path.",
        ),
        (
            "NotFound",
            StatusCode::NOT_FOUND,
            "not_found",
            "The requested path was not found.",
        ),
        (
            "NotReady",
            StatusCode::SERVICE_UNAVAILABLE,
            "not_ready",
            "The gateway is not accepting new chat requests.",
        ),
        (
            "PayloadTooLarge",
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "The request payload is too large.",
        ),
        (
            "InternalError",
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "The gateway could not complete the request.",
        ),
        (
            "ProviderUnavailable",
            StatusCode::BAD_GATEWAY,
            "provider_unavailable",
            "The upstream provider is unavailable.",
        ),
        (
            "ProviderTimeout",
            StatusCode::GATEWAY_TIMEOUT,
            "provider_timeout",
            "The upstream provider did not respond in time.",
        ),
    ];

    /// The eight rows Phase 3 shipped, which this feature may not alter.
    const PHASE_3_FROZEN_ROWS: [(&str, StatusCode, &str, &str); 8] = [
        (
            "InvalidRequest",
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The chat request is invalid.",
        ),
        (
            "UnsupportedFeature",
            StatusCode::BAD_REQUEST,
            "unsupported_feature",
            "Streaming is not supported.",
        ),
        (
            "UnsupportedMediaType",
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "The request media type is not supported.",
        ),
        (
            "MethodNotAllowed",
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "The request method is not allowed for this path.",
        ),
        (
            "NotFound",
            StatusCode::NOT_FOUND,
            "not_found",
            "The requested path was not found.",
        ),
        (
            "NotReady",
            StatusCode::SERVICE_UNAVAILABLE,
            "not_ready",
            "The gateway is not accepting new chat requests.",
        ),
        (
            "PayloadTooLarge",
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "The request payload is too large.",
        ),
        (
            "InternalError",
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "The gateway could not complete the request.",
        ),
    ];

    /// Resolves a row name to the variant, so the tables above stay readable.
    fn variant(name: &str) -> ApiError {
        match name {
            "InvalidRequest" => ApiError::InvalidRequest,
            "UnsupportedFeature" => ApiError::UnsupportedFeature,
            "UnsupportedMediaType" => ApiError::UnsupportedMediaType,
            "MethodNotAllowed" => ApiError::MethodNotAllowed,
            "NotFound" => ApiError::NotFound,
            "NotReady" => ApiError::NotReady,
            "PayloadTooLarge" => ApiError::PayloadTooLarge,
            "InternalError" => ApiError::InternalError,
            "ProviderUnavailable" => ApiError::ProviderUnavailable,
            "ProviderTimeout" => ApiError::ProviderTimeout,
            other => panic!("no ApiError variant named {other}"),
        }
    }

    #[test]
    fn every_documented_row_matches_status_code_and_code() {
        for (name, status, code, _) in ALL_ROWS {
            let error = variant(name);

            assert_eq!(error.status_code(), status, "status for {name}");
            assert_eq!(error.code(), code, "code for {name}");
        }
    }

    #[test]
    fn all_ten_codes_are_distinct() {
        let mut codes: Vec<&str> = ALL_ROWS.iter().map(|(_, _, code, _)| *code).collect();
        codes.sort_unstable();

        let total = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), total, "error codes must be unique");
    }

    #[test]
    fn all_ten_statuses_are_distinct_except_the_two_bad_request_rows() {
        // `invalid_request` and `unsupported_feature` deliberately share 400;
        // every other status must be unique so a client can branch on it.
        let mut statuses: Vec<u16> = ALL_ROWS
            .iter()
            .map(|(_, status, _, _)| status.as_u16())
            .collect();
        let total = statuses.len();
        statuses.sort_unstable();
        statuses.dedup();

        assert_eq!(total - statuses.len(), 1, "exactly one shared status expected");
    }

    #[test]
    fn phase_3_rows_are_byte_identical_to_their_shipped_values() {
        for (name, status, code, message) in PHASE_3_FROZEN_ROWS {
            let error = variant(name);

            assert_eq!(error.status_code(), status, "status for {name}");
            assert_eq!(error.code(), code, "code for {name}");
            assert_eq!(error.message(), message, "message for {name}");
            // `Display` is what `thiserror` renders; it must not drift from
            // `message()` or the two paths would disagree.
            assert_eq!(error.to_string(), message, "Display for {name}");
        }
    }

    #[test]
    fn provider_rows_use_fixed_messages_with_no_interpolation() {
        for (name, _, _, message) in ALL_ROWS.iter().filter(|(name, _, _, _)| {
            name.starts_with("Provider")
        }) {
            let error = variant(name);

            assert_eq!(error.message(), *message);
            assert_eq!(error.to_string(), *message);
        }
    }

    #[test]
    fn every_failure_category_maps_to_its_documented_row() {
        use crate::application::chat::ProviderFailure;

        let cases = [
            (ProviderFailure::Unreachable, 502, "provider_unavailable"),
            (ProviderFailure::Refused, 502, "provider_unavailable"),
            (ProviderFailure::DeadlineExceeded, 504, "provider_timeout"),
            (ProviderFailure::UnusableResponse, 500, "internal_error"),
            (ProviderFailure::InvalidResponse, 500, "internal_error"),
        ];

        for (failure, status, code) in cases {
            let error = ApiError::from_provider_failure(failure);

            assert_eq!(error.status_code().as_u16(), status, "status for {failure:?}");
            assert_eq!(error.code(), code, "code for {failure:?}");
        }
    }

    #[test]
    fn no_failure_category_maps_to_a_validation_row() {
        use crate::application::chat::ProviderFailure;

        let all = [
            ProviderFailure::Unreachable,
            ProviderFailure::Refused,
            ProviderFailure::DeadlineExceeded,
            ProviderFailure::UnusableResponse,
            ProviderFailure::InvalidResponse,
        ];

        for failure in all {
            let error = ApiError::from_provider_failure(failure);

            assert_ne!(error.code(), "invalid_request", "{failure:?} leaked a validation code");
            assert_ne!(error.code(), "unsupported_feature", "{failure:?} leaked a feature code");
        }
    }

    #[test]
    fn provider_failure_mapping_is_total_over_every_category() {
        // Compile-time exhaustiveness: adding a sixth category without mapping it
        // fails to build here rather than silently falling back at runtime.
        use crate::application::chat::ProviderFailure;

        fn assert_mapped(failure: ProviderFailure) -> ApiError {
            ApiError::from_provider_failure(failure)
        }

        let mapped = [
            assert_mapped(ProviderFailure::Unreachable),
            assert_mapped(ProviderFailure::Refused),
            assert_mapped(ProviderFailure::DeadlineExceeded),
            assert_mapped(ProviderFailure::UnusableResponse),
            assert_mapped(ProviderFailure::InvalidResponse),
        ];

        assert_eq!(mapped.len(), 5);
    }
}
