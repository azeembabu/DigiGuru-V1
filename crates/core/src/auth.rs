//! RBAC vocabulary: `Actor` (who is making the request) and `Capability`
//! (what they are trying to do). Every handler will declare its required
//! capability; there is no implicit authorization (see `CLAUDE.md`).
//!
//! This module currently holds types only.

use uuid::Uuid;

/// The authenticated caller of a request, resolved from a validated JWT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Actor {
    /// Full platform access.
    SuperAdmin,
    /// Access scoped to a set of `programs.id` via `sub_admin_scopes`.
    SubAdmin { scopes: Vec<Uuid> },
    /// A single student, identified by `students.id`.
    Student { id: Uuid },
}

/// A single, named permission a handler can require of the `Actor`.
///
/// Kept intentionally coarse for now; the capability-check extractor (below)
/// will map `(Actor, Capability)` pairs to allow/deny, including sub-admin
/// program scoping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    ManageUsers,
    ManagePrograms,
    ManageStudents,
    UploadContent,
    ViewOwnContext,
    JoinClassroom,
}

// TODO: capability-check extractor — an axum `FromRequestParts` that pulls
// the `Actor` from request state (set by the JWT auth middleware), checks it
// against a handler-declared `Capability`, and rejects with
// `PublicError::Forbidden` / `PublicError::Unauthorized` on failure. Must
// also enforce sub-admin `scopes` against the resource's `program_id`.
