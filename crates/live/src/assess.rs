//! The tutor's assessment of one student's conversational performance on a unit.
//!
//! # What this is
//!
//! When a unit session ends, the tutor judges how the student *engaged*: how
//! much they spoke, what they asked, how they answered the comprehension
//! checks. It awards stars, a mark and (sometimes) a trophy, and writes a
//! breakdown the student reads on their dashboard.
//!
//! # What this is not
//!
//! **It is not an exam result.** Exam marks come from `exam_attempts` and are
//! computed in SQL from a stored answer key; nothing here touches them, and the
//! two are surfaced separately so a conversational rating can never be mistaken
//! for a graded paper.
//!
//! # Judgement is anchored to measurements
//!
//! The model is given [`EngagementCounters`] — turns, questions, comprehension
//! passes and failures, speaking time — all counted by the gateway itself, and
//! is told to reason from them. It writes the prose; it does not invent the
//! evidence. A model asked to produce a mark from nothing returns a confident
//! number that means nothing, and a student who asks "why did I get this?"
//! deserves an answer made of facts. The counters are stored beside the verdict
//! for exactly that conversation.
//!
//! The floors below matter for the same reason: a student who barely spoke
//! cannot be awarded a high mark however generous the model feels, because the
//! counters say otherwise and [`UnitAssessment::clamp_to_evidence`] enforces it
//! after the fact.
//!
//! # This never runs on the audio path
//!
//! It is called after the socket has closed, from a spawned task. A REST call
//! to a text model has nothing to do with the p95 speech-to-audio budget in
//! `realtime-audio.md`, and must never be allowed to enter it.

use serde::{Deserialize, Serialize};

use crate::error::LiveError;

/// The default model for assessment.
///
/// A text model, not the Live one: this is a single structured-output call
/// after the fact, with no audio and no tools.
pub const ASSESSMENT_MODEL: &str = "gemini-3.1-flash";

const ENDPOINT_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// Transcript characters sent to the assessor.
///
/// A full hour of conversation is far more than the judgement needs, and the
/// cost of this call scales with it. The *end* of the session is kept rather
/// than the beginning: the later exchanges show where the student got to, which
/// is what a performance verdict is about.
const TRANSCRIPT_MAX_CHARS: usize = 24_000;

/// What the gateway measured during the session.
///
/// Every field is counted server-side. None of it comes from the model, and
/// none of it is derived from anything the client claimed.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngagementCounters {
    /// Distinct student utterances the service transcribed.
    pub student_turns: i32,
    /// Utterances that looked like a question to the student's own tutor —
    /// counted from the transcript, so it is an approximation and is described
    /// as one in the prompt rather than presented as exact.
    pub questions_asked: i32,
    /// Comprehension checks (F-35) the student answered correctly.
    pub comprehension_passed: i32,
    /// Comprehension checks the student got wrong or asked to have repeated.
    pub comprehension_failed: i32,
    /// NN-3 active voice for this session. The one figure here that is already
    /// server-authoritative for another reason, reused rather than recounted.
    pub active_voice_ms: i64,
}

impl EngagementCounters {
    /// Whether there is enough conversation to judge at all.
    ///
    /// A session where the student never spoke is not a bad performance, it is
    /// an *absent* one, and assessing it would put a fabricated verdict in
    /// front of a student who did nothing to earn it. The caller skips the
    /// assessment entirely in that case.
    pub fn is_assessable(&self) -> bool {
        self.student_turns >= MIN_TURNS_TO_ASSESS
    }
}

/// Student turns below which no assessment is produced at all.
///
/// Two, not one: a single "yes" in a whole session is an acknowledgement, not a
/// conversation to have an opinion about.
pub const MIN_TURNS_TO_ASSESS: i32 = 2;

/// A trophy for a unit sitting. Same vocabulary as `dg_core::Trophy`, which is
/// the exam-side award, so the UI renders one icon set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitTrophy {
    Gold,
    Silver,
    Bronze,
}

impl UnitTrophy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gold => "gold",
            Self::Silver => "silver",
            Self::Bronze => "bronze",
        }
    }
}

/// The tutor's verdict on one sitting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitAssessment {
    /// 1..=5. There is no zero: a session that happened at all earned a star,
    /// and "not assessed" is the absence of a row rather than a rating of none.
    pub stars: u8,
    /// 0..=100, so two sittings on the same unit are comparable in a way stars
    /// are too coarse to be.
    pub mark: f64,
    pub trophy: Option<UnitTrophy>,
    /// The breakdown the student reads. Required.
    pub summary: String,
    pub strengths: Vec<String>,
    pub improvements: Vec<String>,
}

