//! Small enums mirrored 1:1 from Postgres types in `IMPLEMENTATION_PLAN.md`
//! §3.1, kept separate from the RBAC vocabulary in `auth.rs` since they are
//! entity lifecycle states, not permissions.

use serde::{Deserialize, Serialize};

/// Mirrors `user_status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Inactive,
    Suspended,
}

/// Mirrors `entity_status` — used by `lscs`, `programs`, `semesters`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "entity_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EntityStatus {
    Active,
    Inactive,
}

/// Mirrors `enrollment_status` — `student_courses.status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "enrollment_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EnrollmentStatus {
    Active,
    Completed,
    Dropped,
}

/// Mirrors `session_status` — `learning_sessions.status` (Phase 3/4 writes
/// this; the type is defined now so `crates/db`'s struct-only model for
/// `learning_sessions` can use it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "session_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    InProgress,
    Completed,
    Abandoned,
}

impl UserStatus {
    /// The `user_status` label this maps to in Postgres.
    pub const fn as_db_str(self) -> &'static str {
        match self {
            UserStatus::Active => "active",
            UserStatus::Inactive => "inactive",
            UserStatus::Suspended => "suspended",
        }
    }
}

impl EntityStatus {
    /// The `entity_status` label this maps to in Postgres.
    pub const fn as_db_str(self) -> &'static str {
        match self {
            EntityStatus::Active => "active",
            EntityStatus::Inactive => "inactive",
        }
    }
}

impl EnrollmentStatus {
    /// The `enrollment_status` label this maps to in Postgres.
    pub const fn as_db_str(self) -> &'static str {
        match self {
            EnrollmentStatus::Active => "active",
            EnrollmentStatus::Completed => "completed",
            EnrollmentStatus::Dropped => "dropped",
        }
    }
}

/// Mirrors `exam_status` — `exams.status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "exam_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ExamStatus {
    Draft,
    Published,
    Archived,
}

/// Mirrors `exam_attempt_status` — `exam_attempts.status`. `Graded` is the
/// only state in which `score` is guaranteed present; the schema enforces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "exam_attempt_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ExamAttemptStatus {
    InProgress,
    Submitted,
    Graded,
    Abandoned,
}

impl ExamStatus {
    /// The `exam_status` label this maps to in Postgres.
    pub const fn as_db_str(self) -> &'static str {
        match self {
            ExamStatus::Draft => "draft",
            ExamStatus::Published => "published",
            ExamStatus::Archived => "archived",
        }
    }
}

impl ExamAttemptStatus {
    /// The `exam_attempt_status` label this maps to in Postgres.
    pub const fn as_db_str(self) -> &'static str {
        match self {
            ExamAttemptStatus::InProgress => "in_progress",
            ExamAttemptStatus::Submitted => "submitted",
            ExamAttemptStatus::Graded => "graded",
            ExamAttemptStatus::Abandoned => "abandoned",
        }
    }
}
