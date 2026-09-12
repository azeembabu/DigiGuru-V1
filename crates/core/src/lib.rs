//! `core` — shared domain types for Digi Guru: the client-facing error type,
//! the `Actor`/`Capability` RBAC vocabulary, and process configuration.
//!
//! This crate holds types only. No database access, no HTTP handlers.

pub mod auth;
pub mod config;
pub mod error;

pub use error::PublicError;