/// Mark above which a sitting can earn each trophy.
const GOLD_MIN: f64 = 90.0;
const SILVER_MIN: f64 = 75.0;
const BRONZE_MIN: f64 = 60.0;

impl UnitAssessment {
    /// Forces the verdict back inside what the evidence can support.
    ///
    /// The model is asked to be fair, but "asked" is not "guaranteed", and the
    /// failure mode of an encouraging tutor is to award four stars to a student
    /// who said three words. These ceilings are the enforcement: a thin session
    /// cannot be rated highly no matter what the prose says.
    ///
    /// The trophy is then **recomputed** from the clamped mark rather than
    /// trusted, so a medal can never contradict the number printed beside it.
    pub fn clamp_to_evidence(mut self, counters: &EngagementCounters) -> Self {
        // Ceilings by how much the student actually said. Deliberately blunt:
        // a precise formula here would imply a precision the underlying signal
        // does not have.
        let ceiling = match counters.student_turns {
            0..=2 => 40.0,
            3..=5 => 65.0,
            6..=10 => 85.0,
            _ => 100.0,
        };
        self.mark = self.mark.clamp(0.0, ceiling);

        // Every comprehension failure that was never recovered pulls the mark
        // down. Not a judgement call: the check exists precisely to measure
        // whether the explanation landed.
        let unrecovered = (counters.comprehension_failed - counters.comprehension_passed).max(0);
        self.mark = (self.mark - f64::from(unrecovered) * 5.0).max(0.0);

        self.stars = stars_for_mark(self.mark);
        self.trophy = trophy_for_mark(self.mark);
        self.summary = self.summary.trim().to_string();
        // Bullets are a UI element; an unbounded list from a model becomes a
        // wall of text on a card.
        self.strengths.truncate(4);
        self.improvements.truncate(4);
        self
    }
}

/// Stars from a mark, on the same boundaries `dg_core::performance` uses for
/// exams, so one rating scale is taught to the student rather than two.
pub fn stars_for_mark(mark: f64) -> u8 {
    match mark {
        m if m >= GOLD_MIN => 5,
        m if m >= SILVER_MIN => 4,
        m if m >= BRONZE_MIN => 3,
        m if m >= 45.0 => 2,
        _ => 1,
    }
}

/// A trophy from a mark, or `None` below the bronze band. There is deliberately
/// no consolation medal.
pub fn trophy_for_mark(mark: f64) -> Option<UnitTrophy> {
    match mark {
        m if m >= GOLD_MIN => Some(UnitTrophy::Gold),
        m if m >= SILVER_MIN => Some(UnitTrophy::Silver),
        m if m >= BRONZE_MIN => Some(UnitTrophy::Bronze),
        _ => None,
    }
}

/// One line of the conversation handed to the assessor.
#[derive(Debug, Clone)]
pub struct TranscriptLine {
    /// `true` when the student said it.
    pub from_student: bool,
    pub text: String,
}

/// Renders the transcript for the prompt, keeping the END when it is too long.
///
/// Truncating the tail would throw away the part of the lesson the verdict is
/// most about; truncating the head loses only the opening, which is the tutor
/// talking.
fn render_transcript(lines: &[TranscriptLine]) -> String {
    let mut rendered = String::new();
    for line in lines {
        let who = if line.from_student { "STUDENT" } else { "TUTOR" };
        rendered.push_str(who);
        rendered.push_str(": ");
        rendered.push_str(line.text.trim());
        rendered.push('\n');
    }
    if rendered.len() > TRANSCRIPT_MAX_CHARS {
        // Cut on a character boundary — a byte slice through a Malayalam
        // codepoint would panic, and this runs in a spawned task where a panic
        // is silent.
        let start = rendered.len() - TRANSCRIPT_MAX_CHARS;
        let start = (start..rendered.len())
            .find(|i| rendered.is_char_boundary(*i))
            .unwrap_or(rendered.len());
        rendered = format!("[earlier conversation omitted]\n{}", &rendered[start..]);
    }
    rendered
}

