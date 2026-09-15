/**
 * YouTube link handling for a unit's introductory video.
 *
 * A **mirror** of `crates/core/src/video.rs`, kept deliberately: the gateway is
 * the enforcement (it is what normalises and stores the link), and this exists
 * so an admin is told a link is wrong while they are still looking at the field
 * rather than after a round trip. If the two ever disagree, the server wins —
 * so keep this the more permissive of the pair, never the stricter one.
 *
 * Students never call this: `/student/blocks/{id}/units` already hands them the
 * eleven-character `video_id`, precisely so no client has to re-implement the
 * several shapes a YouTube link comes in.
 */

const ID_LEN = 11;
const ID_PATTERN = /^[A-Za-z0-9_-]{11}$/;

/** The video id from any accepted link form, or `null`. */
export function parseYouTubeId(input: string): string | null {
  const raw = input.trim();
  if (raw === "") return null;
  // A bare id, which is what a copy from a spreadsheet column usually is.
  // Checked first so it cannot be mistaken for a hostname.
  if (ID_PATTERN.test(raw)) return raw;

  let url: URL;
  try {
    url = new URL(/^https?:\/\//i.test(raw) ? raw : `https://${raw}`);
  } catch {
    return null;
  }

  const host = url.hostname.toLowerCase().replace(/^(www\.|m\.|music\.)/, "");
  const segments = url.pathname.split("/").filter((s) => s !== "");

  if (host === "youtu.be") return candidate(segments[0]);
  if (host !== "youtube.com" && host !== "youtube-nocookie.com") return null;

  if (segments[0] === "watch") return candidate(url.searchParams.get("v"));
  if (["embed", "shorts", "live", "v"].includes(segments[0] ?? "")) {
    return candidate(segments[1]);
  }
  return candidate(segments[0]);
}

/** The canonical watch URL, matching what the gateway stores. */
export function canonicalYouTubeUrl(input: string): string | null {
  const id = parseYouTubeId(input);
  return id === null ? null : `https://www.youtube.com/watch?v=${id}`;
}

/**
 * A segment is an id only if it is exactly the right shape. A truncated or
 * over-long one is a link we do not understand, and guessing would put a broken
 * embed in front of a student.
 */
function candidate(segment: string | null | undefined): string | null {
  if (!segment) return null;
  const cut = segment.split(/[?&#]/)[0] ?? "";
  return cut.length === ID_LEN && ID_PATTERN.test(cut) ? cut : null;
}
