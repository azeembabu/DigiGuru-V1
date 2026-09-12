"use client";

import { useState } from "react";

import { AdminField, AdminInput, AdminSelect, AdminTextarea } from "@/components/admin/controls";
import { FormModal, StatusOptions } from "@/components/admin/academic/FormModal";
import {
  fieldErrorsFor,
  lengthRule,
  messageFor,
  optionalText,
  rangeRule,
} from "@/components/admin/academic/shared";
import { createBlock, updateBlock } from "@/lib/admin/client";
import type { Block, EntityStatus } from "@/lib/admin/types";

// Mirrors `POST /admin/blocks` in `.claude/rules/api-conventions.md`
// (`block_no` 1..999, `title` 2-200). A courtesy check only — the gateway
// revalidates and remains the enforcement.
const TITLE_MIN = 2;
const TITLE_MAX = 200;
const BLOCK_NO_MAX = 999;

/** Create/edit a block of one course. `block_no` is create-only. */
export function BlockForm({
  courseId,
  block,
  onClose,
  onSaved,
}: {
  courseId: string;
  block: Block | null;
  onClose: () => void;
  onSaved: (message: string) => void;
}) {
  const [blockNo, setBlockNo] = useState(block ? String(block.block_no) : "1");
  const [title, setTitle] = useState(block?.title ?? "");
  const [description, setDescription] = useState(block?.description ?? "");
  const [status, setStatus] = useState<EntityStatus>(block?.status ?? "active");
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  function validate(): Record<string, string> {
    const next: Record<string, string> = {};
    if (!block) {
      const noError = rangeRule(blockNo, 1, BLOCK_NO_MAX, "Block number");
      if (noError) next.block_no = noError;
    }
    const titleError = lengthRule(title, TITLE_MIN, TITLE_MAX, "Title");
    if (titleError) next.title = titleError;
    return next;
  }

  async function submit() {
    const found = validate();
    setErrors(found);
    if (Object.keys(found).length > 0) return;

    setPending(true);
    setFormError(null);
    try {
      if (block) {
        await updateBlock(block.id, {
          title: title.trim(),
          description: optionalText(description),
          status,
        });
        onSaved(`Block “${title.trim()}” updated.`);
      } else {
        await createBlock({
          course_id: courseId,
          block_no: Number(blockNo),
          title: title.trim(),
          description: optionalText(description),
        });
        onSaved(`Block “${title.trim()}” created.`);
      }
    } catch (caught: unknown) {
      setErrors(fieldErrorsFor(caught));
      setFormError(messageFor(caught, "Could not save the block."));
    } finally {
      setPending(false);
    }
  }

  return (
    <FormModal
      open
      title={block ? "Edit block" : "New block"}
      description={
        block
          ? "The block number is fixed once the block exists."
          : "A block is the unit a student is taught from, and the unit a textbook is uploaded against."
      }
      submitLabel={block ? "Save changes" : "Create block"}
      pending={pending}
      error={formError}
      onSubmit={submit}
      onClose={onClose}
    >
      {block ? null : (
        <AdminField
          id="block-no"
          label="Block number"
          required
          error={errors.block_no}
          hint="Its order within the course."
        >
          <AdminInput
            id="block-no"
            type="number"
            min={1}
            max={BLOCK_NO_MAX}
            value={blockNo}
            onChange={(event) => setBlockNo(event.target.value)}
            hasError={Boolean(errors.block_no)}
            hasHint
          />
        </AdminField>
      )}

      <AdminField id="block-title" label="Title" required error={errors.title}>
        <AdminInput
          id="block-title"
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          hasError={Boolean(errors.title)}
          autoComplete="off"
          placeholder="Block 1 — Early poetry"
        />
      </AdminField>

      <AdminField id="block-description" label="Description" error={errors.description}>
        <AdminTextarea
          id="block-description"
          value={description}
          onChange={(event) => setDescription(event.target.value)}
          hasError={Boolean(errors.description)}
          placeholder="Optional description…"
        />
      </AdminField>

      {block ? (
        <AdminField id="block-status" label="Status" required error={errors.status}>
          <AdminSelect
            id="block-status"
            value={status}
            onChange={(event) =>
              setStatus(event.target.value === "inactive" ? "inactive" : "active")
            }
            hasError={Boolean(errors.status)}
          >
            <StatusOptions />
          </AdminSelect>
        </AdminField>
      ) : null}
    </FormModal>
  );
}
