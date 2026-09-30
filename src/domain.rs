//! Domain layer: provider-independent core concepts.
//!
//! Owns the chat and catalog concepts. Depends on nothing; other layers may
//! depend on it. See `specs/002-layered-architecture/contracts/layout.md`.

pub mod catalog;
pub mod chat;

pub use catalog::{Model, Provider};
pub use chat::{
    ChatRequest, ChatResponse, MAX_MAX_TOKENS, MAX_TEMPERATURE, MIN_MAX_TOKENS, MIN_TEMPERATURE,
    Message, MessageRole, Usage, is_max_tokens_in_range, is_temperature_in_range,
};
