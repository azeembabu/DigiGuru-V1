//! Field-level validators for student registration and profile updates
//! (`IMPLEMENTATION_PLAN.md` §4.1 item 3).
//!
//! **Judgment call:** the plan specifies "roll number: regex per program",
//! but no per-program pattern table exists anywhere in the schema or the
//! plan (`programs` has no such column). This scaffold applies one
//! conservative, uniform pattern instead and enforces uniqueness at the DB
//! layer (`students.roll_number` is `UNIQUE`) — a genuine per-program regex
//! would need a new column and migration, out of scope for this pass.
//! Flagged in the task's final report, not silently decided.

use std::sync::LazyLock;

use dg_core::FieldError;
use regex::Regex;

static ROLL_NUMBER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Z0-9-]{4,20}$").expect("static regex is valid"));

static INDIAN_MOBILE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[6-9]\d{9}$").expect("static regex is valid"));

pub fn validate_roll_number(value: &str) -> Result<(), FieldError> {
    if ROLL_NUMBER_RE.is_match(value) {
        Ok(())
    } else {
        Err(FieldError::new(
            "roll_number",
            "must be 4-20 uppercase letters, digits, or hyphens",
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
    fn roll_number_rejects_lowercase() {
        assert!(validate_roll_number("baml2024001").is_err());
        assert!(validate_roll_number("BAML2024001").is_ok());
    }
}
