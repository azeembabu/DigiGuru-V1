"use client";

/**
 * Error boundary around the canvas.
 *
 * `.claude/rules/whiteboard-sync.md`: *"A canvas exception must never kill the
 * voice stream. The renderer is wrapped in an error boundary that reports
 * `board_error` and degrades to text-only."*
 *
 * Two distinct failure scopes, and it matters which this is:
 *
 * * a single op failing to render is handled inside `BoardRenderer` and, for a
 *   whole turn, reported as `board_error` by `SessionClient` — the lesson keeps
 *   going and the gateway releases that turn's audio immediately,
 * * a React render crash in the canvas subtree is *this* boundary's job. By then
 *   the component tree is unusable, so the honest move is to say the board is
 *   gone and offer a reload, rather than pretend a dead canvas is still syncing.
 *
 * Deliberately a class component: `componentDidCatch` has no hook equivalent.
 */

import { Component } from "react";
import type { ErrorInfo, ReactNode } from "react";

interface Props {
  children: ReactNode;
  /** Reports upward so a parent can tell the gateway the board is unusable. */
  onCrash?: (error: Error) => void;
}

interface State {
  error: Error | null;
}

export class BoardErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    // Logged, never shown: a component stack is exactly the internal detail the
    // error-exposure rule keeps away from users.
    console.error("classroom board crashed", error, info.componentStack);
    this.props.onCrash?.(error);
  }

  render(): ReactNode {
    if (this.state.error) {
      return (
        <div className="flex min-h-dvh flex-col items-center justify-center gap-4 bg-ink-950 px-5 text-center text-lavender-50">
          <h1 className="font-display text-[24px] font-bold text-white">
            The whiteboard stopped working
          </h1>
          <p className="max-w-md text-[15px] leading-relaxed text-gray-300">
            The lesson was interrupted because the board could not be drawn. Reloading starts a fresh
            session — your program and block are saved, so nothing is lost.
          </p>
          <button
            type="button"
            onClick={() => window.location.reload()}
            className="rounded-lg bg-indigo-500 px-4 py-2 text-[14px] font-semibold text-white hover:bg-indigo-400"
          >
            Reload the classroom
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
