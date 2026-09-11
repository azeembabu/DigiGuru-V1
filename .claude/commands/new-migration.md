---
description: Create a reviewed, forward-only SQL migration
---

Create a migration for: `$ARGUMENTS`

1. Read the current schema in `IMPLEMENTATION_PLAN.md` §3.1 and the existing files in `migrations/`.
2. `sqlx migrate add -r <slug>` to generate the up/down pair.
3. Write the SQL:
   - Forward-only in spirit: never edit an applied migration, even to fix a typo.
   - Additive where possible. A destructive change needs a two-step plan (add, backfill, switch,
     drop later) written into the PR description.
   - Index every new foreign key and every column used as a filter.
   - `NOT NULL` columns on an existing table need a default or a backfill step.
4. Apply locally with `sqlx migrate run`, then `cargo check` so the compile-time query checks catch
   any model that no longer matches.
5. Update the schema section of `IMPLEMENTATION_PLAN.md` if the change alters the documented model.

Report the migration filename, what it changes, and whether any backfill is required.
