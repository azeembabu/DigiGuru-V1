//! Parsing for the four bulk question-pool import formats, and the field
//! validation every channel shares.
//!
//! Split out of `questions.rs` so the handlers stay readable, and because the
//! important property here is that **all four formats converge on one
//! validator**: a rule that holds on the manual form but not on the `.docx`
//! path is exactly how a question with a wrong answer key gets into the pool.
//!
//! # One validator, one grid
//!
//! CSV, `.xlsx` and `.docx` are all *tabular* with the same column headers, so
//! they are parsed into a header row plus a grid of cells and then handed to
//! the same [`parse_grid`]. Only JSON has its own reader. That is deliberate:
//! three near-identical row loops would drift.
//!
//! # Format is sniffed, never declared
//!
//! `Content-Type` is not trusted — the same posture the PDF upload takes with
//! `%PDF-`. `.xlsx` and `.docx` are *both* ZIP containers opening `PK\x03\x04`,
//! so the magic bytes only get us as far as "a ZIP"; which one it is is decided
//! by what is inside it (`xl/workbook.xml` vs `word/document.xml`).
//!
//! # The `.docx` template is exactly one shape
//!
//! A Word document has no inherent MCQ structure, and silently misreading an
//! answer key is far worse than refusing a file. So exactly **one** template is
//! accepted — the first table in the document, whose first row is the same
//! header row the CSV uses — and anything else is rejected with a message
//! naming that template. There is no heuristic parsing of free-form layouts
//! here, by design.

use std::io::Cursor;

use dg_core::{AssessmentType, DifficultyLevel, PublicError};
use uuid::Uuid;

/// Every paper is A/B/C/D.
pub(crate) const OPTION_COUNT: usize = 4;

/// Rows one import may carry. Not a performance limit — the write is a single
/// statement — but a bound on how much a validation failure has to report, and
/// on how large a body is worth buffering.
pub(crate) const MAX_BULK_ROWS: usize = 500;

/// Body cap for the bulk route, applied per route like the PDF upload's is.
/// `.xlsx` and `.docx` are compressed, so 500 rows is far under this; axum's
/// 2 MiB default stays in force on every other JSON endpoint.
pub(crate) const MAX_BULK_IMPORT_BYTES: usize = 8 * 1024 * 1024;

/// Decompressed size cap for one entry inside an imported ZIP container.
///
/// A 4 MiB `.docx` can declare a multi-gigabyte `word/document.xml`; reading it
/// unbounded is a zip-bomb. The cap is generous for a real document — 500 rows
/// of question text is well under 1 MiB of XML — and is applied to the read
/// itself, not to the header's claimed size, which an attacker controls.
const MAX_ZIP_ENTRY_BYTES: u64 = 32 * 1024 * 1024;

/// Which channel a body was sniffed as. Echoed back on success so an author who
/// meant to send a spreadsheet and sent something else can see what happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImportFormat {
    Json,
    Csv,
    Xlsx,
    Docx,
}

impl ImportFormat {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            ImportFormat::Json => "json",
            ImportFormat::Csv => "csv",
            ImportFormat::Xlsx => "xlsx",
            ImportFormat::Docx => "docx",
        }
    }
}

/// The fields every channel supplies, parsed but not yet checked.
///
/// One struct for the form and all four imports so the two cannot drift.
#[derive(Debug, Clone)]
pub(crate) struct QuestionFields {
    /// The pool a question belongs to. Required: the pool is built per course.
    pub course_id: Uuid,
    /// The optional unit/module pointer. When present it must belong to
    /// `course_id` — checked in the write path, because a CHECK constraint
    /// cannot contain the necessary subquery.
    pub block_id: Option<Uuid>,
    pub topic: String,
    pub question_text: String,
    pub options: Vec<String>,
    pub correct_option_index: i16,
    pub explanation: String,
    pub assessment_type: AssessmentType,
    pub difficulty_level: DifficultyLevel,
}

/// Validate one question's fields, returning a message naming what is wrong.
///
/// The message is the human-readable half of a `VALIDATION_ERROR`; the bulk
/// paths prefix it with a row number, so the wording deliberately does not
/// assume a row context.
pub(crate) fn validate_fields(f: &QuestionFields) -> Result<(), String> {
    if f.topic.trim().is_empty() {
        return Err("topic is required".into());
    }
    if f.topic.chars().count() > 200 {
        return Err("topic must be at most 200 characters".into());
    }
    if f.question_text.trim().is_empty() {
        return Err("question_text is required".into());
    }
    if f.question_text.chars().count() > 2000 {
        return Err("question_text must be at most 2000 characters".into());
    }
    if f.options.len() != OPTION_COUNT {
        return Err(format!(
            "exactly {OPTION_COUNT} options are required (A, B, C and D), got {}",
            f.options.len()
        ));
    }
    if let Some(position) = f.options.iter().position(|o| o.trim().is_empty()) {
        return Err(format!("option {} is blank", option_label(position)));
    }
    if !(0..OPTION_COUNT as i16).contains(&f.correct_option_index) {
        return Err(format!(
            "correct_option_index must be between 0 and {} (A to D)",
            OPTION_COUNT - 1
        ));
    }
    if f.explanation.trim().is_empty() {
        return Err("explanation is required".into());
    }
    Ok(())
}

/// `0 -> 'A'`, for an error message an author can act on without counting from
/// zero.
pub(crate) fn option_label(index: usize) -> char {
    char::from(b'A' + (index as u8 % 26))
}

// ---------------------------------------------------------------------------
// Sniffing
// ---------------------------------------------------------------------------

/// ZIP local-file-header signature. Both `.xlsx` and `.docx` start with it.
const ZIP_MAGIC: [u8; 4] = [b'P', b'K', 0x03, 0x04];

/// Decide the format from the bytes.
///
/// A ZIP is opened far enough to see which OOXML part it carries; the extension
/// and the declared `Content-Type` are never consulted, because both are
/// attacker- or mistake-controlled and the two formats are indistinguishable by
/// magic bytes alone.
pub(crate) fn sniff_format(body: &[u8]) -> Result<ImportFormat, PublicError> {
    if body.starts_with(&ZIP_MAGIC) {
        return sniff_ooxml(body);
    }

    let first = body
        .iter()
        .find(|b| !b.is_ascii_whitespace())
        .ok_or_else(|| PublicError::validation("file", "the import body is empty"))?;

    match first {
        b'[' => Ok(ImportFormat::Json),
        // A CSV import must start with its header row, which begins with a
        // column name. Anything else (`{`, an unknown binary signature) is
        // none of the four channels and is rejected rather than half-parsed.
        b if b.is_ascii_alphabetic() => Ok(ImportFormat::Csv),
        _ => Err(unsupported_format()),
    }
}

