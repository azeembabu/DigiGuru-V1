# Pedagogy & Conversational Behavior Rules

Covers conversational presence, adaptive difficulty, and note export — the parts of the tutoring
experience not already governed by NN-1 (whiteboard-first) or NN-2 (first-login) in
`IMPLEMENTATION_PLAN.md`. Citation-before-explanation and the paragraph-level comprehension gate
are already specified there (F-34, F-35) — do not duplicate them here.

## Turn-taking

- **No interruption.** The gateway VAD (`realtime-audio.md`) must not open a turn until end-of-speech
  is detected with its 300 ms hangover. The model never begins narrating before that boundary.
- **Backchannel cues.** While the student's turn is open, the client may play short local audio cues
  ("Mhm", "I see") on natural pauses. These are pre-recorded client-side assets triggered by VAD
  pause detection — **not** LLM-generated and **not** routed through Gemini Live, so they add zero
  round-trip latency and cannot themselves violate turn-taking.
- **Zero-lag response start.** Once end-of-speech fires, the existing latency budget
  (`realtime-audio.md`, p95 < 1.5 s) is the contract — there is no additional artificial delay.

## Adaptive difficulty

- Every block starts a new student at the beginner explanation tier (tier 0 of the block's content).
- A comprehension-gate failure (F-35: student answer is incorrect or a confusion intent is
  detected — "I don't understand", "വീണ്ടും പറയാമോ") drops the tutor back one tier for that
  paragraph before it will re-attempt advancing `para_index`.
- Tier state (`current_tier`) is per `(student_id, block_id, para_index)`, held in `sess:{session_id}`
  alongside the FSM state, not persisted long-term — each new attempt at a paragraph starts from the
  student's last successful tier for it, not always from beginner.

## Tone

- Default persona: light humor, analogies, informal banter, within the curriculum-boundary and
  guardrail rules in `security.md` — humor never substitutes for or hallucinates content.
- A detected serious-tone request ("be serious", "no jokes", "ഗൗരവമായി പറയൂ") is a Tier-0 intent
  match (same local matcher as the polite-phrase shortcut) and flips a per-session flag
  `tone: serious` for the remainder of the session. No LLM round trip needed to switch it.
- The system prompt's tone instruction is templated from this flag; it is never silently reset by
  block or topic changes.

## Whiteboard trigger on confusion

- A detected confusion intent ("I don't understand", "notes please") forces a `board_ops` emission
  on the *next* turn even if the tutor's own plan did not call for one — same SyncGate enforcement
  (NN-1) applies, so the visual still lands before the re-explanation audio.

## Note export & retention

- The client renders **Copy** and **Download (PDF/PNG)** controls against the current board
  op-log (`whiteboard-sync.md`) — export is a client-side render of the JSON ops, not a new
  server artifact, so there is nothing extra to keep in sync.
- A retention selector lets the student tag an exported note with a future revision date. This
  writes a row to a `note_reminders` table (`student_id`, `board_events` range, `remind_at`) — a
  simple reminder ledger, not a new content store. A daily cron surfaces due reminders on next login.
