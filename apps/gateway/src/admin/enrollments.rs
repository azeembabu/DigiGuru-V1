//! `GET /api/v1/admin/enrollments` — the records behind the dashboard's
//! enrolment tiles.
//!
//! Follows the paginated-list conventions the other admin lists already use: a
//! bare JSON array body, the total before pagination in `X-Total-Count`, and
//! `limit`/`offset` validated rather than clamped (`super::page`).
//!
//! Scoping follows `analytics.rs` exactly — the caller's **role** selects the
//! scoped or the unscoped query, never a filter applied afterwards to a
//! platform-wide result, so a sub-admin cannot be handed rows outside its
//! programs even momentarily.

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::{Capability, EnrollmentStatus, PublicError, Role};
use dg_db::models::drilldown::{self, EnrollmentFilters};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

/// One enrolment, denormalised so a table renders without a second request
/// per row.
#[derive(Debug, Serialize)]
pub struct EnrollmentResponse {
    pub id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub roll_number: String,
    pub course_id: Uuid,
    pub course_code: String,
    pub course_name: String,
    pub semester_number: i16,
    pub status: EnrollmentStatus,
    pub assigned_at: DateTime<Utc>,
}

impl From<drilldown::EnrollmentRow> for EnrollmentResponse {
    fn from(r: drilldown::EnrollmentRow) -> Self {
        Self {
            id: r.id,
            student_id: r.student_id.into_uuid(),
            student_name: r.student_name,
            roll_number: r.roll_number,
            course_id: r.course_id.into_uuid(),
            course_code: r.course_code,
            course_name: r.course_name,
            semester_number: r.semester_number,
            status: r.status,
            assigned_at: r.assigned_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListEnrollmentsQuery {
    /// Case-insensitive substring over student name, roll number, course code
    /// and course name.
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub status: Option<EnrollmentStatus>,
    #[serde(default)]
    pub course_id: Option<Uuid>,
    #[serde(default)]
    pub student_id: Option<Uuid>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

pub async fn list_enrollments(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<ListEnrollmentsQuery>,
) -> Result<(HeaderMap, Json<Vec<EnrollmentResponse>>), PublicError> {
    actor.require(Capability::ManagePrograms)?;

    let page = super::page(query.limit, query.offset, 50)?;
    let filters = EnrollmentFilters {
        q: super::search_term(query.q.as_deref()),
        status: query.status,
        course_id: query.course_id,
        student_id: query.student_id,
    };

    let (total, rows) = match actor.role {
        Role::SuperAdmin => (
            drilldown::count_enrollments_all(&state.pool, filters).await,
            drilldown::enrollments_all(&state.pool, filters, page.limit, page.offset).await,
        ),
        Role::SubAdmin => (
            drilldown::count_enrollments_scoped(&state.pool, &actor.scopes, filters).await,
            drilldown::enrollments_scoped(
                &state.pool,
                &actor.scopes,
                filters,
                page.limit,
                page.offset,
            )
            .await,
        ),
        // Unreachable via the capability check above; matched explicitly so a
        // future capability change cannot silently hand a student the
        // unscoped query.
        Role::Student => return Err(PublicError::Forbidden),
    };

    let total = total.map_err(PublicError::from)?;
    let rows = rows.map_err(PublicError::from)?;

    Ok((
        super::total_count(total),
        Json(rows.into_iter().map(EnrollmentResponse::from).collect()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dg_core::{Actor, ProgramId, UserId};

    /// Which query the handler will run for a given actor — the decision that
    /// keeps rows outside a sub-admin's programs away from it.
    #[derive(Debug, PartialEq, Eq)]
    enum Plan {
        All,
        Scoped,
        Forbidden,
    }

    fn plan(actor: &Actor) -> Plan {
        if actor.require(Capability::ManagePrograms).is_err() {
            return Plan::Forbidden;
        }
        match actor.role {
            Role::SuperAdmin => Plan::All,
            Role::SubAdmin => Plan::Scoped,
            Role::Student => Plan::Forbidden,
        }
    }

    #[test]
    fn super_admin_sees_platform_wide_enrollments() {
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert_eq!(plan(&actor), Plan::All);
    }

    #[test]
    fn sub_admin_enrollments_are_scoped_to_its_programs() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        assert_eq!(plan(&actor), Plan::Scoped);
    }

    #[test]
    fn sub_admin_with_no_scopes_still_gets_the_scoped_enrollments_query() {
        // An empty scope set means an empty page, never a platform-wide one.
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![]);
        assert_eq!(plan(&actor), Plan::Scoped);
    }

    #[test]
    fn student_is_forbidden_enrollments() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert_eq!(plan(&actor), Plan::Forbidden);
    }

    #[test]
    fn limit_above_the_ceiling_is_a_validation_error() {
        let err = super::super::page(Some(201), None, 50).expect_err("over the ceiling");
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }
}
