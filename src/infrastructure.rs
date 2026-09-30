//! Infrastructure layer: adapters and external integrations.
//!
//! Owns provider clients, persistence, and other external system access.
//! May depend on `domain` and `config`, never on `api`.
//! See `specs/002-layered-architecture/contracts/layout.md`.

pub mod providers;
