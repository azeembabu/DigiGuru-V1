//! Strip page furniture — headers, footers, page numbers, watermarks —
//! before chunking (`IMPLEMENTATION_PLAN.md` §5.1, E-29).
//!
//! Approach: a line that repeats verbatim (or near-verbatim, allowing a
//! trailing page number) across a majority of pages is a header/footer and
//! is stripped from every page it appears on. A line that is *purely* a
//! page number (all digits, optionally with surrounding punctuation like
//! `- 12 -`) is stripped unconditionally, since it carries no content.

/// Page text as produced by `parse` — a plain per-page string, not the
/// `parse::ParsedPage` type, so this module has no dependency on `parse`
/// and stays a pure, independently-testable function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageText {
    pub page_number: i32,
    pub text: String,
}

/// A repeated line is considered header/footer furniture once it appears on
/// at least this fraction of pages (rounded down, minimum 2 pages so a
/// single-page document never strips anything as "repeated").
const REPETITION_THRESHOLD: f64 = 0.5;

pub fn clean_pages(pages: Vec<PageText>) -> Vec<PageText> {
    if pages.is_empty() {
        return pages;
    }

    let total_pages = pages.len();
    let mut normalized_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    let line_sets: Vec<Vec<(&str, String)>> = pages
        .iter()
        .map(|p| {
            p.text
                .lines()
                .map(|line| (line, normalize_line(line)))
                .collect()
        })
        .collect();

    for lines in &line_sets {
        // Count each distinct normalized line at most once per page, so a
        // line repeated twice on the same page doesn't inflate the count.
        let mut seen_this_page = std::collections::HashSet::new();
        for (_, normalized) in lines {
            if normalized.is_empty() {
                continue;
            }
            if seen_this_page.insert(normalized.clone()) {
                *normalized_counts.entry(normalized.clone()).or_insert(0) += 1;
            }
        }
    }

    let min_pages_for_repetition = ((total_pages as f64) * REPETITION_THRESHOLD).ceil() as usize;
    let min_pages_for_repetition = min_pages_for_repetition.max(2);

    let furniture: std::collections::HashSet<String> = normalized_counts
        .into_iter()
        .filter(|(_, count)| *count >= min_pages_for_repetition && total_pages >= 2)
        .map(|(line, _)| line)
        .collect();

    pages
        .into_iter()
        .zip(line_sets)
        .map(|(page, lines)| {
            let kept: Vec<&str> = lines
                .iter()
                .filter(|(raw, normalized)| {
                    if is_pure_page_number(raw) {
                        return false;
                    }
                    !furniture.contains(normalized)
                })
                .map(|(raw, _)| *raw)
                .collect();
            PageText {
                page_number: page.page_number,
                text: kept.join("\n"),
            }
        })
        .collect()
}

/// Normalizes a line for repetition comparison: trims whitespace, lowercases,
/// and strips a trailing run of digits (a page number tacked onto an
/// otherwise-identical header/footer, e.g. "Chapter 3  12" vs "Chapter 3  13").
fn normalize_line(line: &str) -> String {
    let trimmed = line.trim().to_lowercase();
    let trimmed_of_trailing_number = trimmed
        .trim_end_matches(|c: char| c.is_ascii_digit() || c.is_whitespace() || c == '-')
        .to_string();
    if trimmed_of_trailing_number.is_empty() {
        // The whole line was just a number/whitespace/dash — don't treat an
        // empty normalized form as a "furniture" candidate; page-number-only
        // lines are handled separately by `is_pure_page_number`.
        String::new()
    } else {
        trimmed_of_trailing_number
    }
}

fn is_pure_page_number(line: &str) -> bool {
    let trimmed = line.trim().trim_matches(|c: char| c == '-' || c == '.' || c.is_whitespace());
    !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_repeated_header_and_footer_and_page_numbers() {
        let pages = vec![
            PageText {
                page_number: 1,
                text: "Chapter 3: Poetry\nThis is the first paragraph of real content.\n- 1 -"
                    .to_string(),
            },
            PageText {
                page_number: 2,
                text: "Chapter 3: Poetry\nMore real content continues here on page two.\n- 2 -"
                    .to_string(),
            },
            PageText {
                page_number: 3,
                text: "Chapter 3: Poetry\nAnd a third page of genuine textbook prose.\n- 3 -"
                    .to_string(),
            },
        ];

        let cleaned = clean_pages(pages);

        for page in &cleaned {
            assert!(
                !page.text.contains("Chapter 3: Poetry"),
                "repeated header should be stripped from page {}",
                page.page_number
            );
            assert!(
                !page.text.contains("- 1 -") && !page.text.contains("- 2 -") && !page.text.contains("- 3 -"),
                "page-number footer should be stripped from page {}",
                page.page_number
            );
        }

        assert!(cleaned[0].text.contains("first paragraph"));
        assert!(cleaned[1].text.contains("page two"));
        assert!(cleaned[2].text.contains("third page"));
    }

    #[test]
    fn leaves_non_repeated_content_untouched() {
        let pages = vec![
            PageText {
                page_number: 1,
                text: "Unique sentence one.".to_string(),
            },
            PageText {
                page_number: 2,
                text: "Unique sentence two.".to_string(),
            },
        ];

        let cleaned = clean_pages(pages.clone());
        assert_eq!(cleaned, pages);
    }
}
