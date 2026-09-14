//! How a percentage becomes a performance level, a star rating and a trophy.
//!
//! # Why this is in `core` and not in either handler
//!
//! The same block is graded twice: once for the student on their assessment
//! page, and once for the admin looking at that student's record. If each side
//! derived its own stars, the two screens could disagree about how the same
//! student did — which is exactly the failure `student_reports` avoids by
//! refusing to keep a mirrored gradebook. One function, used by both, makes
//! that impossible rather than merely unlikely.
//!
//! # Why the server computes it at all
//!
//! The thresholds are a pedagogical decision, not a rendering detail. A client
//! that mapped percentages to stars itself would pin that decision into every
//! client independently, and a change would have to land in all of them at
//! once. The API sends the number *and* the level; the client only chooses the
//! icon.
//!
//! # Nothing here invents a measurement
//!
//! Every function takes `Option<f64>` and returns a "not assessed" answer for
//! `None`. A student who has sat no exam in a block has no performance level —
//! not a zero, not one star. Rendering an unassessed block as zero stars tells
//! the student they failed something they never took.

use serde::{Deserialize, Serialize};

/// A coarse band over a percentage score, from `NotAssessed` upward.
///
/// Serialised in `snake_case` to match every other enum on the wire
/// (`api-conventions.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceLevel {
    /// No graded attempt exists. Distinct from `NeedsWork`, which is a real
    /// measured result.
    NotAssessed,
    /// Below 45%.
    NeedsWork,
    /// 45% to under 60%.
    Developing,
    /// 60% to under 75%.
    Proficient,
    /// 75% to under 90%.
    Strong,
    /// 90% and above.
    Excellent,
}

impl PerformanceLevel {
    /// The band a percentage falls in, or [`PerformanceLevel::NotAssessed`]
    /// when there is nothing to band.
    ///
    /// Boundaries are inclusive at the bottom, so exactly 60.0 is
    /// `Proficient` and not `Developing`.
    pub fn from_percentage(percentage: Option<f64>) -> Self {
        match percentage {
            None => Self::NotAssessed,
            Some(p) if p >= 90.0 => Self::Excellent,
            Some(p) if p >= 75.0 => Self::Strong,
            Some(p) if p >= 60.0 => Self::Proficient,
            Some(p) if p >= 45.0 => Self::Developing,
            Some(_) => Self::NeedsWork,
        }
    }

    /// The label a UI shows. Kept beside the variant so the wording cannot
    /// drift between the student's page and the admin's.
    pub fn label(self) -> &'static str {
        match self {
            Self::NotAssessed => "Not assessed",
            Self::NeedsWork => "Needs work",
            Self::Developing => "Developing",
            Self::Proficient => "Proficient",
            Self::Strong => "Strong",
            Self::Excellent => "Excellent",
        }
    }
}

/// Stars out of five, or `None` when the block has not been assessed.
///
/// `None` rather than `Some(0)` on purpose: zero stars is a verdict, and a
/// block the student has not been examined on has not earned one. The client
/// renders `None` as an empty state, not as five grey stars.
///
/// A graded score always earns at least one star — a student who sat the paper
/// and scored badly is not in the same position as one who never sat it, and
/// the page should not render those two identically.
pub fn stars_from_percentage(percentage: Option<f64>) -> Option<u8> {
    let p = percentage?;
    Some(match p {
        p if p >= 90.0 => 5,
        p if p >= 75.0 => 4,
        p if p >= 60.0 => 3,
        p if p >= 45.0 => 2,
        _ => 1,
    })
}

/// A trophy for sustained performance across a whole course or programme.
///
/// Deliberately *not* awarded from a single score. A trophy that a single good
/// paper could win says nothing about the student's work, so this takes both
/// the average and how many graded attempts stand behind it, and awards
/// nothing until there are enough of them to mean something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trophy {
    Gold,
    Silver,
    Bronze,
}