fn unsupported_format() -> PublicError {
    PublicError::validation(
        "file",
        "the import must be a JSON array of questions, a CSV or .xlsx with the \
         documented header row, or a .docx containing that header row as the \
         first row of its first table",
    )
}

/// Which OOXML package this is, from its entry names.
fn sniff_ooxml(body: &[u8]) -> Result<ImportFormat, PublicError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(body)).map_err(|_| {
        PublicError::validation(
            "file",
            "the uploaded file looks like a .xlsx or .docx but could not be opened",
        )
    })?;

    let mut is_xlsx = false;
    let mut is_docx = false;
    for index in 0..archive.len() {
        // `by_index_raw` does not decompress: this only reads the entry names.
        let Ok(entry) = archive.by_index_raw(index) else {
            continue;
        };
        match entry.name() {
            "xl/workbook.xml" => is_xlsx = true,
            "word/document.xml" => is_docx = true,
            _ => {}
        }
    }

    match (is_xlsx, is_docx) {
        (true, _) => Ok(ImportFormat::Xlsx),
        (false, true) => Ok(ImportFormat::Docx),
        (false, false) => Err(unsupported_format()),
    }
}

// ---------------------------------------------------------------------------
// The shared tabular reader
// ---------------------------------------------------------------------------

/// The header, in any order, case-insensitive. `block_id` and
/// `difficulty_level` are the two optional columns.
pub(crate) const COLUMNS: [&str; 12] = [
    "course_id",
    "block_id",
    "topic",
    "question_text",
    "option_a",
    "option_b",
    "option_c",
    "option_d",
    "correct_option",
    "explanation",
    "assessment_type",
    "difficulty_level",
];

/// Accepted spellings of the answer-key column, canonical first.
///
/// `correct_option` is what the console documents and what an admin types. The
/// JSON body's field is `correct_option_index`, so that spelling is accepted
/// too: a spreadsheet produced by dumping a JSON export must import without an
/// invisible rename step.
const ANSWER_KEY_COLUMNS: [&str; 2] = ["correct_option", "correct_option_index"];

/// Columns a table must carry, other than the answer key (which has two
/// accepted spellings and is checked separately).
const REQUIRED_COLUMNS: [&str; 9] = [
    "course_id",
    "topic",
    "question_text",
    "option_a",
    "option_b",
    "option_c",
    "option_d",
    "explanation",
    "assessment_type",
];

/// Normalise a header cell: trim, strip a UTF-8 BOM, lowercase.
fn normalise_header(raw: &str) -> String {
    raw.trim()
        .trim_start_matches('\u{feff}')
        .trim()
        .to_ascii_lowercase()
}

/// Parse an enum-valued cell, naming the legal values in the error — an import
/// rejected with "invalid" and nothing else cannot be fixed.
fn parse_enum<T: serde::de::DeserializeOwned>(
    value: &str,
    field: &str,
    legal: &str,
) -> Result<T, String> {
    serde_json::from_value::<T>(serde_json::Value::String(value.trim().to_ascii_lowercase()))
        .map_err(|_| format!("{field} must be one of {legal}, got `{}`", value.trim()))
}

/// The answer key in a **tabular** import: a single letter `A`-`D`,
/// case-insensitive.
///
/// Letters only, deliberately. A digit would have to mean either a zero-based
/// index (`1` = B) or a one-based position (`1` = A), and there is no way to tell
/// which an admin meant — so `1` would silently mark the wrong option correct on
/// half the imports. That is the one bug class in this whole module worth being
/// inflexible about, and the console documents `A`/`B`/`C`/`D` for exactly this
/// reason. The JSON channel keeps the machine-readable zero-based
/// `correct_option_index`, where the base is unambiguous because it is typed.
pub(crate) fn parse_correct_option(value: &str) -> Result<i16, String> {
    let raw = value.trim();
    let mut letters = raw.chars();
    match (letters.next(), letters.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => {
            let index = (c.to_ascii_uppercase() as u8 - b'A') as i16;
            if (0..OPTION_COUNT as i16).contains(&index) {
                Ok(index)
            } else {
                Err(format!("correct_option must be A, B, C or D, got `{raw}`"))
            }
        }
        _ if raw.chars().all(|c| c.is_ascii_digit()) && !raw.is_empty() => Err(format!(
            "correct_option must be the letter A, B, C or D, not a number — `{raw}` is \
             ambiguous (is option 1 the first option, or the one after it?)"
        )),
        _ => Err(format!("correct_option must be A, B, C or D, got `{raw}`")),
    }
}

/// Turn a header row plus a grid of cells into validated fields, collecting
/// **every** row's errors.
///
/// Shared by CSV, `.xlsx` and `.docx`: the three differ only in how they
/// produce the grid, and must not differ in how the grid is judged.
pub(crate) fn parse_grid(
    header_cells: &[String],
    rows: &[Vec<String>],
) -> Result<Vec<QuestionFields>, PublicError> {
    let header: Vec<String> = header_cells.iter().map(|h| normalise_header(h)).collect();
    let column = |name: &str| header.iter().position(|h| h == name);
    // The answer key under either accepted spelling.
    let key_column = ANSWER_KEY_COLUMNS.iter().copied().find_map(column);

    let mut missing: Vec<&str> = REQUIRED_COLUMNS
        .iter()
        .copied()
        .filter(|name| column(name).is_none())
        .collect();
    if key_column.is_none() {
        missing.push(ANSWER_KEY_COLUMNS[0]);
    }
    if !missing.is_empty() {
        return Err(missing_columns(&missing));
    }

    // A duplicated column is ambiguous — which of the two `option_b` columns is
    // option B? — and a merged header cell in a spreadsheet is one of the ways it
    // happens. Rejecting is the rule for this whole module: never guess at an
    // answer key's layout.
    for (index, name) in header.iter().enumerate() {
        if name.is_empty() {
            continue;
        }
        if header.iter().skip(index + 1).any(|other| other == name) {
            return Err(PublicError::validation(
                "file",
                format!(
                    "the header row names `{name}` more than once. Each column must appear \
                     exactly once; the expected header is: {}",
                    COLUMNS.join(", ")
                ),
            ));
        }
    }

    if rows.is_empty() {
        return Err(PublicError::validation(
            "file",
            "the import has a header row but no question rows",
        ));
    }
    if rows.len() > MAX_BULK_ROWS {
        return Err(too_many_rows(rows.len()));
    }

    let mut parsed = Vec::with_capacity(rows.len());
    let mut errors: Vec<String> = Vec::new();

    for (index, cells) in rows.iter().enumerate() {
        // Row 1 is the first *question*, not the header: that is how an author
        // counts their own data.
        let row = index + 1;
        let cell = |name: &str| -> String {
            column(name)
                .and_then(|i| cells.get(i))
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };

        let course_id = match Uuid::parse_str(&cell("course_id")) {
            Ok(id) => id,
            Err(_) => {
                errors.push(format!("row {row}: course_id is not a valid UUID"));
                continue;
            }
        };

        // Blank means "no unit/module", which is now a legitimate question.
        let raw_block = cell("block_id");
        let block_id = if raw_block.is_empty() {
            None
        } else {
            match Uuid::parse_str(&raw_block) {
                Ok(id) => Some(id),
                Err(_) => {
                    errors.push(format!(
                        "row {row}: block_id is not a valid UUID (leave it blank for a \
                         course-wide question)"
                    ));
                    continue;
                }
            }
        };

        let raw_key = key_column
            .and_then(|i| cells.get(i))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let correct_option_index = match parse_correct_option(&raw_key) {
            Ok(n) => n,
            Err(message) => {
                errors.push(format!("row {row}: {message}"));
                continue;
            }
        };
        let assessment_type = match parse_enum::<AssessmentType>(
            &cell("assessment_type"),
            "assessment_type",
            "assignment, mid_term_quiz, semester_exam",
        ) {
            Ok(v) => v,
            Err(message) => {
                errors.push(format!("row {row}: {message}"));
                continue;
            }
        };
        let raw_difficulty = cell("difficulty_level");
        let difficulty_level = if raw_difficulty.is_empty() {
            DifficultyLevel::Beginner
        } else {
            match parse_enum::<DifficultyLevel>(
                &raw_difficulty,
                "difficulty_level",
                "beginner, intermediate, advanced",
            ) {
                Ok(v) => v,
                Err(message) => {
                    errors.push(format!("row {row}: {message}"));
                    continue;
                }
            }
        };

        let fields = QuestionFields {
            course_id,
            block_id,
            topic: cell("topic"),
            question_text: cell("question_text"),
            options: vec![
                cell("option_a"),
                cell("option_b"),
                cell("option_c"),
                cell("option_d"),
            ],
            correct_option_index,
            explanation: cell("explanation"),
            assessment_type,
            difficulty_level,
        };

        if let Err(message) = validate_fields(&fields) {
            errors.push(format!("row {row}: {message}"));
            continue;
        }
        parsed.push(fields);
    }

    if !errors.is_empty() {
        return Err(row_errors(errors));
    }
    Ok(parsed)
}

