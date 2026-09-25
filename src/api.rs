//! API layer (placeholder): HTTP/transport exposure.
//!
//! Will own request/response handling and routing. May depend on
//! `application` and `config`, never on `infrastructure`.
//! See `specs/002-layered-architecture/contracts/layout.md`.

pub mod chat;
pub mod dto;
pub mod error;
pub mod health;
pub mod middleware;
pub mod server;