/// The instruction. Separate from the tutor's own persona on purpose: this is
/// an evaluation, not a lesson, and reusing the teaching prompt would invite
/// the model to keep teaching.
fn assessment_instruction(unit_title: &str, counters: &EngagementCounters) -> String {
    format!(
        "You are assessing one student's CONVERSATIONAL performance in a single \
         tutoring session on the unit \"{unit_title}\". You are not marking an \
         exam and you are not teaching. Judge only how the student engaged in \
         the conversation.\n\n\
         MEASURED FACTS about this session. These were counted by the platform, \
         not by you. Reason from them and do not contradict them:\n\
         - Student spoke {turns} time(s).\n\
         - Roughly {questions} of those were questions (an approximation from \
           the transcript, not an exact count).\n\
         - Comprehension checks answered correctly: {passed}.\n\
         - Comprehension checks answered wrongly or asked to be repeated: {failed}.\n\
         - Student's active speaking time: {minutes} minute(s).\n\n\
         HOW TO JUDGE.\n\
         1. Base every claim on the transcript and the facts above. Never invent \
            an exchange that is not there, and never praise something the student \
            did not do.\n\
         2. A short session is a short session. If the student barely spoke, say \
            so plainly and mark accordingly — an encouraging verdict on an absent \
            student is a lie that helps nobody.\n\
         3. Asking good questions, answering the comprehension checks, correcting \
            themselves, and staying on the unit are what a high mark is for. \
            Silence, one-word answers and repeated confusion are what a low mark \
            is for.\n\
         4. Write to the STUDENT, in the second person, warmly and specifically. \
            Name the actual topic they struggled with rather than saying \
            \"some topics\".\n\
         5. If the student spoke a language other than English, write your \
            summary in that language — they must be able to read their own \
            feedback.\n\n\
         OUTPUT. Return JSON only, matching the schema. `mark` is 0-100. \
         `summary` is 2-4 sentences of specific, honest feedback. `strengths` \
         and `improvements` are each at most 3 short phrases; either may be \
         empty if the session does not support any, and an empty list is far \
         better than an invented entry.",
        turns = counters.student_turns,
        questions = counters.questions_asked,
        passed = counters.comprehension_passed,
        failed = counters.comprehension_failed,
        minutes = counters.active_voice_ms / 60_000,
    )
}

/// The structured-output schema.
///
/// Requested explicitly rather than parsed out of prose: this verdict is
/// written to a database and rendered on two screens, and "usually returns
/// JSON" is not a contract. `stars` and `trophy` are deliberately NOT in the
/// schema — they are derived from the clamped mark in
/// [`UnitAssessment::clamp_to_evidence`], so they cannot disagree with it.
fn response_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "mark": { "type": "number" },
            "summary": { "type": "string" },
            "strengths": { "type": "array", "items": { "type": "string" } },
            "improvements": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["mark", "summary", "strengths", "improvements"]
    })
}

/// What the model returns, before clamping.
#[derive(Debug, Deserialize)]
struct RawVerdict {
    mark: f64,
    summary: String,
    #[serde(default)]
    strengths: Vec<String>,
    #[serde(default)]
    improvements: Vec<String>,
}