/// The "wrong header" answer, quoting the expected one back. Shared so CSV,
/// `.xlsx` and `.docx` all tell an author the same thing.
fn missing_columns(missing: &[&str]) -> PublicError {
    PublicError::validation(
        "file",
        format!(
            "the import is missing required columns: {}. The expected header is: {}",
            missing.join(", "),
            COLUMNS.join(", ")
        ),
    )
}

pub(crate) fn too_many_rows(count: usize) -> PublicError {
    PublicError::validation(
        "file",
        format!("an import may contain at most {MAX_BULK_ROWS} questions, got {count}"),
    )
}

/// Per-row failures as one `VALIDATION_ERROR`.
///
/// The envelope is fixed (`api-conventions.md`), so the detail goes in the
/// message: `row N: ...` clauses joined by `"; "`, ending with its own
/// `nothing was imported` clause, which the console splits on `"; "` and
/// renders as a checklist. Capped so a 500-row import with 500 mistakes does
/// not return a response nobody can read — the count is still reported.
pub(crate) fn row_errors(errors: Vec<String>) -> PublicError {
    const MAX_REPORTED: usize = 20;
    let total = errors.len();
    let mut message = errors
        .iter()
        .take(MAX_REPORTED)
        .cloned()
        .collect::<Vec<_>>()
        .join("; ");
    if total > MAX_REPORTED {
        message.push_str(&format!("; and {} further rows", total - MAX_REPORTED));
    }
    message.push_str("; nothing was imported");
    PublicError::validation("questions", message)
}

// ---------------------------------------------------------------------------
// CSV
// ---------------------------------------------------------------------------

/// Split one CSV line, honouring double quotes and `""` escapes.
///
/// Hand-rolled rather than adding a dependency for twelve columns. It handles
/// exactly what the format needs — quoted fields containing commas, quotes and
/// newlines — and treats anything else as literal text, because a question body
/// is prose and has to survive round-tripping.
fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    current.push('"');
                } else {
                    in_quotes = false;
                }
            }
            '"' => in_quotes = true,
            ',' if !in_quotes => fields.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    fields.push(current);
    fields
}

/// Logical CSV lines: a newline inside a quoted field does not end a row.
fn csv_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for c in text.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                current.push(c);
            }
            '\n' if !in_quotes => {
                lines.push(std::mem::take(&mut current));
            }
            '\r' if !in_quotes => {}
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        lines.push(current);
    }

    lines.into_iter().filter(|l| !l.trim().is_empty()).collect()
}

pub(crate) fn parse_csv(text: &str) -> Result<Vec<QuestionFields>, PublicError> {
    let lines = csv_lines(text);
    let Some(header_line) = lines.first() else {
        return Err(PublicError::validation("file", "the CSV has no header row"));
    };

    let header = split_csv_line(header_line);
    let rows: Vec<Vec<String>> = lines[1..].iter().map(|l| split_csv_line(l)).collect();
    parse_grid(&header, &rows)
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

/// One row of a JSON import. Identical to the create body, so an author can
/// post the same object singly or in an array.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct BulkQuestionRow {
    pub course_id: Uuid,
    /// Optional unit/module.
    #[serde(default)]
    pub block_id: Option<Uuid>,
    pub topic: String,
    pub question_text: String,
    pub options: Vec<String>,
    pub correct_option_index: i16,
    pub explanation: String,
    pub assessment_type: AssessmentType,
    #[serde(default)]
    pub difficulty_level: Option<DifficultyLevel>,
}

impl From<BulkQuestionRow> for QuestionFields {
    fn from(r: BulkQuestionRow) -> Self {
        Self {
            course_id: r.course_id,
            block_id: r.block_id,
            topic: r.topic,
            question_text: r.question_text,
            options: r.options,
            correct_option_index: r.correct_option_index,
            explanation: r.explanation,
            assessment_type: r.assessment_type,
            difficulty_level: r.difficulty_level.unwrap_or(DifficultyLevel::Beginner),
        }
    }
}

pub(crate) fn parse_json(body: &[u8]) -> Result<Vec<QuestionFields>, PublicError> {
    // Deserialised as `Value` first so one bad row reports *as a row* rather
    // than failing the whole document with serde's byte offset, which an author
    // cannot map back to a question.
    let rows: Vec<serde_json::Value> = serde_json::from_slice(body).map_err(|_| {
        PublicError::validation("file", "the import is not a valid JSON array of questions")
    })?;

    if rows.is_empty() {
        return Err(PublicError::validation(
            "file",
            "the import contains no questions",
        ));
    }
    if rows.len() > MAX_BULK_ROWS {
        return Err(too_many_rows(rows.len()));
    }

    let mut parsed = Vec::with_capacity(rows.len());
    let mut errors: Vec<String> = Vec::new();

    for (index, value) in rows.into_iter().enumerate() {
        let row = index + 1;
        match serde_json::from_value::<BulkQuestionRow>(value) {
            Ok(parsed_row) => {
                let fields = QuestionFields::from(parsed_row);
                match validate_fields(&fields) {
                    Ok(()) => parsed.push(fields),
                    Err(message) => errors.push(format!("row {row}: {message}")),
                }
            }
            Err(e) => errors.push(format!("row {row}: {e}")),
        }
    }

    if !errors.is_empty() {
        return Err(row_errors(errors));
    }
    Ok(parsed)
}

