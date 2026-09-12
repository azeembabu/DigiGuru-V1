-- Blocks admin CRUD (`/api/v1/admin/blocks`) needs the same two editable
-- fields every other academic entity already has: a free-text `description`
-- (like `programs.description` / `courses.description`) and an
-- `entity_status` lifecycle flag (like `programs.status` /
-- `semesters.status` / `lscs.status`). `blocks` was created in
-- `0001_init.sql` with neither, because nothing could create a block through
-- the API at all.
--
-- Forward-only: both columns are nullable-or-defaulted, so existing rows and
-- every existing query (`SELECT id, course_id, block_no, title ...`) keep
-- working unchanged.

ALTER TABLE blocks ADD COLUMN description TEXT;
ALTER TABLE blocks ADD COLUMN status entity_status NOT NULL DEFAULT 'active';
