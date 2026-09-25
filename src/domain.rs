//! Domain layer: provider-independent core concepts.
//!
//! Owns the chat and catalog concepts. Depends on nothing; other layers may
//! depend on it. See `specs/002-layered-architecture/contracts/layout.md`.

pub mod catalog;
pub mod chat;

pub use catalog::{Model, Provider};
pub use chat::{ChatRequest, ChatResponse, Message, MessageRole, Usage};