// ---------------------------------------------------------------------------
// XLSX
// ---------------------------------------------------------------------------

/// Parse the **first worksheet** of an `.xlsx` workbook.
///
/// The first sheet, not a named one: an admin exporting from the owner's
/// template has one sheet, and guessing among several is the kind of silent
/// choice this module refuses to make elsewhere.
pub(crate) fn parse_xlsx(body: &[u8]) -> Result<Vec<QuestionFields>, PublicError> {
    use calamine::{Data, Reader};

    let mut workbook = calamine::open_workbook_auto_from_rs(Cursor::new(body))
        .map_err(|_| PublicError::validation("file", "the .xlsx workbook could not be read"))?;

    let sheet_names = workbook.sheet_names().to_vec();
    let Some(first) = sheet_names.first() else {
        return Err(PublicError::validation(
            "file",
            "the .xlsx workbook has no worksheets",
        ));
    };

    let range = workbook
        .worksheet_range(first)
        .map_err(|_| PublicError::validation("file", "the first worksheet could not be read"))?;

    /// A cell as text. A number typed into `correct_option_index` arrives as a
    /// float, so `1` must not render as `1.0` and fail the parse.
    fn cell_text(cell: &Data) -> String {
        match cell {
            Data::Empty => String::new(),
            Data::String(s) => s.clone(),
            Data::Float(f) if f.fract() == 0.0 => format!("{}", *f as i64),
            Data::Float(f) => f.to_string(),
            Data::Int(i) => i.to_string(),
            Data::Bool(b) => b.to_string(),
            other => other.to_string(),
        }
    }

    let mut grid = range
        .rows()
        .map(|row| row.iter().map(cell_text).collect::<Vec<String>>())
        // A trailing run of blank rows is what a spreadsheet looks like when
        // someone has deleted content; it is not 300 empty questions.
        .filter(|row| row.iter().any(|c| !c.trim().is_empty()))
        .collect::<Vec<Vec<String>>>();

    if grid.is_empty() {
        return Err(PublicError::validation(
            "file",
            "the first worksheet is empty",
        ));
    }

    let header = grid.remove(0);
    parse_grid(&header, &grid)
}

// ---------------------------------------------------------------------------
// DOCX
// ---------------------------------------------------------------------------

/// The one accepted `.docx` template, quoted back at an author whose document
/// does not match it.
fn docx_template_error(detail: &str) -> PublicError {
    PublicError::validation(
        "file",
        format!(
            "{detail} A .docx import must contain exactly one supported layout: a table \
             whose first row is the header row `{}` and whose remaining rows are one \
             question each. Free-form Word documents are not parsed, because \
             misreading an answer key is worse than refusing the file — export to \
             .csv or .xlsx instead.",
            COLUMNS.join(", ")
        ),
    )
}

/// Read `word/document.xml` out of a `.docx`, bounded against a zip bomb.
fn docx_document_xml(body: &[u8]) -> Result<String, PublicError> {
    use std::io::Read;

    let mut archive = zip::ZipArchive::new(Cursor::new(body))
        .map_err(|_| docx_template_error("The .docx could not be opened."))?;
    let entry = archive
        .by_name("word/document.xml")
        .map_err(|_| docx_template_error("The .docx has no main document part."))?;

    let mut xml = String::new();
    // `take` bounds the *decompressed* read: the entry header's declared size is
    // attacker-controlled and must not be trusted to allocate against.
    entry
        .take(MAX_ZIP_ENTRY_BYTES)
        .read_to_string(&mut xml)
        .map_err(|_| docx_template_error("The .docx main document part is not valid text."))?;

    Ok(xml)
}

/// Extract the first `w:tbl` as a grid of cell texts.
///
/// A cell's text is every `w:t` run inside it concatenated, which is how Word
/// splits a single typed sentence when it records spell-check or formatting
/// boundaries — reading only the first run would silently truncate a question.
fn docx_first_table(xml: &str) -> Result<Vec<Vec<String>>, PublicError> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    let mut table: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut cell = String::new();

    let mut depth_tbl = 0usize;
    let mut in_row = false;
    let mut in_cell = false;
    let mut in_text = false;
    let mut finished = false;

    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                // A nested table's rows would otherwise be read as rows of the
                // outer one. Counting depth means only the outermost table is
                // taken, and a nested one is ignored rather than interleaved.
                b"tbl" => depth_tbl += 1,
                b"tr" if depth_tbl == 1 => {
                    in_row = true;
                    row.clear();
                }
                b"tc" if depth_tbl == 1 && in_row => {
                    in_cell = true;
                    cell.clear();
                }
                // `depth_tbl == 1` as well as `in_cell`: a nested table sits
                // *inside* an outer cell, so without the depth guard its text
                // would be appended to that cell and shift every column of the
                // outer row.
                b"t" if in_cell && depth_tbl == 1 => in_text = true,
                _ => {}
            },
            Ok(Event::Text(t)) if in_text => {
                // `unescape` turns `&amp;` and friends back into text: a
                // question body is prose and an escaped ampersand must not
                // survive into the stored question.
                if let Ok(text) = t.unescape() {
                    cell.push_str(text.as_ref());
                }
            }
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                b"t" => in_text = false,
                b"tc" if in_cell => {
                    in_cell = false;
                    row.push(std::mem::take(&mut cell));
                }
                b"tr" if depth_tbl == 1 && in_row => {
                    in_row = false;
                    if row.iter().any(|c| !c.trim().is_empty()) {
                        table.push(std::mem::take(&mut row));
                    } else {
                        row.clear();
                    }
                }
                b"tbl" => {
                    depth_tbl = depth_tbl.saturating_sub(1);
                    if depth_tbl == 0 && !table.is_empty() {
                        // The *first* table only: a document may legitimately
                        // carry a second, unrelated table and guessing between
                        // them is exactly the heuristic this refuses.
                        finished = true;
                    }
                }
                _ => {}
            },
            // Malformed XML is a corrupt file, not something to salvage.
            Err(_) => return Err(docx_template_error("The .docx could not be parsed.")),
            _ => {}
        }
        if finished {
            break;
        }
    }

    if table.is_empty() {
        return Err(docx_template_error("The .docx contains no table."));
    }
    Ok(table)
}

