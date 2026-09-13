//! `GET /api/v1/admin/safety-incidents` — the guardrail hits behind the
//! dashboard's safety tiles (NN-5).
//!
//! Same conventions as every other paginated admin list, and the same
//! role-selects-the-query scoping as `analytics.rs`, walking
//! `safety_incidents -> students -> program_id`.
//!
//! `excerpt` is returned exactly as stored: it is PII-redacted at write time
//! (`.claude/rules/security.md`), and this endpoint neither re-redacts nor
//! un-redacts it.

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::{Capability, PublicError, Role};
use dg_db::models::drilldown::{self, SafetyIncidentFilters};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

/// The highest guardrail tier: 0 = pre-LLM tap, 1 = transcript, 2 = output.
const MAX_TIER: i16 = 2;

/// `safety_incidents.kind`, typed at the query boundary so an unknown value is
/// rejected rather than quietly matching nothing.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentKind {
    Jailbreak,
    Toxicity,
    OutOfScope,
}

impl IncidentKind {
    const fn as_db_str(self) -> &'static str {
        match self {
            IncidentKind::Jailbreak => "jailbreak",
            IncidentKind::Toxicity => "toxicity",
            IncidentKind::OutOfScope => "out_of_scope",
        }
    }
}

/// One incident, denormalised with the student it belongs to.
#[derive(Debug, Serialize)]
pub struct SafetyIncidentResponse {
    pub id: Uuid,
    pub session_id: Option<Uuid>,
    pub student_id: Uuid,
    pub student_name: String,
    pub roll_number: String,
    pub kind: String,
    pub tier: i16,
    pub excerpt: String,
    pub created_at: DateTime<Utc>,
}

impl From<drilldown::SafetyIncidentRow> for SafetyIncidentResponse {
    fn from(r: drilldown::SafetyIncidentRow) -> Self {
        Self {
            id: r.id,
            session_id: r.session_id.map(dg_core::SessionId::into_uuid),
            student_id: r.student_id.into_uuid(),
            student_name: r.student_name,
            roll_number: r.roll_number,
            kind: r.kind,
            tier: r.tier,
            excerpt: r.excerpt,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListIncidentsQuery {
    #[serde(default)]
    pub tier: Option<i16>,
    #[serde(default)]
    pub kind: Option<IncidentKind>,
    #[serde(default)]
    pub student_id: Option<Uuid>,
    #[serde(default)]
    pub session_id: Option<Uuid>,
    /// Inclusive RFC3339 bounds on `created_at`.
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

/// A tier outside `0..=2` names no guardrail at all, so it is a caller
/// mistake worth reporting rather than a filter that silently matches nothing.
fn tier(raw: Option<i16>) -> Result<Option<i16>, PublicError> {
    match raw {
        Some(t) if !(0..=MAX_TIER).contains(&t) => Err(PublicError::validation(
            "tier",
            format!("must be between 0 and {MAX_TIER}"),
        )),
        other => Ok(other),
    }
}

pub async fn list_safety_incidents(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<ListIncidentsQuery>,
) -> Result<(HeaderMap, Json<Vec<SafetyIncidentResponse>>), PublicError> {
    actor.require(Capability::ManagePrograms)?;

    let page = super::page(query.limit, query.offset, 50)?;
    let filters = SafetyIncidentFilters {
        tier: tier(query.tier)?,
        kind: query.kind.map(IncidentKind::as_db_str),
        student_id: query.student_id,
        session_id: query.session_id,
        from: super::sessions::timestamp("from", query.from.as_deref())?,
        to: super::sessions::timestamp("to", query.to.as_deref())?,
    };

    let (total, rows) = match actor.role {
        Role::SuperAdmin => (
            drilldown::count_incidents_all(&state.pool, filters).await,
            drilldown::incidents_all(&state.pool, filters, page.limit, page.offset).await,
        ),
        Role::SubAdmin => (
            drilldown::count_incidents_scoped(&state.pool, &actor.scopes, filters).await,
            drilldown::incidents_scoped(
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
        Json(
            rows.into_iter()
                .map(SafetyIncidentResponse::from)
                .collect(),
        ),
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
    fn super_admin_sees_platform_wide_incidents() {
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert_eq!(plan(&actor), Plan::All);
    }

    #[test]
    fn sub_admin_incidents_are_scoped_to_its_programs() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        assert_eq!(plan(&actor), Plan::Scoped);
    }

    #[test]
    fn student_is_forbidden_safety_incidents() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert_eq!(plan(&actor), Plan::Forbidden);
    }

    #[test]
    fn every_guardrail_tier_is_an_accepted_filter() {
        for t in 0..=MAX_TIER {
            assert_eq!(tier(Some(t)).expect("a real tier"), Some(t));
        }
        assert_eq!(tier(None).expect("no filter"), None);
    }

    #[test]
    fn a_tier_outside_the_guardrail_range_is_a_validation_error() {
        assert_eq!(
            tier(Some(3)).expect_err("no tier 3").code(),
            "VALIDATION_ERROR"
        );
        assert_eq!(
            tier(Some(-1)).expect_err("no negative tier").code(),
            "VALIDATION_ERROR"
        );
    }

    #[test]
    fn malformed_to_is_a_validation_error() {
        let err = super::super::sessions::timestamp("to", Some("2026-13-45"))
            .expect_err("not RFC3339");
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn incident_kind_maps_to_its_database_label() {
        assert_eq!(IncidentKind::OutOfScope.as_db_str(), "out_of_scope");
        assert_eq!(IncidentKind::Jailbreak.as_db_str(), "jailbreak");
    }
}
