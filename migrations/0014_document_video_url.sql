-- A unit may have an introductory video, shown before the tutor session.
--
-- It lives on `documents` (the "unit" the student actually opens) rather than
-- on `blocks`: the video introduces one unit's material, and a block holds
-- several. NULL is the normal shape and means "no video" — the classroom then
-- routes straight to the interactive discussion, so every unit uploaded before
-- this migration keeps behaving exactly as it did.
--
-- Stored as the canonical watch URL the gateway normalises to; the 11-character
-- id the player needs is derived from it on the way out, so a malformed link
-- can never reach a client as a broken embed.
ALTER TABLE documents ADD COLUMN video_url TEXT;

COMMENT ON COLUMN documents.video_url IS
  'Optional YouTube watch URL shown before the tutoring session. NULL = no video.';