pub(crate) fn parse_docx(body: &[u8]) -> Result<Vec<QuestionFields>, PublicError> {
    let xml = docx_document_xml(body)?;
    let mut table = docx_first_table(&xml)?;

    let header = table.remove(0);
    // Checked before `parse_grid` so the failure quotes the template rather
    // than reading as a generic missing-column error: a Word document that is
    // simply the wrong shape is the common mistake here, and the author needs
    // to be told what shape to use.
    let normalised: Vec<String> = header.iter().map(|h| normalise_header(h)).collect();
    let mut missing: Vec<&str> = REQUIRED_COLUMNS
        .iter()
        .copied()
        .filter(|name| !normalised.iter().any(|h| h == name))
        .collect();
    if !ANSWER_KEY_COLUMNS
        .iter()
        .any(|name| normalised.iter().any(|h| h == name))
    {
        missing.push(ANSWER_KEY_COLUMNS[0]);
    }
    if !missing.is_empty() {
        return Err(docx_template_error(&format!(
            "The first table's header row is missing: {}.",
            missing.join(", ")
        )));
    }

    parse_grid(&header, &table)
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Sniff and parse, returning the rows and which channel they came from.
pub(crate) fn parse_import(
    body: &[u8],
) -> Result<(ImportFormat, Vec<QuestionFields>), PublicError> {
    let format = sniff_format(body)?;
    let rows = match format {
        ImportFormat::Json => parse_json(body)?,
        ImportFormat::Csv => {
            let text = std::str::from_utf8(body).map_err(|_| {
                PublicError::validation("file", "the CSV import must be UTF-8 encoded")
            })?;
            parse_csv(text)?
        }
        ImportFormat::Xlsx => parse_xlsx(body)?,
        ImportFormat::Docx => parse_docx(body)?,
    };
    Ok((format, rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn fields() -> QuestionFields {
        QuestionFields {
            course_id: Uuid::new_v4(),
            block_id: None,
            topic: "Phonology".into(),
            question_text: "What is sandhi?".into(),
            options: vec![
                "Euphonic combination".into(),
                "Verb conjugation".into(),
                "Case marking".into(),
                "Metre".into(),
            ],
            correct_option_index: 0,
            explanation: "Sandhi is the euphonic combination of adjacent sounds.".into(),
            assessment_type: AssessmentType::MidTermQuiz,
            difficulty_level: DifficultyLevel::Beginner,
        }
    }

    // -- Field validation -------------------------------------------------

    #[test]
    fn a_well_formed_question_validates() {
        assert!(validate_fields(&fields()).is_ok());
    }

    #[test]
    fn a_question_must_have_exactly_four_options() {
        let mut f = fields();
        f.options.pop();
        assert!(validate_fields(&f).is_err());

        let mut f = fields();
        f.options.push("Extra".into());
        assert!(validate_fields(&f).is_err());
    }

    #[test]
    fn a_blank_option_is_rejected_and_named_by_letter() {
        let mut f = fields();
        f.options[2] = "   ".into();
        let message = validate_fields(&f).expect_err("blank option");
        assert!(message.contains('C'), "got: {message}");
    }

    #[test]
    fn the_answer_key_must_point_at_one_of_the_four_options() {
        let mut f = fields();
        f.correct_option_index = 4;
        assert!(validate_fields(&f).is_err());
        f.correct_option_index = -1;
        assert!(validate_fields(&f).is_err());
    }

    #[test]
    fn an_explanation_is_mandatory() {
        let mut f = fields();
        f.explanation = "  ".into();
        assert!(validate_fields(&f).is_err());
    }

    #[test]
    fn a_topic_is_mandatory_because_weak_area_tagging_rolls_up_by_it() {
        let mut f = fields();
        f.topic = String::new();
        assert!(validate_fields(&f).is_err());
    }

    // -- Sniffing ---------------------------------------------------------

    #[test]
    fn the_import_format_is_sniffed_from_the_bytes_not_the_content_type() {
        assert_eq!(sniff_format(b"  [\n{}]").expect("json"), ImportFormat::Json);
        assert_eq!(
            sniff_format(b"course_id,topic\n").expect("csv"),
            ImportFormat::Csv
        );
        assert!(
            sniff_format(b"{\"course_id\":\"x\"}").is_err(),
            "a bare JSON object is none of the four channels"
        );
        assert!(sniff_format(b"%PDF-1.7").is_err());
        assert!(sniff_format(b"   ").is_err(), "an empty body is rejected");
    }

    /// `.xlsx` and `.docx` are the same magic bytes, so a ZIP carrying neither
    /// OOXML part must be refused rather than guessed at.
    #[test]
    fn a_zip_that_is_neither_xlsx_nor_docx_is_rejected() {
        let zip = build_zip(&[("hello.txt", b"hi".to_vec())]);
        assert!(zip.starts_with(&ZIP_MAGIC));
        let err = sniff_format(&zip).expect_err("not an OOXML package");
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn a_truncated_zip_is_a_validation_error_not_a_panic() {
        let mut zip = build_zip(&[("xl/workbook.xml", b"<x/>".to_vec())]);
        zip.truncate(12);
        assert!(sniff_format(&zip).is_err());
    }

    // -- CSV --------------------------------------------------------------

    const CSV_HEADER: &str = "course_id,block_id,topic,question_text,option_a,option_b,option_c,option_d,correct_option,explanation,assessment_type,difficulty_level";

    fn csv_row(course_id: Uuid, block: &str, index: &str) -> String {
        format!(
            "{course_id},{block},Phonology,\"What is sandhi, exactly?\",A,B,C,D,{index},\"Because, euphony\",mid_term_quiz,beginner"
        )
    }

    #[test]
    fn a_csv_import_parses_quoted_fields_containing_commas() {
        let course_id = Uuid::new_v4();
        let csv = format!("{CSV_HEADER}\n{}\n", csv_row(course_id, "", "C"));
        let rows = parse_csv(&csv).expect("parses");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].course_id, course_id);
        assert_eq!(rows[0].question_text, "What is sandhi, exactly?");
        assert_eq!(rows[0].explanation, "Because, euphony");
        assert_eq!(rows[0].correct_option_index, 2, "the letter C is index 2");
        assert_eq!(rows[0].options.len(), 4);
    }

    /// The unit/module is optional: a blank `block_id` cell is a course-wide
    /// question, not a parse failure.
    #[test]
    fn a_blank_block_id_means_a_course_wide_question() {
        let csv = format!("{CSV_HEADER}\n{}\n", csv_row(Uuid::new_v4(), "", "B"));
        let rows = parse_csv(&csv).expect("parses");
        assert!(rows[0].block_id.is_none());
    }

    #[test]
    fn a_present_block_id_is_carried_through() {
        let block = Uuid::new_v4();
        let csv = format!(
            "{CSV_HEADER}\n{}\n",
            csv_row(Uuid::new_v4(), &block.to_string(), "B")
        );
        let rows = parse_csv(&csv).expect("parses");
        assert_eq!(rows[0].block_id, Some(block));
    }

    #[test]
    fn a_missing_course_id_is_rejected_because_the_pool_is_per_course() {
        let header = "block_id,topic,question_text,option_a,option_b,option_c,option_d,correct_option,explanation,assessment_type";
        let err = parse_csv(&format!("{header}\n,,,,,,,,,\n")).expect_err("no course_id");
        assert!(err.public_message().contains("course_id"));
    }

    /// The answer key in a table is a letter, case-insensitive — and nothing
    /// else. This is the authoritative rule the console documents.
    #[test]
    fn a_tabular_answer_key_is_a_letter_a_to_d() {
        assert_eq!(parse_correct_option("A").expect("letter"), 0);
        assert_eq!(parse_correct_option("b").expect("lowercase"), 1);
        assert_eq!(parse_correct_option(" D ").expect("padded"), 3);
        assert!(
            parse_correct_option("E").is_err(),
            "only four options exist"
        );
        assert!(parse_correct_option("banana").is_err());
        assert!(parse_correct_option("").is_err());
    }

    /// A digit is **refused**, not interpreted. `1` could mean "the first
    /// option" or "index 1", and guessing wrong silently marks the wrong answer
    /// correct — the one failure mode in this module worth being rigid about.
    #[test]
    fn a_numeric_answer_key_is_refused_as_ambiguous_rather_than_guessed() {
        for digit in ["0", "1", "2", "3", "4"] {
            let message = parse_correct_option(digit).expect_err("digits are ambiguous");
            assert!(
                message.contains("ambiguous"),
                "the author must be told why, got: {message}"
            );
        }
    }

    /// The JSON channel keeps the zero-based index, where the base is explicit
    /// because it is typed rather than typed-in.
    #[test]
    fn the_json_channel_still_takes_a_zero_based_index() {
        let body = serde_json::to_vec(&vec![json_row(Uuid::new_v4(), 0)]).expect("serialises");
        let rows = parse_json(&body).expect("parses");
        assert_eq!(rows[0].correct_option_index, 0, "0 is option A");
    }

    /// The older `correct_option_index` spelling stays accepted as a header, so
    /// a spreadsheet built from a JSON export imports without a rename. The
    /// *value* is still a letter: the column name does not change the format.
    #[test]
    fn the_legacy_answer_key_column_name_is_still_accepted() {
        let header = CSV_HEADER.replace("correct_option", "correct_option_index");
        let csv = format!(
            "{header}
{}
",
            csv_row(Uuid::new_v4(), "", "C")
        );
        let rows = parse_csv(&csv).expect("parses");
        assert_eq!(rows[0].correct_option_index, 2);
    }

    /// A duplicated column is ambiguous — and a merged header cell in a
    /// spreadsheet is one way it arises — so it is rejected, not resolved.
    #[test]
    fn a_duplicated_header_column_is_rejected() {
        let header = format!("{CSV_HEADER},option_b");
        let row = format!("{},X", csv_row(Uuid::new_v4(), "", "A"));
        let err = parse_csv(&format!(
            "{header}
{row}
"
        ))
        .expect_err("ambiguous header");
        assert!(err.public_message().contains("more than once"));
    }

    #[test]
    fn a_csv_without_the_optional_difficulty_column_defaults_to_beginner() {
        let header = CSV_HEADER.trim_end_matches(",difficulty_level");
        let course_id = Uuid::new_v4();
        let row = format!("{course_id},,Phonology,Q,A,B,C,D,B,Because,assignment");
        let rows = parse_csv(&format!("{header}\n{row}\n")).expect("parses");
        assert_eq!(rows[0].difficulty_level, DifficultyLevel::Beginner);
    }

    #[test]
    fn one_bad_row_rejects_the_whole_import_and_names_its_row_number() {
        let good = csv_row(Uuid::new_v4(), "", "B");
        let bad = csv_row(Uuid::new_v4(), "", "Z");
        let csv = format!("{CSV_HEADER}\n{good}\n{bad}\n{good}\n");

        let err = parse_csv(&csv).expect_err("row 2 is invalid");
        assert_eq!(err.code(), "VALIDATION_ERROR");
        let message = err.public_message();
        assert!(
            message
                .split("; ")
                .any(|clause| clause.contains("row 2: correct_option")),
            "each row's failure is its own clause: {message}"
        );
        assert!(
            message.split("; ").any(|c| c == "nothing was imported"),
            "the author must be told the import was rejected whole: {message}"
        );
    }

    #[test]
    fn every_bad_row_is_reported_not_just_the_first() {
        let bad_a = csv_row(Uuid::new_v4(), "", "Z");
        let bad_b = "not-a-uuid,,T,Q,A,B,C,D,A,E,assignment,beginner";
        let csv = format!("{CSV_HEADER}\n{bad_a}\n{bad_b}\n");

        let message = parse_csv(&csv).expect_err("both rows bad").public_message();
        assert!(
            message.contains("row 1") && message.contains("row 2"),
            "got: {message}"
        );
    }

    #[test]
    fn a_csv_row_cap_is_enforced() {
        let row = csv_row(Uuid::new_v4(), "", "B");
        let mut csv = String::from(CSV_HEADER);
        for _ in 0..=MAX_BULK_ROWS {
            csv.push('\n');
            csv.push_str(&row);
        }
        let err = parse_csv(&csv).expect_err("over the row cap");
        assert!(err.public_message().contains(&MAX_BULK_ROWS.to_string()));
    }

    #[test]
    fn a_header_only_csv_is_rejected() {
        assert!(parse_csv(&format!("{CSV_HEADER}\n")).is_err());
    }

    // -- JSON -------------------------------------------------------------

    fn json_row(course_id: Uuid, index: i16) -> serde_json::Value {
        serde_json::json!({
            "course_id": course_id,
            "topic": "Phonology",
            "question_text": "What is sandhi?",
            "options": ["A", "B", "C", "D"],
            "correct_option_index": index,
            "explanation": "Euphony.",
            "assessment_type": "semester_exam"
        })
    }

    #[test]
    fn a_json_array_import_parses_and_defaults_the_optional_fields() {
        let body = serde_json::to_vec(&vec![json_row(Uuid::new_v4(), 1)]).expect("serialises");
        let rows = parse_json(&body).expect("parses");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].difficulty_level, DifficultyLevel::Beginner);
        assert_eq!(rows[0].assessment_type, AssessmentType::SemesterExam);
        assert!(rows[0].block_id.is_none(), "the unit/module is optional");
    }

    #[test]
    fn a_json_import_reports_the_offending_row_number_not_a_byte_offset() {
        let body = serde_json::to_vec(&vec![
            json_row(Uuid::new_v4(), 1),
            json_row(Uuid::new_v4(), 7),
        ])
        .expect("serialises");

        let message = parse_json(&body)
            .expect_err("row 2 is invalid")
            .public_message();
        assert!(message.contains("row 2"), "got: {message}");
    }

    #[test]
    fn an_empty_json_array_is_rejected() {
        assert!(parse_json(b"[]").is_err());
    }

    #[test]
    fn a_malformed_json_body_is_a_validation_error_not_a_panic() {
        let err = parse_json(b"[{oops").expect_err("not JSON");
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn a_json_row_cap_is_enforced() {
        let rows: Vec<serde_json::Value> = (0..=MAX_BULK_ROWS)
            .map(|_| json_row(Uuid::new_v4(), 1))
            .collect();
        let body = serde_json::to_vec(&rows).expect("serialises");
        assert!(parse_json(&body).is_err());
    }

    // -- ZIP / XLSX / DOCX fixtures ---------------------------------------

    /// A minimal stored-entry ZIP, so the OOXML tests need no fixture files on
    /// disk and no Excel or Word to produce them.
    fn build_zip(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
        use std::io::Write;
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for (name, bytes) in entries {
                writer.start_file(*name, options).expect("entry starts");
                writer.write_all(bytes).expect("entry writes");
            }
            writer.finish().expect("archive finishes");
        }
        cursor.into_inner()
    }

    fn docx(body_xml: &str) -> Vec<u8> {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>{body_xml}</w:body></w:document>"#
        );
        build_zip(&[("word/document.xml", xml.into_bytes())])
    }

    fn docx_cell(text: &str) -> String {
        // Two runs on purpose: Word splits a typed sentence across `w:t`
        // elements, and a parser reading only the first would truncate it.
        let (head, tail) = text.split_at(text.len().min(1));
        format!("<w:tc><w:p><w:r><w:t>{head}</w:t></w:r><w:r><w:t>{tail}</w:t></w:r></w:p></w:tc>")
    }

    fn docx_row(cells: &[&str]) -> String {
        let inner: String = cells.iter().map(|c| docx_cell(c)).collect();
        format!("<w:tr>{inner}</w:tr>")
    }

    fn docx_table(rows: &[Vec<&str>]) -> String {
        let inner: String = rows.iter().map(|r| docx_row(r)).collect();
        format!("<w:tbl>{inner}</w:tbl>")
    }

    fn docx_header_cells() -> Vec<&'static str> {
        COLUMNS.to_vec()
    }

    /// A minimal single-sheet workbook with inline strings, so the `.xlsx` path
    /// is tested without a binary fixture in the repo and without Excel to
    /// produce one.
    fn xlsx(rows: &[Vec<&str>]) -> Vec<u8> {
        fn escape(s: &str) -> String {
            s.replace('&', "&amp;").replace('<', "&lt;")
        }

        let mut sheet_rows = String::new();
        for (r, cells) in rows.iter().enumerate() {
            let mut row_xml = String::new();
            for (c, value) in cells.iter().enumerate() {
                // Inline strings: no shared-strings part needed, and every value
                // in this format is text anyway.
                let reference = format!("{}{}", option_label(c), r + 1);
                row_xml.push_str(&format!(
                    r#"<c r="{reference}" t="inlineStr"><is><t>{}</t></is></c>"#,
                    escape(value)
                ));
            }
            sheet_rows.push_str(&format!(r#"<row r="{}">{row_xml}</row>"#, r + 1));
        }

        let sheet = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData>{sheet_rows}</sheetData></worksheet>"#
        );
        let workbook = r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<sheets><sheet name="Questions" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
        let workbook_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1"
 Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet"
 Target="worksheets/sheet1.xml"/></Relationships>"#;
        let root_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1"
 Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
 Target="xl/workbook.xml"/></Relationships>"#;
        let content_types = r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels"
 ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/xl/workbook.xml"
 ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
<Override PartName="/xl/worksheets/sheet1.xml"
 ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#;

        build_zip(&[
            ("[Content_Types].xml", content_types.as_bytes().to_vec()),
            ("_rels/.rels", root_rels.as_bytes().to_vec()),
            ("xl/workbook.xml", workbook.as_bytes().to_vec()),
            (
                "xl/_rels/workbook.xml.rels",
                workbook_rels.as_bytes().to_vec(),
            ),
            ("xl/worksheets/sheet1.xml", sheet.into_bytes()),
        ])
    }

    fn tabular_header() -> Vec<&'static str> {
        COLUMNS.to_vec()
    }

    #[test]
    fn an_xlsx_package_is_recognised_by_its_workbook_part() {
        let body = xlsx(&[tabular_header()]);
        assert!(body.starts_with(&ZIP_MAGIC), "an xlsx is a ZIP");
        assert_eq!(sniff_format(&body).expect("xlsx"), ImportFormat::Xlsx);
    }

    #[test]
    fn an_xlsx_first_worksheet_in_the_documented_template_parses() {
        let course_id = Uuid::new_v4();
        let course = course_id.to_string();
        let rows = vec![
            tabular_header(),
            vec![
                &course,
                "",
                "Phonology",
                "What is sandhi?",
                "Euphony",
                "Verbs",
                "Case",
                "Metre",
                "A",
                "Because euphony.",
                "semester_exam",
                "intermediate",
            ],
        ];

        let parsed = parse_xlsx(&xlsx(&rows)).expect("parses");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].course_id, course_id);
        assert_eq!(parsed[0].correct_option_index, 0, "the letter A is index 0");
        assert_eq!(parsed[0].difficulty_level, DifficultyLevel::Intermediate);
        assert!(parsed[0].block_id.is_none(), "the unit/module is optional");
        assert_eq!(parsed[0].assessment_type, AssessmentType::SemesterExam);
    }

    /// Renamed columns are refused, not guessed at — the rule the console states
    /// to admins before they upload.
    #[test]
    fn an_xlsx_with_renamed_columns_is_refused() {
        let rows = vec![vec!["Course", "Question", "Answer"], vec!["x", "y", "z"]];
        let err = parse_xlsx(&xlsx(&rows)).expect_err("renamed header");
        assert!(err.public_message().contains("course_id"));
    }

    #[test]
    fn an_xlsx_with_only_a_header_row_is_rejected() {
        assert!(parse_xlsx(&xlsx(&[tabular_header()])).is_err());
    }

    /// Blank rows left behind by deleted content are not empty questions.
    #[test]
    fn an_xlsx_trailing_blank_rows_are_not_questions() {
        let course_id = Uuid::new_v4();
        let course = course_id.to_string();
        let blank = vec![""; COLUMNS.len()];
        let rows = vec![
            tabular_header(),
            vec![
                &course,
                "",
                "Phonology",
                "Q",
                "A",
                "B",
                "C",
                "D",
                "B",
                "E",
                "assignment",
                "beginner",
            ],
            blank.clone(),
            blank,
        ];
        let parsed = parse_xlsx(&xlsx(&rows)).expect("parses");
        assert_eq!(parsed.len(), 1);
    }

    /// Dispatch must route a ZIP to the right reader, which is the only thing
    /// separating the two OOXML formats.
    #[test]
    fn dispatch_routes_an_xlsx_and_a_docx_to_different_readers() {
        let course = Uuid::new_v4().to_string();
        let data_row = vec![
            course.as_str(),
            "",
            "Phonology",
            "Q",
            "A",
            "B",
            "C",
            "D",
            "C",
            "E",
            "assignment",
            "beginner",
        ];

        let (format, rows) =
            parse_import(&xlsx(&[tabular_header(), data_row.clone()])).expect("xlsx parses");
        assert_eq!(format, ImportFormat::Xlsx);
        assert_eq!(rows[0].correct_option_index, 2);

        let (format, rows) =
            parse_import(&docx(&docx_table(&[tabular_header(), data_row]))).expect("docx parses");
        assert_eq!(format, ImportFormat::Docx);
        assert_eq!(rows[0].correct_option_index, 2);
    }

    #[test]
    fn a_docx_package_is_recognised_by_its_main_document_part() {
        let body = docx(&docx_table(&[docx_header_cells()]));
        assert_eq!(sniff_format(&body).expect("docx"), ImportFormat::Docx);
    }

    /// The one supported template: the first table, header row first.
    #[test]
    fn a_docx_table_in_the_documented_template_parses() {
        let course_id = Uuid::new_v4();
        let course = course_id.to_string();
        let rows = vec![
            docx_header_cells(),
            vec![
                &course,
                "",
                "Phonology",
                "What is sandhi?",
                "A",
                "B",
                "C",
                "D",
                "B",
                "Euphony.",
                "mid_term_quiz",
                "advanced",
            ],
        ];
        let parsed = parse_docx(&docx(&docx_table(&rows))).expect("parses");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].course_id, course_id);
        // Both runs of the cell were read, not just the first.
        assert_eq!(parsed[0].question_text, "What is sandhi?");
        assert_eq!(parsed[0].correct_option_index, 1, "the letter B is index 1");
        assert_eq!(parsed[0].difficulty_level, DifficultyLevel::Advanced);
        assert!(parsed[0].block_id.is_none());
    }

    /// The anti-heuristic requirement: a free-form Word document is refused
    /// with a message naming the template, never guessed at.
    #[test]
    fn a_free_form_docx_is_refused_and_the_template_is_named() {
        let prose = "<w:p><w:r><w:t>Q: What is sandhi?</w:t></w:r></w:p>\
                     <w:p><w:r><w:t>A) Euphony B) Verbs</w:t></w:r></w:p>\
                     <w:p><w:r><w:t>Ans: A</w:t></w:r></w:p>";
        let err = parse_docx(&docx(prose)).expect_err("no table, no import");
        assert_eq!(err.code(), "VALIDATION_ERROR");
        let message = err.public_message();
        assert!(message.contains("no table"), "got: {message}");
        assert!(
            message.contains("course_id") && message.contains("correct_option"),
            "the expected header must be quoted back: {message}"
        );
        assert!(
            message.contains(".csv") || message.contains(".xlsx"),
            "and the author told what to do instead: {message}"
        );
    }

    #[test]
    fn a_docx_table_with_the_wrong_header_names_the_template() {
        let rows = vec![vec!["question", "answer"], vec!["Q", "A"]];
        let message = parse_docx(&docx(&docx_table(&rows)))
            .expect_err("wrong header")
            .public_message();
        assert!(message.contains("course_id"), "got: {message}");
    }

    #[test]
    fn a_docx_with_no_question_rows_is_rejected() {
        let rows = vec![docx_header_cells()];
        assert!(parse_docx(&docx(&docx_table(&rows))).is_err());
    }

    #[test]
    fn a_docx_row_cap_is_enforced() {
        let course = Uuid::new_v4().to_string();
        let mut rows = vec![docx_header_cells()];
        for _ in 0..=MAX_BULK_ROWS {
            rows.push(vec![
                &course,
                "",
                "Phonology",
                "Q",
                "A",
                "B",
                "C",
                "D",
                "B",
                "E",
                "assignment",
                "beginner",
            ]);
        }
        let err = parse_docx(&docx(&docx_table(&rows))).expect_err("over the cap");
        assert!(err.public_message().contains(&MAX_BULK_ROWS.to_string()));
    }

    /// A nested table's rows must not be folded into the outer table's, which
    /// would shift every column.
    #[test]
    fn a_nested_docx_table_does_not_corrupt_the_outer_rows() {
        let course = Uuid::new_v4().to_string();
        let inner = docx_table(&[vec!["ignored", "also ignored"]]);
        let header = docx_row(&docx_header_cells());
        let data = docx_row(&[
            &course,
            "",
            "Phonology",
            "Q",
            "A",
            "B",
            "C",
            "D",
            "B",
            "E",
            "assignment",
            "beginner",
        ]);
        // The nested table sits inside the first data row's last cell.
        let table = format!("<w:tbl>{header}{data}<w:tr><w:tc>{inner}</w:tc></w:tr></w:tbl>");

        let parsed = parse_docx(&docx(&table)).expect("parses the outer table");
        assert_eq!(parsed.len(), 1, "only the real question row is a row");
        assert_eq!(parsed[0].correct_option_index, 1);
    }

    #[test]
    fn a_docx_without_a_document_part_is_refused() {
        let body = build_zip(&[("word/styles.xml", b"<x/>".to_vec())]);
        assert!(parse_docx(&body).is_err());
    }

    #[test]
    fn malformed_docx_xml_is_a_validation_error_not_a_panic() {
        let body = build_zip(&[("word/document.xml", b"<w:document><w:tbl><w:tr".to_vec())]);
        let err = parse_docx(&body).expect_err("malformed");
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    // -- Dispatch ---------------------------------------------------------

    #[test]
    fn dispatch_reports_the_channel_it_sniffed() {
        let body = serde_json::to_vec(&vec![json_row(Uuid::new_v4(), 1)]).expect("serialises");
        let (format, rows) = parse_import(&body).expect("parses");
        assert_eq!(format, ImportFormat::Json);
        assert_eq!(rows.len(), 1);

        let csv = format!("{CSV_HEADER}\n{}\n", csv_row(Uuid::new_v4(), "", "B"));
        let (format, rows) = parse_import(csv.as_bytes()).expect("parses");
        assert_eq!(format, ImportFormat::Csv);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn a_non_utf8_csv_is_a_validation_error() {
        // A lone 0xFF after an ASCII first byte: sniffed as CSV, then rejected
        // as not UTF-8 rather than lossily decoded.
        let body = vec![b'c', b'o', b'u', b'r', b's', b'e', 0xFF];
        let err = parse_import(&body).expect_err("not UTF-8");
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }
}