/// Asks the model for a verdict on one sitting.
///
/// Returns `Ok(None)` when the session is too thin to judge — see
/// [`EngagementCounters::is_assessable`]. That is a normal outcome, not an
/// error: most abandoned sessions land here, and the caller simply writes no
/// row.
pub async fn assess_unit(
    api_key: &str,
    model: &str,
    unit_title: &str,
    counters: &EngagementCounters,
    transcript: &[TranscriptLine],
) -> Result<Option<UnitAssessment>, LiveError> {
    if !counters.is_assessable() {
        return Ok(None);
    }

    let body = serde_json::json!({
        "systemInstruction": {
            "parts": [{ "text": assessment_instruction(unit_title, counters) }]
        },
        "contents": [{
            "role": "user",
            "parts": [{ "text": format!(
                "Transcript of the session:\n\n{}", render_transcript(transcript)
            ) }]
        }],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseSchema": response_schema(),
            // Low, not zero: this is a judgement rendered as prose, and a
            // fully greedy decode produces the same three sentences for every
            // student, which reads as a form letter rather than feedback.
            "temperature": 0.4
        }
    });

    let url = format!("{ENDPOINT_BASE}/{model}:generateContent");
    let response = reqwest::Client::new()
        .post(&url)
        .header("x-goog-api-key", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|err| LiveError::Session(format!("assessment request failed: {err}")))?;

    let status = response.status();
    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|err| LiveError::Session(format!("assessment response was not JSON: {err}")))?;

    if !status.is_success() {
        // The API's own message, not the body: a failed assessment is logged
        // server-side and never reaches a student, so detail here is free.
        let detail = payload
            .pointer("/error/message")
            .and_then(|m| m.as_str())
            .unwrap_or("no message");
        return Err(LiveError::Session(format!(
            "assessment rejected ({status}): {detail}"
        )));
    }

    let text = payload
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(|t| t.as_str())
        .ok_or_else(|| LiveError::Session("assessment returned no text part".to_string()))?;

    let raw: RawVerdict = serde_json::from_str(text)
        .map_err(|err| LiveError::Session(format!("assessment was not the agreed shape: {err}")))?;

    if raw.summary.trim().is_empty() {
        return Err(LiveError::Session(
            "assessment returned an empty summary".to_string(),
        ));
    }

    Ok(Some(
        UnitAssessment {
            stars: 1,
            mark: raw.mark,
            trophy: None,
            summary: raw.summary,
            strengths: raw.strengths,
            improvements: raw.improvements,
        }
        .clamp_to_evidence(counters),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(mark: f64) -> UnitAssessment {
        UnitAssessment {
            stars: 1,
            mark,
            trophy: None,
            summary: "  You asked good questions.  ".to_string(),
            strengths: vec![],
            improvements: vec![],
        }
    }

    fn counters(turns: i32) -> EngagementCounters {
        EngagementCounters {
            student_turns: turns,
            ..Default::default()
        }
    }

    /// The point of the clamp: an encouraging model cannot award a high mark to
    /// a student who barely spoke.
    #[test]
    fn a_thin_session_cannot_be_marked_highly_however_generous_the_model_is() {
        let clamped = verdict(98.0).clamp_to_evidence(&counters(1));
        assert!(clamped.mark <= 40.0, "got {}", clamped.mark);
        assert_eq!(clamped.trophy, None, "a trophy needs a real conversation");

        // A full conversation is left alone.
        let full = verdict(98.0).clamp_to_evidence(&counters(20));
        assert_eq!(full.mark, 98.0);
        assert_eq!(full.trophy, Some(UnitTrophy::Gold));
    }

    /// Stars and trophy are recomputed from the clamped mark, so a medal can
    /// never contradict the number printed next to it.
    #[test]
    fn stars_and_trophy_are_derived_from_the_final_mark_not_trusted() {
        let clamped = verdict(95.0).clamp_to_evidence(&counters(4));
        assert_eq!(clamped.mark, 65.0, "ceiling for 3-5 turns");
        assert_eq!(clamped.stars, stars_for_mark(65.0));
        assert_eq!(clamped.trophy, Some(UnitTrophy::Bronze));
    }

    /// Unrecovered confusion pulls the mark down — the comprehension gate is a
    /// measurement of whether the teaching landed, not decoration.
    #[test]
    fn unrecovered_comprehension_failures_reduce_the_mark() {
        let c = EngagementCounters {
            student_turns: 20,
            comprehension_passed: 1,
            comprehension_failed: 4,
            ..Default::default()
        };
        // Three unrecovered failures at five marks each.
        assert_eq!(verdict(90.0).clamp_to_evidence(&c).mark, 75.0);
    }

    /// An absent student is not assessed at all, rather than assessed badly.
    #[test]
    fn a_session_with_no_conversation_is_not_assessable() {
        assert!(!counters(0).is_assessable());
        assert!(!counters(1).is_assessable());
        assert!(counters(2).is_assessable());
    }

    /// The tail is what a performance verdict is about, so the head is what
    /// gets dropped.
    #[test]
    fn an_overlong_transcript_keeps_the_end_and_stays_on_a_char_boundary() {
        // Malayalam, so a naive byte slice would panic rather than truncate.
        let lines: Vec<TranscriptLine> = (0..4000)
            .map(|i| TranscriptLine {
                from_student: i % 2 == 0,
                text: "വനം എന്നാൽ എന്ത്".to_string(),
            })
            .collect();
        let rendered = render_transcript(&lines);
        assert!(rendered.starts_with("[earlier conversation omitted]"));
        assert!(rendered.len() <= TRANSCRIPT_MAX_CHARS + 64);
    }

    #[test]
    fn the_summary_is_trimmed_and_bullet_lists_are_bounded() {
        let mut v = verdict(80.0);
        v.strengths = (0..9).map(|i| format!("s{i}")).collect();
        v.improvements = (0..9).map(|i| format!("i{i}")).collect();
        let clamped = v.clamp_to_evidence(&counters(20));
        assert_eq!(clamped.summary, "You asked good questions.");
        assert_eq!(clamped.strengths.len(), 4);
        assert_eq!(clamped.improvements.len(), 4);
    }
}
