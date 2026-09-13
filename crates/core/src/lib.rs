//! `core` — shared domain types for Digi Guru: newtype entity ids, the
//! `Actor`/`Role`/`Capability` RBAC vocabulary, the `Error`/`PublicError`
//! error split, the "Remember Me" context payload types, and process
//! configuration.
//!
//! This crate holds types (and pure logic over them) only. No database
//! access, no HTTP handlers — those live in `dg-db` and `apps/gateway`.

pub mod auth;
pub mod config;
pub mod context;
pub mod domain;
pub mod error;
pub mod ids;

pub use auth::{Actor, Capability, Role};
pub use domain::{
    EntityStatus, EnrollmentStatus, ExamAttemptStatus, ExamStatus, SessionStatus, UserStatus,
};
pub use error::{Error, FieldError, PublicError};
pub use ids::{
    BlockId, CourseId, DocumentId, ExamAttemptId, ExamId, LscId, ProgramId, SemesterId, SessionId,
    StudentId, UserId,
};
