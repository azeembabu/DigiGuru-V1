// Turning a placement failure into something an admin can act on.
//
// `PATCH /students/{id}/academic` and `.../current-block` have named outcomes
// that are not "something went wrong": a live 20-minute session blocks the
// change (`409 SESSION_ACTIVE`), and each of the four ids can be rejected on
// its own (`422 INVALID_PROGRAM|INVALID_SEMESTER|INVALID_LSC|INVALID_BLOCK`).
// A raw code must never reach the screen, so every one of them is translated
// here and the 422s are also routed to the field that caused them.

import { ApiError } from "@/lib/api";

export type PlacementFailure = {
  /** Banner copy. Always human, never a code. */
  message: string;
  /** Per-input messages, keyed by form field name. */
  fieldErrors: Record<string, string>;
  /** The student is mid-session; the form should stay disabled until it ends. */
  sessionActive: boolean;
};

const UNPROCESSABLE: Record<string, { field: string; message: string }> = {
  INVALID_PROGRAM: {
    field: "program_id",
    message: "That program no longer exists or is not active.",
  },
  INVALID_SEMESTER: {
    field: "semester_id",
    message: "That semester does not belong to the selected program.",
  },
  INVALID_LSC: {
    field: "lsc_id",
    message: "That learner support centre no longer exists.",
  },
  INVALID_BLOCK: {
    field: "block_id",
    message: "That block no longer exists.",
  },
};

const SESSION_ACTIVE_MESSAGE =
  "This student is in a live session. Their placement can't change until it ends.";

export function describeFailure(caught: unknown): PlacementFailure {
  if (!(caught instanceof ApiError)) {
    return { message: "Could not save the change. Please try again.", fieldErrors: {}, sessionActive: false };
  }

  if (caught.code === "SESSION_ACTIVE") {
    return { message: SESSION_ACTIVE_MESSAGE, fieldErrors: {}, sessionActive: true };
  }

  const unprocessable = UNPROCESSABLE[caught.code];
  if (unprocessable) {
    return {
      message: unprocessable.message,
      fieldErrors: { [unprocessable.field]: unprocessable.message },
      sessionActive: false,
    };
  }

  // Everything else keeps the gateway's own public message, which is already
  // display-safe by construction (`crates/core/src/error.rs`).
  return { message: caught.message, fieldErrors: caught.fieldErrors(), sessionActive: false };
}
