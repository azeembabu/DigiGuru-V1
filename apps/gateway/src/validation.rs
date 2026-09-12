//! Field-level validators for student registration and profile updates
//! (`IMPLEMENTATION_PLAN.md` §4.1 item 3).
//!
//! **Roll number format.** `YY` + intake + level + programme + a 5-digit
//! serial, e.g. `25XHBML11450`.
//!
//! | Part | Example | Meaning |
//! |---|---|---|
//! | `YY` | `25` | admission year (2025) |
//! | intake | `X` | first intake; `Y` is the second (there are exactly two a year) |
//! | level | `HB` | honours bachelor; `B` plain bachelor, `M` masters |
//! | programme | `ML` | two-letter programme code — `ML` Malayalam, `SO` Sociology, `EG` English |
//! | serial | `11450` | five digits |
//!
//! So `25XHBML11450` is an honours-bachelor Malayalam student,
//! `25XBSO11450` a (non-honours) bachelor Sociology student, and
//! `25XMEG21121` a masters English student.
//!
//! Honours applies to bachelor programmes only, so `H` is accepted only
//! immediately before `B` — there is no `HM`.
//!
//! The programme code is matched as any two letters rather than against a
//! fixed list: the full set is not recorded anywhere in this repo, and
//! hard-coding a partial list would reject valid students. Cross-checking the
//! code against the selected `program_id` would be a stronger rule and is
//! worth adding once a code-to-programme mapping exists.
//!
//! The format is university-wide, not per-program: the plan's "regex per
//! program" line predates this rule, and `programs` carries no pattern column.
//! Uniqueness is still enforced at the DB layer (`students.roll_number` is
//! `UNIQUE`).

use std::sync::LazyLock;

use dg_core::FieldError;
use regex::Regex;

static ROLL_NUMBER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{2}[XY](?:HB|B|M)[A-Z]{2}\d{5}$").expect("static regex is valid"));

static INDIAN_MOBILE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[6-9]\d{9}$").expect("static regex is valid"));

pub fn validate_roll_number(value: &str) -> Result<(), FieldError> {
    if ROLL_NUMBER_RE.is_match(value) {
        Ok(())
    } else {
        Err(FieldError::new(
            "roll_number",
            "must be year, intake (X/Y), level (HB, B or M), a 2-letter programme code, then 5 digits (e.g. 25XHBML11450)",
        ))
    }
}

/// Normalise an Indian phone number to E.164 (`+91XXXXXXXXXX`). Accepts a
/// bare 10-digit mobile number, or one already prefixed with `0`/`91`/`+91`,
/// with any spaces/dashes/parens stripped first.
pub fn normalize_indian_phone(raw: &str) -> Result<String, FieldError> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();

    let ten_digit = if digits.len() == 10 {
        digits
    } else if digits.len() == 11 && digits.starts_with('0') {
        digits[1..].to_string()
    } else if digits.len() == 12 && digits.starts_with("91") {
        digits[2..].to_string()
    } else {
        return Err(invalid_phone());
    };

    if INDIAN_MOBILE_RE.is_match(&ten_digit) {
        Ok(format!("+91{ten_digit}"))
    } else {
        Err(invalid_phone())
    }
}

fn invalid_phone() -> FieldError {
    FieldError::new("phone_number", "must be a valid 10-digit Indian mobile number")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_bare_ten_digit() {
        assert_eq!(normalize_indian_phone("9876543210").unwrap(), "+919876543210");
    }

    #[test]
    fn normalizes_with_country_code_and_punctuation() {
        assert_eq!(normalize_indian_phone("+91 98765-43210").unwrap(), "+919876543210");
    }

    #[test]
    fn rejects_landline_prefix() {
        assert!(normalize_indian_phone("1234567890").is_err());
    }

    #[test]
    fn roll_number_accepts_each_level() {
        assert!(validate_roll_number("25XHBML11450").is_ok()); // honours bachelor, Malayalam
        assert!(validate_roll_number("25XBSO11450").is_ok()); // bachelor, Sociology
        assert!(validate_roll_number("25XMEG21121").is_ok()); // masters, English
    }

    #[test]
    fn roll_number_accepts_both_intakes() {
        assert!(validate_roll_number("25XBSO11450").is_ok());
        assert!(validate_roll_number("25YBSO11450").is_ok());
    }

    #[test]
    fn roll_number_rejects_honours_masters() {
        // Honours is a bachelor-only distinction, so there is no `HM`.
        assert!(validate_roll_number("25XHMEG21121").is_err());
    }

    #[test]
    fn roll_number_rejects_missing_level() {
        assert!(validate_roll_number("25XML11450").is_err());
    }

    #[test]
    fn roll_number_rejects_lowercase() {
        assert!(validate_roll_number("25xbso11450").is_err());
        assert!(validate_roll_number("25XBso11450").is_err());
    }

    #[test]
    fn roll_number_rejects_bad_intake_letter() {
        assert!(validate_roll_number("25ABSO11450").is_err());
        assert!(validate_roll_number("25ZBSO11450").is_err());
    }

    #[test]
    fn roll_number_rejects_wrong_serial_length() {
        assert!(validate_roll_number("25XBSO1145").is_err()); // 4 digits
        assert!(validate_roll_number("25XBSO114500").is_err()); // 6 digits
    }

    #[test]
    fn roll_number_rejects_wrong_programme_code_length() {
        assert!(validate_roll_number("25XBS11450").is_err()); // 1 letter
        assert!(validate_roll_number("25XBSOC11450").is_err()); // 3 letters
    }

    #[test]
    fn roll_number_rejects_superseded_formats() {
        assert!(validate_roll_number("25X1234").is_err());
        assert!(validate_roll_number("BAML2024001").is_err());
    }
}
