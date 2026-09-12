//! RBAC vocabulary: `Role`, `Capability`, and `Actor`.
//!
//! Authorization is capability-based, not role-based: every handler declares
//! the `Capability` it requires (see `.claude/rules/security.md` — "no role
//! check inside business logic"). `Role` only decides which capabilities a
//! given actor holds; scoping (which programs a sub-admin may touch) is
//! carried on `Actor` and checked separately from the capability itself, via
//! `Actor::require_scoped`.

use crate::error::PublicError;
use crate::ids::{ProgramId, UserId};
use serde::{Deserialize, Serialize};

/// Mirrors the Postgres `user_role` enum (see `IMPLEMENTATION_PLAN.md` §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Role {
    SuperAdmin,
    SubAdmin,
    Student,
}

/// A single, named permission a handler can require of the caller.
///
/// Deliberately flat — scoping (e.g. "manage programs, but only within
/// `sub_admin_scopes`") is not encoded in the variant. A handler checks the
/// capability via `Actor::require`/`Actor::require_scoped`; there is no
/// implicit allow anywhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    /// Create/update users and assign roles. Super-admin only.
    ManageUsers,
    /// CRUD on `programs`, `semesters`, `courses`, `lscs`. Scoped to
    /// `sub_admin_scopes` for sub-admins; unrestricted for super-admins.
    ManagePrograms,
    /// CRUD on `students` and `student_courses`. Scoped the same way.
    ManageStudents,
    /// Upload documents for ingestion. The Phase 2 pipeline consumes this;
    /// the capability exists now so the RBAC surface is complete. Scoped the
    /// same way as `ManagePrograms`.
    UploadDocuments,
    /// Read one's own `/me/context` payload.
    ViewOwnContext,
    /// `PATCH /me/profile` — self-service, allow-listed fields only.
    UpdateOwnProfile,
    /// List/revoke one's own `auth_sessions`.
    ManageOwnSessions,
}

/// The authenticated caller of a request, resolved once per request from a
/// validated access-token JWT (see `apps/gateway/src/extractors`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub user_id: UserId,
    pub role: Role,
    /// Programs this actor may act on. Always empty for `SuperAdmin` (no
    /// scoping needed) and `Student` (students never hold scoped
    /// capabilities); populated from `sub_admin_scopes` for `SubAdmin`.
    pub scopes: Vec<ProgramId>,
}

impl Actor {
    pub fn new(user_id: UserId, role: Role, scopes: Vec<ProgramId>) -> Self {
        Self { user_id, role, scopes }
    }

    /// Whether this actor's role holds `cap` at all. Does not check scope —
    /// call `in_scope`/`require_scoped` for a `Capability` that is
    /// program-scoped (`ManagePrograms`, `ManageStudents`, `UploadDocuments`).
    pub fn can(&self, cap: Capability) -> bool {
        use Capability::*;
        use Role::*;
        match (self.role, cap) {
            (SuperAdmin, _) => true,

            (SubAdmin, ManagePrograms | ManageStudents | UploadDocuments) => true,
            (SubAdmin, ViewOwnContext | UpdateOwnProfile | ManageOwnSessions) => true,
            (SubAdmin, ManageUsers) => false,

            (Student, ViewOwnContext | UpdateOwnProfile | ManageOwnSessions) => true,
            (Student, ManageUsers | ManagePrograms | ManageStudents | UploadDocuments) => false,
        }
    }

    /// For a scoped capability, whether this actor may act on `program_id`.
    /// Super-admins pass unconditionally; sub-admins must have the program in
    /// `scopes`; students never hold scoped capabilities so always fail.
    pub fn in_scope(&self, program_id: ProgramId) -> bool {
        match self.role {
            Role::SuperAdmin => true,
            Role::SubAdmin => self.scopes.contains(&program_id),
            Role::Student => false,
        }
    }

    /// Require `cap`, unscoped. Returns `PublicError::Forbidden` if the
    /// actor's role does not hold it.
    pub fn require(&self, cap: Capability) -> Result<(), PublicError> {
        if self.can(cap) {
            Ok(())
        } else {
            Err(PublicError::Forbidden)
        }
    }

    /// Require `cap` scoped to `program_id`. Fails closed: the capability
    /// must be held *and* the program must be in scope, so a sub-admin can
    /// never read or write outside `sub_admin_scopes`.
    pub fn require_scoped(&self, cap: Capability, program_id: ProgramId) -> Result<(), PublicError> {
        if self.can(cap) && self.in_scope(program_id) {
            Ok(())
        } else {
            Err(PublicError::Forbidden)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn student_cannot_manage_programs() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert!(!actor.can(Capability::ManagePrograms));
        assert!(actor.require(Capability::ManagePrograms).is_err());
    }

    #[test]
    fn student_can_view_own_context() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert!(actor.require(Capability::ViewOwnContext).is_ok());
    }

    #[test]
    fn sub_admin_scoped_to_own_programs_only() {
        let scoped = ProgramId::new();
        let other = ProgramId::new();
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![scoped]);
        assert!(actor.require_scoped(Capability::ManagePrograms, scoped).is_ok());
        assert!(actor.require_scoped(Capability::ManagePrograms, other).is_err());
    }

    #[test]
    fn sub_admin_cannot_manage_users() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        assert!(actor.require(Capability::ManageUsers).is_err());
    }

    #[test]
    fn super_admin_bypasses_scope() {
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert!(actor
            .require_scoped(Capability::ManagePrograms, ProgramId::new())
            .is_ok());
    }
}
