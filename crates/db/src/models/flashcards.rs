//! `flashcards` — a student's own notebook plus its SM-2-lite spaced-repetition
//! ledger (`migrations/0006_question_pool_and_flashcards.sql`).
//!
//! Every query here binds `student_id` into the `WHERE` clause. A card belongs
//! to exactly one student, there is no shared deck, and there is deliberately
//! no function in this module that can reach a card without naming its owner —
//! so "self-only" (`.claude/rules/security.md`) is a property of the API rather
//! than a check each handler has to remember.
//!
//! Due-ness is a **calendar** question, answered in the student's own timezone
//! (the NN-3 rule, for the same reason): `due_on` is a `DATE`, and the caller
//! passes the student's local "today" in rather than letting Postgres compare
//! against a UTC `current_date`. [`crate::models::flashcards::schedule_next`] is
//! pure and takes no clock at all, so the review arithmetic is unit-testable
//! without a database or a fixed time.

use chrono::{DateTime, NaiveDate, Utc};
use dg_core::{BlockId, FlashcardId, SessionId, StudentId};
use sqlx::PgPool;

use crate::error::{Error, Result};

/// One card, exactly as stored.
#[derive(Debug, Clone)]
pub struct Flashcard {
    pub id: FlashcardId,
    pub student_id: StudentId,
    pub block_id: BlockId,
    pub block_no: i16,
    pub block_title: String,
    pub front: String,
    pub back: String,
    pub topic: Option<String>,
    pub source_session_id: Option<SessionId>,
    pub due_on: NaiveDate,
    pub interval_days: i16,
    /// Ease factor in hundredths: 250 = 2.50. Integral so review arithmetic is
    /// exact and a replayed review produces the same schedule.
    pub ease: i16,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// The SM-2-lite step: given a card's current schedule and whether the student
/// recalled it, the next `(interval_days, ease)`.
///
/// Pure and clock-free on purpose — this is the logic `.claude/rules/testing.md`
/// wants covered by a unit test, and it cannot be tested meaningfully through a
/// query. The shape is SM-2 with the response quality collapsed to a boolean:
///
/// * a lapse resets the interval to 1 day and drops ease by 0.20,
///   floored at 130 so a repeatedly-failed card still eventually spaces out;
/// * a success steps 0 -> 1 -> 6 days and then multiplies by the ease,
///   capped at the column's ten-year ceiling.
///
/// Both outputs stay inside the table's CHECK ranges for every input inside
/// them, so a scheduled card never fails to store.
#[must_use]
pub fn schedule_next(interval_days: i16, ease: i16, recalled: bool) -> (i16, i16) {
    if !recalled {
        // Ease floor 130 matches SM-2: below it the interval stops growing at
        // all and the card can never leave the daily queue.
        return (1, (ease - 20).max(130));
    }

    let next_interval = match interval_days {
        i if i <= 0 => 1,
        1 => 6,
        i => {
            // `i64` for the multiply: 3650 * 500 overflows an i16 long before
            // the clamp would see it.
            let grown = (i64::from(i) * i64::from(ease)) / 100;
            grown.clamp(1, 3650) as i16
        }
    };

    // Ceiling 500 is the column's: ease is a multiplier, not a score, and an
    // unbounded one turns the third review into a decade.
    (next_interval, (ease + 10).min(500))
}

/// Insert one card.
///
/// This is the internal path the live session calls when a turn produces a card
/// — there is no student-facing create endpoint, and no LLM call here (the
/// contract is explicit that generation is out of scope): the caller supplies
/// `front`/`back` already decided.
#[allow(clippy::too_many_arguments)]
pub async fn create(
    pool: &PgPool,
    student_id: StudentId,
    block_id: BlockId,
    front: &str,
    back: &str,
    topic: Option<&str>,
    source_session_id: Option<SessionId>,
    due_on: NaiveDate,
) -> Result<Flashcard> {
    sqlx::query_as!(
        Flashcard,
        r#"
        WITH inserted AS (
            INSERT INTO flashcards (student_id, block_id, front, back, topic,
                                    source_session_id, due_on)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, student_id, block_id, front, back, topic, source_session_id,
                      due_on, interval_days, ease, reviewed_at, created_at
        )
        SELECT
            f.id                as "id!: FlashcardId",
            f.student_id        as "student_id!: StudentId",
            f.block_id          as "block_id!: BlockId",
            b.block_no          as "block_no!",
            b.title             as "block_title!",
            f.front             as "front!",
            f.back              as "back!",
            f.topic             as "topic",
            f.source_session_id as "source_session_id: SessionId",
            f.due_on            as "due_on!",
            f.interval_days     as "interval_days!",
            f.ease              as "ease!",
            f.reviewed_at       as "reviewed_at",
            f.created_at        as "created_at!"
        FROM inserted f
        JOIN blocks b ON b.id = f.block_id
        "#,
        student_id.into_uuid(),
        block_id.into_uuid(),
        front,
        back,
        topic,
        source_session_id.map(SessionId::into_uuid),
        due_on
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// One page of this student's notebook, most recently created first.
///
/// `due_on_or_before` is the `due_only` filter: the caller passes the student's
/// local today, so "due" means due in the student's own calendar rather than in
/// the server's. `None` returns the whole notebook.
pub async fn list_for_student(
    pool: &PgPool,
    student_id: StudentId,
    due_on_or_before: Option<NaiveDate>,
    block_id: Option<BlockId>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Flashcard>> {
    sqlx::query_as!(
        Flashcard,
        r#"
        SELECT
            f.id                as "id!: FlashcardId",
            f.student_id        as "student_id!: StudentId",
            f.block_id          as "block_id!: BlockId",
            b.block_no          as "block_no!",
            b.title             as "block_title!",
            f.front             as "front!",
            f.back              as "back!",
            f.topic             as "topic",
            f.source_session_id as "source_session_id: SessionId",
            f.due_on            as "due_on!",
            f.interval_days     as "interval_days!",
            f.ease              as "ease!",
            f.reviewed_at       as "reviewed_at",
            f.created_at        as "created_at!"
        FROM flashcards f
        JOIN blocks b ON b.id = f.block_id
        WHERE f.student_id = $1
          AND ($2::date IS NULL OR f.due_on <= $2)
          AND ($3::uuid IS NULL OR f.block_id = $3)
        ORDER BY f.due_on ASC, f.created_at DESC, f.id DESC
        LIMIT $4 OFFSET $5
        "#,
        student_id.into_uuid(),
        due_on_or_before,
        block_id.map(BlockId::into_uuid),
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// `X-Total-Count` for [`list_for_student`] — same filters, no `LIMIT`.
pub async fn count_for_student(
    pool: &PgPool,
    student_id: StudentId,
    due_on_or_before: Option<NaiveDate>,
    block_id: Option<BlockId>,
) -> Result<i64> {
    sqlx::query_scalar!(
        r#"
        SELECT count(*) as "count!"
        FROM flashcards f
        WHERE f.student_id = $1
          AND ($2::date IS NULL OR f.due_on <= $2)
          AND ($3::uuid IS NULL OR f.block_id = $3)
        "#,
        student_id.into_uuid(),
        due_on_or_before,
        block_id.map(BlockId::into_uuid)
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// The dashboard's two notebook numbers in one round trip.
#[derive(Debug, Clone, Copy)]
pub struct FlashcardCounts {
    pub total: i64,
    pub due: i64,
}

/// Total and due counts for the dashboard tile. `today` is the student's local
/// date, for the reason given on [`list_for_student`].
pub async fn counts_for_student(
    pool: &PgPool,
    student_id: StudentId,
    today: NaiveDate,
) -> Result<FlashcardCounts> {
    let rec = sqlx::query!(
        r#"
        SELECT count(*)                                  as "total!",
               count(*) FILTER (WHERE f.due_on <= $2)    as "due!"
        FROM flashcards f
        WHERE f.student_id = $1
        "#,
        student_id.into_uuid(),
        today
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)?;

    Ok(FlashcardCounts {
        total: rec.total,
        due: rec.due,
    })
}

/// One card, **only** if it belongs to `student_id`.
///
/// Another student's card id selects no row, so "not yours" and "does not
/// exist" are the same answer and the handler returns `404` for both — a `403`
/// would confirm the id exists.
pub async fn find_for_student(
    pool: &PgPool,
    student_id: StudentId,
    id: FlashcardId,
) -> Result<Option<Flashcard>> {
    sqlx::query_as!(
        Flashcard,
        r#"
        SELECT
            f.id                as "id!: FlashcardId",
            f.student_id        as "student_id!: StudentId",
            f.block_id          as "block_id!: BlockId",
            b.block_no          as "block_no!",
            b.title             as "block_title!",
            f.front             as "front!",
            f.back              as "back!",
            f.topic             as "topic",
            f.source_session_id as "source_session_id: SessionId",
            f.due_on            as "due_on!",
            f.interval_days     as "interval_days!",
            f.ease              as "ease!",
            f.reviewed_at       as "reviewed_at",
            f.created_at        as "created_at!"
        FROM flashcards f
        JOIN blocks b ON b.id = f.block_id
        WHERE f.id = $2 AND f.student_id = $1
        "#,
        student_id.into_uuid(),
        id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Record a review: advance `due_on`, `interval_days` and `ease`.
///
/// The new schedule is computed by [`schedule_next`] from the row's *current*
/// values, which this function reads and writes under one `UPDATE ... FROM` so
/// two rapid reviews cannot both step from the same starting interval. `today`
/// comes from the student's own timezone; `due_on` is `today + interval`, never
/// `current_date + interval`.
///
/// `None` when the card does not exist or is not this student's.
pub async fn review(
    pool: &PgPool,
    student_id: StudentId,
    id: FlashcardId,
    recalled: bool,
    today: NaiveDate,
) -> Result<Option<Flashcard>> {
    let mut tx = pool.begin().await.map_err(Error::from_sqlx)?;

    // `FOR UPDATE` so the read-compute-write below is serialised against a
    // concurrent review of the same card: without it, two taps on "I knew it"
    // would both step from the old interval and the second would overwrite the
    // first's schedule with an identical one.
    let current = sqlx::query!(
        r#"
        SELECT interval_days, ease
        FROM flashcards
        WHERE id = $2 AND student_id = $1
        FOR UPDATE
        "#,
        student_id.into_uuid(),
        id.into_uuid()
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(Error::from_sqlx)?;

    let Some(current) = current else {
        return Ok(None);
    };

    let (interval_days, ease) = schedule_next(current.interval_days, current.ease, recalled);

    let updated = sqlx::query_as!(
        Flashcard,
        r#"
        WITH updated AS (
            UPDATE flashcards
            SET interval_days = $3,
                ease          = $4,
                due_on        = $5::date + ($3::smallint)::int,
                reviewed_at   = now()
            WHERE id = $2 AND student_id = $1
            RETURNING id, student_id, block_id, front, back, topic, source_session_id,
                      due_on, interval_days, ease, reviewed_at, created_at
        )
        SELECT
            f.id                as "id!: FlashcardId",
            f.student_id        as "student_id!: StudentId",
            f.block_id          as "block_id!: BlockId",
            b.block_no          as "block_no!",
            b.title             as "block_title!",
            f.front             as "front!",
            f.back              as "back!",
            f.topic             as "topic",
            f.source_session_id as "source_session_id: SessionId",
            f.due_on            as "due_on!",
            f.interval_days     as "interval_days!",
            f.ease              as "ease!",
            f.reviewed_at       as "reviewed_at",
            f.created_at        as "created_at!"
        FROM updated f
        JOIN blocks b ON b.id = f.block_id
        "#,
        student_id.into_uuid(),
        id.into_uuid(),
        interval_days,
        ease,
        today
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(Error::from_sqlx)?;

    tx.commit().await.map_err(Error::from_sqlx)?;

    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::schedule_next;

    #[test]
    fn first_successful_review_schedules_one_day_out() {
        let (interval, ease) = schedule_next(0, 250, true);
        assert_eq!(interval, 1);
        assert_eq!(ease, 260);
    }

    #[test]
    fn second_successful_review_schedules_six_days_out() {
        assert_eq!(schedule_next(1, 250, true).0, 6);
    }

    #[test]
    fn later_successful_reviews_multiply_the_interval_by_the_ease() {
        // 6 days at ease 2.50 -> 15 days.
        assert_eq!(schedule_next(6, 250, true).0, 15);
    }

    #[test]
    fn lapse_resets_the_interval_to_one_day_and_drops_the_ease() {
        let (interval, ease) = schedule_next(60, 250, false);
        assert_eq!(interval, 1);
        assert_eq!(ease, 230);
    }

    #[test]
    fn repeated_lapses_never_push_the_ease_below_the_sm2_floor() {
        let mut ease = 250;
        for _ in 0..50 {
            ease = schedule_next(10, ease, false).1;
        }
        assert_eq!(ease, 130);
    }

    #[test]
    fn schedule_stays_inside_the_column_check_ranges() {
        // Every reachable input, including the column ceilings, must produce a
        // storable row — an interval over 3650 or an ease over 500 would fail
        // the CHECK and lose the review.
        for interval in [0i16, 1, 6, 100, 3650] {
            for ease in [130i16, 250, 500] {
                for recalled in [true, false] {
                    let (next_interval, next_ease) = schedule_next(interval, ease, recalled);
                    assert!(
                        (0..=3650).contains(&next_interval),
                        "interval {next_interval}"
                    );
                    assert!((130..=500).contains(&next_ease), "ease {next_ease}");
                }
            }
        }
    }
}
