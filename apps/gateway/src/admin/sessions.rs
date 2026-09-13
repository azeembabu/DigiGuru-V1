//! `GET /api/v1/admin/sessions` — the classroom sessions behind the
//! dashboard's session tiles.
//!
//! Same conventions as every other paginated admin list (bare array body,
//! `X-Total-Count`, validated `limit`/`offset`) and the same role-selects-the-
//! query scoping as `analytics.rs`.
//!
//! Each row carries its whiteboard roll-ups (`board_ops`, `board_violations`)
//! so a bad session is visible in the list without opening it. A violation is
//! an op acked later than the NN-1 `HOLD_MAX` of 400 ms, or never acked.

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::{Capability, PublicError, Role, SessionStatus};
use dg_db::models::drilldown_sessions::{self as drill, SessionFilters};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

/// `learning_sessions.end_reason`, typed at the query boundary so an unknown
/// value is rejected rather than quietly matching nothing.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    Quota,
    Idle,
    User,
    Jailbreak,
    Error,
}

impl EndReason {
    const fn as_db_str(self) -> &'static str {
        match self {
            EndReason::Quota => "quota",
            EndReason::Idle => "idle",
            EndReason::User => "user",
            EndReason::Jailbreak => "jailbreak",
            EndReason::Error => "error",
        }
    }
}

/// One session row, denormalised across student, course and block.
#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub roll_number: String,
    pub course_id: Uuid,
    pub course_code: String,
    pub block_id: Uuid,
    pub block_no: i16,
    pub block_title: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    /// NN-3 server-authoritative active voice time; never above 1 200 000.
    pub active_voice_ms: i64,
    pub status: SessionStatus,
    /// `null` while the session is still running.
    pub end_reason: Option<String>,
    pub last_topic: Option<String>,
    pub last_page: i32,
    pub board_ops: i64,
    pub board_violations: i64,
}

impl From<drill::SessionRow> for SessionResponse {
    fn from(r: drill::SessionRow) -> Self {
        Self {
            id: r.id.into_uuid(),
            student_id: r.student_id.into_uuid(),
            student_name: r.student_name,
            roll_number: r.roll_number,
            course_id: r.course_id.into_uuid(),
            course_code: r.course_code,
            block_id: r.block_id.into_uuid(),
            block_no: r.block_no,
            block_title: r.block_title,
            started_at: r.started_at,
            ended_at: r.ended_at,
            active_voice_ms: r.active_voice_ms,
            status: r.status,
            end_reason: r.end_reason,
            last_topic: r.last_topic,
            last_page: r.last_page,
            board_ops: r.board_ops,
            board_violations: r.board_violations,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListSessionsQuery {
    #[serde(default)]
    pub status: Option<SessionStatus>,
    #[serde(default)]
    pub end_reason: Option<EndReason>,
    #[serde(default)]
    pub student_id: Option<Uuid>,
    #[serde(default)]
    pub block_id: Option<Uuid>,
    #[serde(default)]
    pub course_id: Option<Uuid>,
    /// Inclusive RFC3339 bounds on `started_at`.
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

/// Parse an optional RFC3339 timestamp query parameter.
///
/// A malformed value is the caller's mistake, so it is a `400
/// VALIDATION_ERROR` naming the offending field — never a `500` from a
/// parse failure deeper in the handler. Shared with `safety_incidents.rs`,
/// which takes the same `from`/`to` pair.
pub(crate) fn timestamp(
    field: &'static str,
    raw: Option<&str>,
) -> Result<Option<DateTime<Utc>>, PublicError> {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| Some(dt.with_timezone(&Utc)))
        .map_err(|_| PublicError::validation(field, "must be an RFC3339 timestamp"))
}

pub async fn list_sessions(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<ListSessionsQuery>,
) -> Result<(HeaderMap, Json<Vec<SessionResponse>>), PublicError> {
    actor.require(Capability::ManagePrograms)?;

    let page = super::page(query.limit, query.offset, 50)?;
    let filters = SessionFilters {
        status: query.status,
        end_reason: query.end_reason.map(EndReason::as_db_str),
        student_id: query.student_id,
        block_id: query.block_id,
        course_id: query.course_id,
        from: timestamp("from", query.from.as_deref())?,
        to: timestamp("to", query.to.as_deref())?,
    };

    let (total, rows) = match actor.role {
        Role::SuperAdmin => (
            drill::count_sessions_all(&state.pool, filters).await,
            drill::sessions_all(&state.pool, filters, page.limit, page.offset).await,
        ),
        Role::SubAdmin => (
            drill::count_sessions_scoped(&state.pool, &actor.scopes, filters).await,
            drill::sessions_scoped(&state.pool, &actor.scopes, filters, page.limit, page.offset)
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
        Json(rows.into_iter().map(SessionResponse::from).collect()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dg_core::{Actor, ProgramId, UserId};

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
    fn super_admin_sees_platform_wide_sessions() {
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert_eq!(plan(&actor), Plan::All);
    }

    #[test]
    fn sub_admin_sessions_are_scoped_to_its_programs() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        assert_eq!(plan(&actor), Plan::Scoped);
    }

    #[test]
    fn sub_admin_with_no_scopes_still_gets_the_scoped_sessions_query() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![]);
        assert_eq!(plan(&actor), Plan::Scoped);
    }

    #[test]
    fn student_is_forbidden_sessions() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert_eq!(plan(&actor), Plan::Forbidden);
    }

    #[test]
    fn malformed_from_is_a_validation_error() {
        let err = timestamp("from", Some("yesterday")).expect_err("not RFC3339");
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn absent_or_blank_timestamp_is_no_filter() {
        assert!(timestamp("from", None).expect("absent is fine").is_none());
        assert!(timestamp("to", Some("  ")).expect("blank is fine").is_none());
    }

    #[test]
    fn rfc3339_timestamp_is_normalised_to_utc() {
        let parsed = timestamp("from", Some("2026-09-13T10:30:00+05:30"))
            .expect("valid RFC3339")
            .expect("a bound");
        assert_eq!(parsed.to_rfc3339(), "2026-09-13T05:00:00+00:00");
    }

    #[test]
    fn end_reason_maps_to_its_database_label() {
        assert_eq!(EndReason::Jailbreak.as_db_str(), "jailbreak");
        assert_eq!(EndReason::Quota.as_db_str(), "quota");
    }
}