/// Graded attempts required before any trophy is awarded.
///
/// Three is the smallest number that can show a trend rather than an accident.
pub const TROPHY_MIN_ATTEMPTS: i64 = 3;

/// The trophy for an average across `graded_attempts` papers, if any.
///
/// Returns `None` both when there is no average and when too few papers stand
/// behind it — the client shows "keep going" rather than an empty medal.
pub fn trophy_for(average_percentage: Option<f64>, graded_attempts: i64) -> Option<Trophy> {
    if graded_attempts < TROPHY_MIN_ATTEMPTS {
        return None;
    }
    match average_percentage? {
        p if p >= 90.0 => Some(Trophy::Gold),
        p if p >= 75.0 => Some(Trophy::Silver),
        p if p >= 60.0 => Some(Trophy::Bronze),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The distinction the whole module exists to preserve: "never examined"
    /// and "examined and scored zero" must not render the same.
    #[test]
    fn an_unassessed_block_has_no_level_and_no_stars() {
        assert_eq!(
            PerformanceLevel::from_percentage(None),
            PerformanceLevel::NotAssessed
        );
        assert_eq!(stars_from_percentage(None), None);

        // A real zero is a real verdict, and earns the bottom band — not the
        // "not assessed" one.
        assert_eq!(
            PerformanceLevel::from_percentage(Some(0.0)),
            PerformanceLevel::NeedsWork
        );
        assert_eq!(stars_from_percentage(Some(0.0)), Some(1));
    }

    /// Boundaries are inclusive at the bottom. Pinned because an off-by-one
    /// here silently re-grades every student sitting exactly on a threshold.
    #[test]
    fn band_boundaries_are_inclusive_at_the_bottom() {
        for (pct, level, stars) in [
            (90.0, PerformanceLevel::Excellent, 5),
            (89.9, PerformanceLevel::Strong, 4),
            (75.0, PerformanceLevel::Strong, 4),
            (74.9, PerformanceLevel::Proficient, 3),
            (60.0, PerformanceLevel::Proficient, 3),
            (59.9, PerformanceLevel::Developing, 2),
            (45.0, PerformanceLevel::Developing, 2),
            (44.9, PerformanceLevel::NeedsWork, 1),
        ] {
            assert_eq!(PerformanceLevel::from_percentage(Some(pct)), level, "{pct}");
            assert_eq!(stars_from_percentage(Some(pct)), Some(stars), "{pct}");
        }
    }

    /// A trophy must not be winnable with one lucky paper.
    #[test]
    fn a_trophy_needs_enough_papers_behind_it() {
        assert_eq!(trophy_for(Some(100.0), TROPHY_MIN_ATTEMPTS - 1), None);
        assert_eq!(
            trophy_for(Some(100.0), TROPHY_MIN_ATTEMPTS),
            Some(Trophy::Gold)
        );
    }

    #[test]
    fn trophies_band_the_way_levels_do_and_run_out_at_the_bottom() {
        assert_eq!(trophy_for(Some(90.0), 5), Some(Trophy::Gold));
        assert_eq!(trophy_for(Some(75.0), 5), Some(Trophy::Silver));
        assert_eq!(trophy_for(Some(60.0), 5), Some(Trophy::Bronze));
        // Below the bronze band there is no trophy, rather than a fourth,
        // consolation one: a medal for 30% is not an honest signal.
        assert_eq!(trophy_for(Some(59.9), 5), None);
        assert_eq!(trophy_for(None, 5), None);
    }

    /// The labels are part of the contract — both screens read them from here.
    #[test]
    fn every_level_has_a_label() {
        for level in [
            PerformanceLevel::NotAssessed,
            PerformanceLevel::NeedsWork,
            PerformanceLevel::Developing,
            PerformanceLevel::Proficient,
            PerformanceLevel::Strong,
            PerformanceLevel::Excellent,
        ] {
            assert!(!level.label().is_empty());
        }
    }
}
