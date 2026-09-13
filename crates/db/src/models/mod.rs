//! sqlx models and query modules, one module per table.
//!
//! Per the Phase 1 scope: `users`, `admins`, `lscs`, `programs`, `semesters`,
//! `courses`, `blocks`, `students`, `student_courses`, `sub_admin_scopes`,
//! `auth_sessions`, `password_resets`, and `audit_logs` get full query
//! modules. The later-phase tables (`documents`, `learning_sessions`,
//! `safety_incidents`, `board_events`, `note_reminders`) get struct
//! definitions only, in `content.rs`, so shared typing exists before the
//! phases that write to them land.

pub mod admins;
pub mod analytics;
pub mod audit_logs;
pub mod auth_sessions;
pub mod blocks;
pub mod content;
pub mod courses;
pub mod documents;
pub mod drilldown;
pub mod drilldown_sessions;
pub mod exams;
pub mod ingestion_jobs;
pub mod lscs;
pub mod password_resets;
pub mod programs;
pub mod semesters;
pub mod stats;
pub mod student_courses;
pub mod students;
pub mod sub_admin_scopes;
pub mod uploads;
pub mod users;
