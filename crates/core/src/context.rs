//! Types for the "Remember Me" context payload — `GET /me/context` and the
//! WebSocket `session_ready` message (Phase 3) both carry this shape. See
//! `IMPLEMENTATION_PLAN.md` §3.3 (`ctx:{student_id}` in Redis) and §4.1 item 5.

use crate::ids::{BlockId, CourseId, ProgramId, SemesterId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramSummary {
    pub id: ProgramId,
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemesterSummary {
    pub id: SemesterId,
    pub semester_number: i16,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockSummary {
    pub id: BlockId,
    pub course_id: CourseId,
    pub block_no: i16,
    pub title: String,
}

/// The full "Remember Me" payload: a student's academic context, restored on
/// login so the classroom opens directly on the right block with no picker
/// (`IMPLEMENTATION_PLAN.md` §4.1 item 5, checklist C-15).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentContext {
    pub program: ProgramSummary,
    pub semester: SemesterSummary,
    /// `None` until the student has been placed into a block (e.g. brand
    /// new enrolment before Phase 2/3 content assignment).
    pub current_block: Option<BlockSummary>,
    /// NN-2: the greeting plays only while this is `true`. Flipped to
    /// `false` in the same transaction that opens the first learning
    /// session — this field mirrors `students.is_first_login` verbatim.
    pub is_first_login: bool,
}
