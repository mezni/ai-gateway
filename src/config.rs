//! Configuration layer (placeholder): configuration types and loading.
//!
//! Will own configuration parsing and environment handling. May depend on
//! `domain`, never on `api` or `infrastructure`.
//! See `specs/002-layered-architecture/contracts/layout.md`.

pub mod server;

pub use server::ServerConfig;
