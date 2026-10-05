# Meal Planner — Progress Tracker

Tracks build status against the phases in [meal-planner-spec.md](meal-planner-spec.md). The spec is the source of truth for *what* we're building; this file records *where we are*. Update it at the end of every phase (and when decisions change), then commit.

**Current phase:** 3 — Summary logic (not started)
**Last updated:** 2026-10-05

---

## Phases

Status key: `[ ]` not started · `[~]` in progress · `[x]` done

- [x] **1. Scaffold** — Tauri 2 + React/TS + SQLite, migrations, seed data, two-page navigation shell
- [x] **2. Ingredients + items CRUD** — Library page Items and Ingredients tabs, all three types, per-serve labelling for meats
- [ ] **3. Summary logic** — Rust `summary()` with unit tests, before any UI depends on it
- [ ] **4. Planner calendar** — scrollable weeks (past + future), lazy loading, Today button, show entries
- [ ] **5. Drag and drop** — library → day, move between days, remove, serves stepper for meats
- [ ] **6. Notes + history** — entry notes in chip popover, 📝 indicator, latest note and "last had" on cards, history in item editor, edit-item drawer from planner
- [ ] **7. Summary UI** — range picker, grouping, contributing items, have-it ticks, copy as text
- [ ] **8. Templates (meals only)** — Templates tab + "start from template" toggle in meal editor
- [ ] **9. Polish** — search, colours, settings, copy last week, JSON backup/restore
- [ ] **10. Android (stretch)** — Tauri Android build, slide-up library drawer, long-press dnd-kit activation

---

## Decisions

Settled in the spec (see spec §10) plus choices made while setting up:

- Stack mirrors the budget tracker (https://github.com/DudeUnleashed/budget-tracker): Tauri 2, React 19, TypeScript 5.8, Vite 7, Tailwind CSS 4 (`@tailwindcss/vite`).
- Rust: `rusqlite` 0.32 (`bundled`) + `rusqlite_migration` 1, `serde`/`serde_json`, `chrono`; `tauri-plugin-dialog` for backup file pickers.
- Frontend adds `@dnd-kit/core` and `date-fns`.
- Dates stored as ISO `YYYY-MM-DD` strings.
- Git repo initialised in this folder; commit after each phase.

## Notes / open questions

- Budget tracker source layout wasn't visible (only manifests), so conventions are our own: Rust module per command group; `src/pages`, `src/components`, and a typed `invoke` wrapper on the frontend.

## Log

| Date | Entry |
|---|---|
| 2026-10-05 | Spec read, stack confirmed against budget tracker, git repo initialised, this tracker created. |
| 2026-10-05 | **Phase 1 done.** Tauri 2 + React 19/TS 5.8/Vite 7/Tailwind 4 scaffold; Vite/TS/plugin-react pinned to match budget tracker. SQLite schema (`001_init.sql`) + seed (`002_seed.sql`: 7 categories, 34 ingredients, 8 items, 2 templates, settings) applied via `rusqlite_migration` on startup; DB lives in the app data dir (`%APPDATA%\com.mealplanner.app\meal_planner.db`). Planner/Library navigation shell; `categories_list` command used as a backend smoke test. `cargo test` (migration + seed counts) and `npm run build` pass; app launches and creates its DB. |
| 2026-10-05 | **Phase 2 done.** Rust commands `ingredients_{list,create,update,archive}` and `items_{list,get,create,update,duplicate,archive}` (core logic in plain fns taking a `Connection`; 10 unit tests pass). Library page: **Items** tab (type filter, search, show archived, recipe-style editor with ingredient lines, per-serve labelling for meat, inline ingredient creation, reorder, duplicate, archive/restore, unsaved-changes guard) and **Ingredients** tab (create/edit/archive, used-in count). Templates and Settings tabs still placeholders. Not yet clicked through by hand — worth a manual pass in `npm run tauri dev`. |
