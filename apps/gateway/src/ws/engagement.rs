//! What the gateway observes about a student's conversation, and the job that
//! turns it into an assessment once the session ends.
//!
//! # Why the counting happens here and not in the model
//!
//! The model writes the verdict's prose, but it must not also supply the
//! evidence for it — a model asked "how engaged was this student?" with nothing
//! to measure returns a confident number that means nothing. So the socket
//! counts what it can actually see (`crates/live/src/assess.rs` explains what
//! is then done with those counts), and the model reasons from figures it did
//! not choose.
//!
//! # These counters are honest about being approximations
//!
//! Two of them are heuristics over the transcript and are described as such in
//! the prompt:
//!
//! * **Questions** are detected by question marks and interrogatives. A student
//!   who asks something without either is not counted.
//! * **Confusion signals** are the local intent match from `pedagogy.md` ("I
//!   don't understand", "വീണ്ടും പറയാമോ"). They are stored in the
//!   `comprehension_failed` column because that is what they are evidence of,
//!   but the comprehension gate itself (F-35) has no server-side pass/fail
//!   record yet, so `comprehension_passed` stays `0` rather than being invented.
//!   That asymmetry is deliberate: an uncounted success must never be reported
//!   as a success.
//!
//! # Nothing here runs on the audio path
//!
//! Counting is a few string comparisons per transcript fragment, and the
//! assessment call happens in a task spawned after the socket has closed.

use dg_core::{BlockId, DocumentId, SessionId, StudentId};
use live::assess::{EngagementCounters, TranscriptLine};

/// Transcript lines kept in memory for one session.
///
/// A cap, not a target: an hour of conversation is well under this, and the
/// bound exists so a pathological session cannot grow a socket's memory without
/// limit. The oldest lines are dropped first — the assessor keeps the end of
/// the conversation anyway.
const MAX_TRANSCRIPT_LINES: usize = 2_000;

/// Phrases that mean "I did not follow that", in both languages of the
/// platform. Matched case-insensitively as substrings, which is what the local
/// intent matcher in `pedagogy.md` does.
const CONFUSION_PHRASES: &[&str] = &[
    "i don't understand",
    "i dont understand",
    "i didn't understand",
    "didn't get that",
    "did not understand",
    "say that again",
    "repeat that",
    "come again",
    "confused",
    "മനസ്സിലായില്ല",
    "വീണ്ടും പറയാമോ",
    "ഒന്നുകൂടി",
];

/// Interrogatives, so a question asked without a question mark still counts.
/// The transcript is speech recognition output and punctuation is unreliable.
///
/// The Malayalam entries are **stems**, and that is load-bearing: an
/// interrogative inflects, and the inflection replaces the very character a
/// full-form match relies on. `എന്ത്` ("what") becomes `എന്താണ്` ("what is"),
/// where the chandrakkala after ത is replaced by the vowel sign ാ — so `എന്ത്`
/// is *not* a substring of `എന്താണ്`, and matching the full form silently
/// missed the most common way the question is actually asked. The stem `എന്ത`
/// matches every inflection of it.
const QUESTION_WORDS: &[&str] = &[
    "what", "why", "how", "when", "where", "which", "who", "can you", "could you", "is it",
    "does it",
    // Stems, for the reason above.
    "എന്ത",   // എന്ത്, എന്താണ്, എന്തുകൊണ്ട്, എന്തിന്
    "എങ്ങനെ", // how
    "ഏത",     // ഏത്, ഏതാണ്
    "എപ്പോ",  // എപ്പോൾ, എപ്പോഴാണ്
    "എവിടെ",  // where
    // Left as the full form deliberately: the stem `ആര` also opens common
    // ordinary words such as ആരംഭം ("beginning"), which would count a
    // statement as a question.
    "ആര്",
];

/// The conversation as the socket saw it, plus the counters derived from it.
#[derive(Debug, Default)]
pub struct EngagementLog {
    lines: Vec<TranscriptLine>,
    counters: EngagementCounters,
    /// Whether the last fragment came from the student, so incremental
    /// fragments of one utterance are not counted as separate turns. Live emits
    /// transcription a word or two at a time; counting fragments would report a
    /// single sentence as a dozen turns and inflate every assessment.
    last_was_student: bool,
    /// The student's current utterance, accumulated across fragments and
    /// classified only once it is complete.
    pending: String,
}

