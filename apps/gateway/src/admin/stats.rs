//! `GET /api/v1/admin/stats` — the dashboard's "at a glance" counts.
//!
//! A sub-admin must never see a platform-wide total: the numbers returned to
//! one are restricted to its `sub_admin_scopes` programs, which is why the
//! role decides *which query runs* rather than being a filter applied to the
//! result. `lscs` is the one figure that stays platform-wide in both cases —
//! an LSC has no program (see `admin/lscs.rs`) and every admin can already
//! list them all, so scoping that count would misreport, not protect.

use axum::{extract::State, Json};
use serde::Serialize;

use dg_core::{Capability, PublicError, Role};
use dg_db::models::stats;

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct DocumentCounts {
    pub total: i64,
    pub pending_review: i64,
    pub embedded: i64,
    pub failed: i64,
}

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub programs: i64,
    pub semesters: i64,
    pub courses: i64,
    pub blocks: i64,
    pub lscs: i64,
    pub students: i64,
    pub documents: DocumentCounts,
}

impl From<stats::AdminStats> for StatsResponse {
    fn from(s: stats::AdminStats) -> Self {
        Self {
            programs: s.programs,
            semesters: s.semesters,
            courses: s.courses,
            blocks: s.blocks,
            lscs: s.lscs,
            students: s.students,
            documents: DocumentCounts {
                total: s.documents_total,
                pending_review: s.documents_pending_review,
                embedded: s.documents_embedded,
                failed: s.documents_failed,
            },
        }
    }
}

pub async fn get_stats(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
) -> Result<Json<StatsResponse>, PublicError> {
    actor.require(Capability::ManagePrograms)?;

    let counts = match actor.role {
        Role::SuperAdmin => stats::counts_all(&state.pool).await,
        Role::SubAdmin => stats::counts_scoped(&state.pool, &actor.scopes).await,
        // Unreachable via the capability check above; matched explicitly so
        // a future capability change cannot silently hand a student the
        // unscoped query.
        Role::Student => return Err(PublicError::Forbidden),
    }
    .map_err(PublicError::from)?;

    Ok(Json(counts.into()))
}

#[cfg(test)]
mod tests {
    use dg_core::{Actor, Capability, ProgramId, Role, UserId};

    /// Which query the handler will run for a given actor — the decision
    /// that keeps platform-wide totals away from a scoped sub-admin.
    #[derive(Debug, PartialEq, Eq)]
    enum Query {
        All,
        Scoped,
        Forbidden,
    }

    fn plan(actor: &Actor) -> Query {
        if actor.require(Capability::ManagePrograms).is_err() {
            return Query::Forbidden;
        }
        match actor.role {
            Role::SuperAdmin => Query::All,
            Role::SubAdmin => Query::Scoped,
            Role::Student => Query::Forbidden,
        }
    }

    #[test]
    fn super_admin_gets_platform_wide_counts() {
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert_eq!(plan(&actor), Query::All);
    }

    #[test]
    fn sub_admin_gets_counts_scoped_to_its_programs() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        assert_eq!(plan(&actor), Query::Scoped);
    }

    #[test]
    fn sub_admin_with_no_scopes_still_gets_the_scoped_query() {
        // Not the unscoped one: no scopes means zeroes, never everything.
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![]);
        assert_eq!(plan(&actor), Query::Scoped);
    }

    #[test]
    fn student_is_forbidden_stats() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert_eq!(plan(&actor), Query::Forbidden);
    }
}
