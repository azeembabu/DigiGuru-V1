//! YouTube link parsing for a unit's introductory video.
//!
//! Pure string logic, deliberately in `core` rather than in the gateway: the
//! admin write path validates with it and the student read path derives the
//! embeddable id with it, and the two must not be able to disagree about what
//! a link means.
//!
//! # Only the id is ever trusted
//!
//! What an admin pastes is whatever YouTube's share button gave them — a watch
//! URL, a `youtu.be` short link, an embed URL, a Shorts link, any of them with
//! tracking parameters attached. All that actually matters downstream is the
//! eleven-character video id, so a link is reduced to that id at the boundary
//! and stored as the one canonical watch URL built back from it. A link that
//! yields no id is rejected at write time rather than stored and discovered to
//! be broken by a student, and nothing else in the original string (a playlist,
//! a `t=` offset, a UTM tag) survives into the player.

/// A YouTube video id is exactly eleven characters of this alphabet.
const ID_LEN: usize = 11;

/// The id extracted from any accepted YouTube link form, or `None`.
///
/// Accepts, with or without a scheme, `www.`/`m.`/`music.` and query noise:
/// `youtube.com/watch?v=ID`, `youtu.be/ID`, `youtube.com/embed/ID`,
/// `youtube.com/shorts/ID`, `youtube.com/live/ID`, and a bare `ID`.
pub fn youtube_id(input: &str) -> Option<String> {
    let raw = input.trim();
    if raw.is_empty() {
        return None;
    }

    // A bare id, which is what an admin copying from a spreadsheet column
    // often has. Checked first so it is never mistaken for a hostname.
    if is_id(raw) {
        return Some(raw.to_owned());
    }

    let without_scheme = raw
        .strip_prefix("https://")
        .or_else(|| raw.strip_prefix("http://"))
        .or_else(|| raw.strip_prefix("//"))
        .unwrap_or(raw);

    let (authority, path_and_query) = match without_scheme.find('/') {
        Some(at) => (&without_scheme[..at], &without_scheme[at + 1..]),
        None => (without_scheme, ""),
    };
    let host = authority
        .split('@')
        .next_back()?
        .split(':')
        .next()?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let host = host
        .strip_prefix("www.")
        .or_else(|| host.strip_prefix("m."))
        .or_else(|| host.strip_prefix("music."))
        .unwrap_or(&host)
        .to_owned();

    let (path, query) = match path_and_query.find('?') {
        Some(at) => (&path_and_query[..at], &path_and_query[at + 1..]),
        None => (path_and_query, ""),
    };
    let first = path.split('/').next().unwrap_or("");

    match host.as_str() {
        // The short form puts the id in the first path segment.
        "youtu.be" => candidate(first),
        "youtube.com" | "youtube-nocookie.com" => match first {
            "watch" => query_param(query, "v").and_then(|v| candidate(&v)),
            "embed" | "shorts" | "live" | "v" => {
                candidate(path.split('/').nth(1).unwrap_or(""))
            }
            // `/watch/ID` and `/ID` both appear in the wild.
            _ => candidate(first),
        },
        _ => None,
    }
}

/// The canonical watch URL for an accepted link, or `None` if it is not one.
///
/// This is what is stored, so the database never holds a playlist link, a
/// timestamped link, or somebody's tracking parameters — and a later change of
/// player has one shape to read rather than every shape YouTube has shipped.
pub fn canonical_youtube_url(input: &str) -> Option<String> {
    youtube_id(input).map(|id| format!("https://www.youtube.com/watch?v={id}"))
}

/// Pull one parameter out of a query string without a URL crate: the values
/// here are ids from a copy-pasted link, never percent-encoded payloads.
fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| v.to_owned())
    })
}

/// A path segment is an id only if it is exactly the right shape — a truncated
/// or over-long segment is a link we do not understand, and guessing at one
/// would put a broken embed in front of a student.
fn candidate(segment: &str) -> Option<String> {
    let cut = segment
        .split(['?', '&', '#'])
        .next()
        .unwrap_or(segment)
        .trim_end_matches('/');
    is_id(cut).then(|| cut.to_owned())
}

fn is_id(s: &str) -> bool {
    s.len() == ID_LEN
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_every_link_form_youtube_hands_an_admin() {
        for link in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "http://youtube.com/watch?v=dQw4w9WgXcQ",
            "https://m.youtube.com/watch?v=dQw4w9WgXcQ&t=42s",
            "https://www.youtube.com/watch?list=PL123&v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ?si=trackingjunk",
            "https://www.youtube.com/embed/dQw4w9WgXcQ",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://www.youtube.com/live/dQw4w9WgXcQ",
            "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ",
            "  dQw4w9WgXcQ  ",
        ] {
            assert_eq!(
                youtube_id(link).as_deref(),
                Some("dQw4w9WgXcQ"),
                "failed on {link}"
            );
        }
    }

    #[test]
    fn rejects_a_link_that_is_not_a_single_video() {
        for link in [
            "",
            "   ",
            "https://www.youtube.com/playlist?list=PL1234567890",
            "https://www.youtube.com/@somechannel",
            "https://vimeo.com/123456789",
            "https://example.com/watch?v=dQw4w9WgXcQ",
            "https://www.youtube.com/watch?v=tooshort",
            "https://www.youtube.com/watch?v=waaaaaaaaaaytoolong",
            "not a url at all",
        ] {
            assert_eq!(youtube_id(link), None, "accepted {link}");
        }
    }

    #[test]
    fn stores_one_canonical_shape_whatever_was_pasted() {
        let expected = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
        assert_eq!(
            canonical_youtube_url("https://youtu.be/dQw4w9WgXcQ?si=abc").as_deref(),
            Some(expected)
        );
        assert_eq!(
            canonical_youtube_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=90")
                .as_deref(),
            Some(expected),
            "a timestamp must not survive into the stored link"
        );
    }
}
