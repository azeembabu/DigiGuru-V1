//! `GET /api/v1/admin/board-events` — the whiteboard audit log behind the
//! dashboard's ACK-latency chart (NN-1).
//!
//! Same conventions as every other paginated admin list, and the same
//! role-selects-the-query scoping as `analytics.rs`, walking
//! `board_events -> learning_sessions -> courses -> program_id`.
//!
//! Ordering is deliberately conditional: with `session_id` given the page is a
//! transcript of one session (`turn_seq` ascending), otherwise it is a
//! reverse-chronological feed.

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use dg_core::{Capability, PublicError, Role};
use dg_db::models::drilldown_sessions::{self as drill, BoardEventFilters};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

/// The ACK-latency buckets, identical to the analytics chart's so a click on a
/// bar filters to exactly the rows it counted.
#[derive(Debug, Clone, Copy, Deserialize)]
pub enum AckBucket {
    #[serde(rename = "0-100ms")]
    Under100,
    #[serde(rename = "100-250ms")]
    Under250,
    #[serde(rename = "250-400ms")]
    Under400,
    #[serde(rename = ">400ms")]
    Over400,
}

impl AckBucket {
    const fn as_label(self) -> &'static str {
        match self {
            AckBucket::Under100 => "0-100ms",
            AckBucket::Under250 => "100-250ms",
            AckBucket::Under400 => "250-400ms",
            AckBucket::Over400 => ">400ms",
        }
    }
}

/// One board event. `op_kind` is lifted out of `op` so a list renders without
/// parsing the payload; `op` carries the full validated op for a detail view.
#[derive(Debug, Serialize)]
pub struct BoardEventResponse {
    pub id: i64,
    pub session_id: Uuid,
    pub turn_seq: i32,
    pub op_kind: String,
    pub op: Value,
    pub emitted_at: DateTime<Utc>,
    pub acked_ms: Option<i32>,
    pub is_violation: bool,
}

impl From<drill::BoardEventRow> for BoardEventResponse {
    fn from(r: drill::BoardEventRow) -> Self {
        Self {
            id: r.id,
            session_id: r.session_id.into_uuid(),
            turn_seq: r.turn_seq,
            op_kind: r.op_kind,
            op: r.op,
            emitted_at: r.emitted_at,
            acked_ms: r.acked_ms,
            is_violation: r.is_violation,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListBoardEventsQuery {
    #[serde(default)]
    pub session_id: Option<Uuid>,
    /// `true` keeps only ops that were never acked or acked after `HOLD_MAX`.
    #[serde(default)]
    pub violations_only: Option<bool>,
    #[serde(default)]
    pub bucket: Option<AckBucket>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

/// An omitted `violations_only` means "no filter", the same as an explicit
/// `false` — a drill-down link that simply drops the parameter must not
/// silently narrow the list to violations.
fn violations_only_filter(flag: Option<bool>) -> bool {
    flag.unwrap_or(false)
}

pub async fn list_board_events(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<ListBoardEventsQuery>,
) -> Result<(HeaderMap, Json<Vec<BoardEventResponse>>), PublicError> {
    actor.require(Capability::ManagePrograms)?;

    let page = super::page(query.limit, query.offset, 50)?;
    let filters = BoardEventFilters {
        session_id: query.session_id,
        violations_only: violations_only_filter(query.violations_only),
        bucket: query.bucket.map(AckBucket::as_label),
    };

    let (total, rows) = match actor.role {
        Role::SuperAdmin => (
            drill::count_board_events_all(&state.pool, filters).await,
            drill::board_events_all(&state.pool, filters, page.limit, page.offset).await,
        ),
        Role::SubAdmin => (
            drill::count_board_events_scoped(&state.pool, &actor.scopes, filters).await,
            drill::board_events_scoped(
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
        Json(rows.into_iter().map(BoardEventResponse::from).collect()),
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
    fn super_admin_sees_platform_wide_board_events() {
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert_eq!(plan(&actor), Plan::All);
    }

    #[test]
    fn sub_admin_board_events_are_scoped_to_its_programs() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        assert_eq!(plan(&actor), Plan::Scoped);
    }

    #[test]
    fn student_is_forbidden_board_events() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert_eq!(plan(&actor), Plan::Forbidden);
    }

    #[test]
    fn bucket_labels_match_the_analytics_chart() {
        assert_eq!(AckBucket::Under100.as_label(), "0-100ms");
        assert_eq!(AckBucket::Under250.as_label(), "100-250ms");
        assert_eq!(AckBucket::Under400.as_label(), "250-400ms");
        assert_eq!(AckBucket::Over400.as_label(), ">400ms");
    }

    #[test]
    fn an_omitted_violations_only_is_no_filter() {
        // The query builder defaults the flag off, so an absent parameter and
        // an explicit `false` must reach the SQL identically.
        assert!(!violations_only_filter(None));
        assert!(!violations_only_filter(Some(false)));
        assert!(violations_only_filter(Some(true)));
    }

    #[test]
    fn an_unknown_bucket_label_is_rejected_at_the_query_boundary() {
        let parsed = serde_json::from_str::<AckBucket>("\"soon\"");
        assert!(parsed.is_err());
        let valid = serde_json::from_str::<AckBucket>("\">400ms\"").expect("a real bucket");
        assert_eq!(valid.as_label(), ">400ms");
    }

    #[test]
    fn the_violation_threshold_is_the_nn1_hold_max() {
        assert_eq!(drill::HOLD_MAX_MS, 400);
    }
}
