"use client";

import { useState } from "react";

import { AdminField, AdminInput } from "@/components/admin/controls";
import { FormModal } from "@/components/admin/academic/FormModal";
import { fieldErrorsFor, messageFor } from "@/components/admin/academic/shared";
import { setDocumentVideo } from "@/lib/admin/client";
import { parseYouTubeId } from "@/lib/youtube";
import type { AdminDocument } from "@/lib/admin/types";

/**
 * Set or clear the YouTube video a student sees before this unit's discussion.
 *
 * The link decides the student's route: with one saved, opening the unit plays
 * the video first and then hands them to the tutor; with the field left empty
 * the classroom opens straight onto the interactive discussion. That is said on
 * the form, because it is the only place an admin can see the consequence of
 * what they are typing.
 *
 * Validation here is a courtesy so a bad paste is caught while the field is
 * still on screen. The gateway revalidates, normalises the link to a canonical
 * watch URL and remains the enforcement (`crates/core/src/video.rs`).
 */
export function UnitVideoForm({
  unit,
  onClose,
  onSaved,
}: {
  unit: AdminDocument;
  onClose: () => void;
  onSaved: (message: string) => void;
}) {
  const [value, setValue] = useState(unit.video_url ?? "");
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  const trimmed = value.trim();
  const videoId = trimmed === "" ? null : parseYouTubeId(trimmed);

  async function submit() {
    if (trimmed !== "" && videoId === null) {
      setErrors({
        video_url: "That is not a link to a single YouTube video.",
      });
      return;
    }
    setErrors({});
    setPending(true);
    setFormError(null);
    try {
      // An emptied field clears the video, which is the same state a unit that
      // never had one is in — students then go straight to the discussion.
      await setDocumentVideo(unit.id, trimmed === "" ? null : trimmed);
      onSaved(
        trimmed === ""
          ? `Video removed from “${unit.title}”. Students now go straight to the discussion.`
          : `Video saved for “${unit.title}”.`,
      );
    } catch (caught: unknown) {
      setErrors(fieldErrorsFor(caught));
      setFormError(messageFor(caught, "Could not save the video link."));
    } finally {
      setPending(false);
    }
  }

  return (
    <FormModal
      open
      title="Unit video"
      description="Shown before the tutoring session. Leave it empty and students open the discussion directly."
      submitLabel={trimmed === "" && unit.video_url ? "Remove video" : "Save video"}
      pending={pending}
      error={formError}
      onSubmit={submit}
      onClose={onClose}
    >
      <AdminField
        id="unit-video-url"
        label="YouTube link"
        error={errors.video_url}
        hint="A watch, youtu.be, Shorts or embed link — anything but a playlist. Empty means no video."
      >
        <AdminInput
          id="unit-video-url"
          type="url"
          inputMode="url"
          placeholder="https://www.youtube.com/watch?v=…"
          value={value}
          onChange={(event) => setValue(event.target.value)}
          hasError={Boolean(errors.video_url)}
          hasHint
        />
      </AdminField>

      {/*
        The thumbnail is the check that matters: an admin pasting the wrong row
        of a spreadsheet gets a valid link to the wrong video, which no amount
        of parsing can catch and one glance can.
      */}
      {videoId ? (
        <div className="flex items-center gap-3 rounded-lg border border-gray-200 bg-gray-50 p-3">
          {/* eslint-disable-next-line @next/next/no-img-element -- a YouTube
              thumbnail is a remote URL on a host we do not configure in
              `next.config.ts`; optimising it would mean proxying every
              preview through the app for no benefit. */}
          <img
            src={`https://i.ytimg.com/vi/${videoId}/mqdefault.jpg`}
            alt=""
            width={120}
            height={68}
            className="h-[68px] w-[120px] shrink-0 rounded object-cover"
          />
          <div className="min-w-0 text-sm">
            <p className="font-medium text-gray-900">This is what students will see.</p>
            <p className="mt-0.5 truncate text-gray-500">Video id {videoId}</p>
            <a
              href={`https://www.youtube.com/watch?v=${videoId}`}
              target="_blank"
              rel="noreferrer"
              className="mt-0.5 inline-block text-indigo-600 hover:underline"
            >
              Open on YouTube
            </a>
          </div>
        </div>
      ) : null}
    </FormModal>
  );
}