impl EngagementLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one transcript fragment.
    pub fn push(&mut self, from_student: bool, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }

        if from_student {
            if !self.last_was_student {
                // A new student utterance begins wherever the speaker changes.
                self.counters.student_turns += 1;
                self.pending.clear();
            }
            if !self.pending.is_empty() {
                self.pending.push(' ');
            }
            self.pending.push_str(text);
        } else if self.last_was_student {
            // The student's utterance just ended — classify it whole rather
            // than per fragment, or "what" and "is sandhi" would be judged
            // separately and the interrogative missed.
            self.classify_pending();
        }

        self.last_was_student = from_student;
        self.lines.push(TranscriptLine {
            from_student,
            text: text.to_string(),
        });
        if self.lines.len() > MAX_TRANSCRIPT_LINES {
            self.lines.remove(0);
        }
    }

    fn classify_pending(&mut self) {
        let utterance = std::mem::take(&mut self.pending).to_lowercase();
        if utterance.is_empty() {
            return;
        }
        if utterance.contains('?') || QUESTION_WORDS.iter().any(|w| utterance.contains(w)) {
            self.counters.questions_asked += 1;
        }
        if CONFUSION_PHRASES.iter().any(|p| utterance.contains(p)) {
            self.counters.comprehension_failed += 1;
        }
    }

    /// Finishes the log and returns what was observed.
    ///
    /// `active_voice_ms` is passed in rather than counted here: it is the NN-3
    /// ledger's figure, already server-authoritative for quota, and recounting
    /// it would create a second number free to disagree with the one the
    /// student is billed against.
    pub fn finish(mut self, active_voice_ms: i64) -> (EngagementCounters, Vec<TranscriptLine>) {
        // A session that ended while the student was still talking still has an
        // utterance to classify.
        if self.last_was_student {
            self.classify_pending();
        }
        self.counters.active_voice_ms = active_voice_ms;
        (self.counters, self.lines)
    }
}

/// Everything the assessment job needs, captured before the socket task exits.
pub struct AssessmentJob {
    pub student_id: StudentId,
    pub document_id: DocumentId,
    pub block_id: BlockId,
    pub session_id: SessionId,
    pub unit_title: String,
    pub counters: EngagementCounters,
    pub transcript: Vec<TranscriptLine>,
}

