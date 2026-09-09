# Digi Guru — Project Rules

## Git Sync Rule (MANDATORY)

**Always pull before starting work, and push after every completed change.**

This project has three parts in one repo — they must always stay in sync:

```text
backend/       Node + Express + Prisma API
student-app/   Student portal (React + Vite)
admin-app/     Admin portal (React + Vite)
```

### Before starting any new task / session

```bash
git pull origin main
```

All three parts are pulled together — never work on a stale copy.

### After completing any change

```bash
git add -A
git commit -m "<type>: <description>"
git push origin main
```

### Rules

1. **Pull first** — no edits before `git pull` succeeds (avoids merge conflicts).
2. **Push after every finished unit of work** — don't accumulate local-only changes; a finished feature/fix must reach GitHub the same session.
3. **All parts together** — backend, student-app, and admin-app live in one repo; commit and push them together so refs across apps stay consistent.
4. **Never force-push** to `main`.
5. **Never commit** `.env`, secrets, or `node_modules` (gitignore covers this — verify with `git status` before committing).
6. If a pull brings conflicts, resolve them before continuing work — never push over or ignore conflicts.