/// Produces and stores the assessment for one finished sitting.
///
/// Spawned after the socket closes, so it cannot affect the session it is about
/// and cannot delay a teardown. Every failure is logged and swallowed: a
/// missing assessment is a missing card on a dashboard, and is never worth
/// surfacing to a student who has already left the classroom.
pub async fn run_assessment(
    pool: dg_db::PgPool,
    api_key: String,
    model: String,
    job: AssessmentJob,
) {
    let verdict = match live::assess::assess_unit(
        &api_key,
        &model,
        &job.unit_title,
        &job.counters,
        &job.transcript,
    )
    .await
    {
        Ok(Some(verdict)) => verdict,
        Ok(None) => {
            // Not an error: the student barely spoke, and an assessment of a
            // conversation that did not happen would be fabricated.
            tracing::debug!(
                session_id = %job.session_id,
                turns = job.counters.student_turns,
                "session too thin to assess; no verdict written"
            );
            return;
        }
        Err(err) => {
            tracing::error!(%err, session_id = %job.session_id, "unit assessment failed");
            return;
        }
    };

    let row = dg_db::models::unit_assessments::NewUnitAssessment {
        student_id: job.student_id,
        document_id: job.document_id,
        block_id: job.block_id,
        session_id: job.session_id,
        stars: i16::from(verdict.stars),
        mark: verdict.mark,
        trophy: verdict.trophy.map(|t| t.as_str().to_string()),
        summary: verdict.summary,
        strengths: verdict.strengths,
        improvements: verdict.improvements,
        student_turns: job.counters.student_turns,
        questions_asked: job.counters.questions_asked,
        comprehension_passed: job.counters.comprehension_passed,
        comprehension_failed: job.counters.comprehension_failed,
        active_voice_ms: job.counters.active_voice_ms,
    };

    match dg_db::models::unit_assessments::insert(&pool, &row).await {
        Ok(id) => tracing::info!(
            assessment_id = %id,
            session_id = %job.session_id,
            stars = row.stars,
            "unit assessment recorded"
        ),
        Err(err) => tracing::error!(%err, session_id = %job.session_id, "failed to store the unit assessment"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live emits transcription in fragments. Counting them as turns would
    /// report one sentence as a dozen and inflate every assessment that
    /// followed.
    #[test]
    fn fragments_of_one_utterance_count_as_a_single_turn() {
        let mut log = EngagementLog::new();
        log.push(true, "what");
        log.push(true, "is");
        log.push(true, "sandhi");
        let (counters, _) = log.finish(0);
        assert_eq!(counters.student_turns, 1);
        assert_eq!(counters.questions_asked, 1, "classified whole, not per word");
    }

    /// A turn boundary is a change of speaker.
    #[test]
    fn each_student_utterance_between_tutor_turns_is_its_own_turn() {
        let mut log = EngagementLog::new();
        log.push(true, "what is sandhi");
        log.push(false, "Sandhi is the joining of sounds.");
        log.push(true, "and what is samasam");
        log.push(false, "Samasam is compounding.");
        let (counters, _) = log.finish(0);
        assert_eq!(counters.student_turns, 2);
        assert_eq!(counters.questions_asked, 2);
    }

    /// Speech recognition rarely produces question marks, so an interrogative
    /// has to be enough on its own — in either language.
    ///
    /// `എന്താണ്` is the case that caught a real bug: it is the inflected form
    /// of `എന്ത്`, and the inflection replaces the chandrakkala the full-form
    /// match depended on, so the commonest Malayalam question went uncounted.
    #[test]
    fn questions_are_detected_without_punctuation_in_both_languages() {
        let mut log = EngagementLog::new();
        log.push(true, "എന്താണ് വനം");
        log.push(false, "...");
        log.push(true, "tell me more");
        let (counters, _) = log.finish(0);
        assert_eq!(counters.questions_asked, 1, "only the interrogative counts");
        assert_eq!(counters.student_turns, 2);
    }

    /// Every inflection of the same interrogative has to count, not just the
    /// dictionary form.
    #[test]
    fn inflected_malayalam_interrogatives_all_count() {
        for question in ["എന്ത്", "എന്താണ്", "എന്തുകൊണ്ട്", "ഏതാണ്", "എപ്പോഴാണ്"] {
            let mut log = EngagementLog::new();
            log.push(true, question);
            let (counters, _) = log.finish(0);
            assert_eq!(counters.questions_asked, 1, "missed: {question}");
        }
    }

    /// The stem `ആര` opens ordinary words like ആരംഭം ("beginning"), so the
    /// full form is matched instead — a statement must not be counted as a
    /// question.
    #[test]
    fn an_ordinary_word_sharing_an_interrogative_stem_is_not_a_question() {
        let mut log = EngagementLog::new();
        log.push(true, "ആരംഭം നന്നായിരുന്നു");
        let (counters, _) = log.finish(0);
        assert_eq!(counters.questions_asked, 0);
    }

    #[test]
    fn confusion_is_counted_in_both_languages() {
        let mut log = EngagementLog::new();
        log.push(true, "sorry I don't understand");
        log.push(false, "Let me explain again.");
        log.push(true, "മനസ്സിലായില്ല");
        let (counters, _) = log.finish(0);
        assert_eq!(counters.comprehension_failed, 2);
        // Never invented: nothing server-side measures a comprehension PASS.
        assert_eq!(counters.comprehension_passed, 0);
    }

    /// A session that ends mid-sentence still has that sentence to classify.
    #[test]
    fn an_utterance_left_open_at_teardown_is_still_classified() {
        let mut log = EngagementLog::new();
        log.push(false, "Do you follow?");
        log.push(true, "no I am confused");
        let (counters, _) = log.finish(0);
        assert_eq!(counters.student_turns, 1);
        assert_eq!(counters.comprehension_failed, 1);
    }

    /// The quota ledger owns speaking time; this must not invent a second one.
    #[test]
    fn active_voice_time_comes_from_the_ledger_not_from_counting_here() {
        let log = EngagementLog::new();
        let (counters, _) = log.finish(123_456);
        assert_eq!(counters.active_voice_ms, 123_456);
    }

    /// Memory is bounded, and the END of the conversation is what survives —
    /// the assessor cares about where the student got to.
    #[test]
    fn the_transcript_is_bounded_and_keeps_the_most_recent_lines() {
        let mut log = EngagementLog::new();
        for i in 0..(MAX_TRANSCRIPT_LINES + 50) {
            log.push(i % 2 == 0, &format!("line {i}"));
        }
        let (_, lines) = log.finish(0);
        assert_eq!(lines.len(), MAX_TRANSCRIPT_LINES);
        let last = &lines[lines.len() - 1].text;
        assert_eq!(last, &format!("line {}", MAX_TRANSCRIPT_LINES + 49));
    }

    /// Empty fragments arrive routinely as speech starts and stops; they must
    /// not open a turn.
    #[test]
    fn blank_fragments_are_ignored() {
        let mut log = EngagementLog::new();
        log.push(true, "   ");
        log.push(true, "");
        let (counters, lines) = log.finish(0);
        assert_eq!(counters.student_turns, 0);
        assert!(lines.is_empty());
    }
}
