# Meal Planner — Project Spec

A desktop app (Android later) for planning dinners a week or fortnight ahead and getting a single shopping summary of the meat, veg and other key ingredients needed for the shop.

This is a planning tool for the *big stuff*, not a full recipe app. Meals only need the ingredients that matter for the shop — spices, oil, salt etc. can be left out.

---

## 1. Goals and non-goals

**Goals**
- Build a personal library of meals, meats and sides, each with a short list of key ingredients and quantities.
- Create and edit items in a simple recipe-style editor at any time; optionally start a **meal** from a template (e.g. "Risotto" → pick protein + veg).
- Drag items onto days in a calendar of dinners.
- Generate a summary of total quantities per ingredient for any date range, grouped by ingredient category (meat, veg, dairy, pantry…).
- Scroll back through past weeks to see what was made / bought ("what did we have last Tuesday?") and vary meals over a month or two.
- Leave notes on a planned meal ("try a different sauce next time") that show up as a reminder when that meal is used again.

**Non-goals (v1)**
- Breakfast / lunch planning — **dinner only**.
- Leftover tracking — made meals are already sized to include leftovers.
- Persistent pantry / stock tracking — "already have it" ticks reset each shop.
- Full recipes, nutrition, pricing, store integration, online sync, multi-user.

---

## 2. Tech stack

Match the budget tracker so patterns and code can be reused:

- **Tauri 2** (Rust backend) — desktop first; Tauri 2's Android target is the path to mobile later.
- **React + TypeScript** frontend (Vite).
- **SQLite** via `rusqlite` (or `sqlx`) on the Rust side, exposed through Tauri commands.
- **Drag and drop:** `@dnd-kit/core` — supports mouse *and* touch sensors, so it carries over to Android.
- **Dates:** `date-fns`.
- Styling: whatever the budget tracker uses, for consistency.

All data stays local. Add a JSON export/import early (backup + moving between devices).

---

## 3. Core concepts

| Concept | What it is |
|---|---|
| **Ingredient** | A shoppable item in a catalogue: name, ingredient category, unit. e.g. *Beef mince — Meat — g*. |
| **Item** | Anything you can put on the calendar. Has a **type**: `meal`, `meat` or `side`, plus a list of ingredient lines. |
| **Template** | Optional, **meals only**. A meal skeleton with fixed lines and *slots* to fill (e.g. "Protein: 500 g", "Veg: 2 each"). Most items won't use one. |
| **Plan entry** | An item placed on a date. Meat entries also store a number of serves. Can carry a note. |
| **Summary** | Totals of every ingredient across plan entries in a chosen date range. |

> Naming: **item type** (meal / meat / side) is what the item *is* on the calendar. **Ingredient category** (Meat, Veg, Dairy…) is how ingredients are grouped in the shopping summary. Keep these two separate in code to avoid confusion.

### The three item types

| Type | Examples | Quantities | On the calendar |
|---|---|---|---|
| **Meal** | Chow mein, risotto, spag bol | Fixed — the amount you always cook (leftovers already included) | Dropped as-is, no serves |
| **Meat** | Steak, chicken Kievs, sausages | **Per serve**, set once when the meat is created and kept — e.g. 1 serve = 250 g steak, or 2 sausages | Dropped with a serves count (− 3 +) |
| **Side** | Potato salad, chips, veggies, Caesar salad | Fixed — one batch | Dropped as-is, no serves |

Only meats scale, and only by the number of serves — the per-serve amount lives on the item and is changed only by editing the item. Meals and sides never scale — if someone isn't home, there are just more leftovers.

A day can hold **multiple entries**: e.g. *Steak × 3 serves* + *Chips* + *Caesar salad*, or *Chow mein* alone.

Entry quantity per line:
- `meal` / `side`: `line.quantity`
- `meat`: `line.quantity × entry.serves`

---

## 4. Data model (SQLite)

```sql
CREATE TABLE ingredient_category (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL UNIQUE,      -- Meat, Seafood, Veg, Fruit, Dairy, Pantry, Other
  sort_order  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE ingredient (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL UNIQUE,      -- "Beef mince", "Carrot", "Zucchini"
  category_id INTEGER NOT NULL REFERENCES ingredient_category(id),
  unit        TEXT NOT NULL,             -- 'g' | 'ml' | 'each'
  archived    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE item (
  id           INTEGER PRIMARY KEY,
  name         TEXT NOT NULL,            -- "Chicken & mushroom risotto", "Sausages", "Potato salad"
  type         TEXT NOT NULL CHECK (type IN ('meal','meat','side')),
  colour       TEXT,                     -- card colour on calendar
  notes        TEXT,                     -- permanent notes about the item itself
  template_id  INTEGER REFERENCES template(id),  -- where it came from, informational only
  archived     INTEGER NOT NULL DEFAULT 0
);

-- For type = 'meat', quantity is PER SERVE.
CREATE TABLE item_ingredient (
  id            INTEGER PRIMARY KEY,
  item_id       INTEGER NOT NULL REFERENCES item(id) ON DELETE CASCADE,
  ingredient_id INTEGER NOT NULL REFERENCES ingredient(id),
  quantity      REAL NOT NULL,           -- in the ingredient's unit
  position      INTEGER NOT NULL DEFAULT 0
);

-- Templates are for meals only (no type column needed).
CREATE TABLE template (
  id    INTEGER PRIMARY KEY,
  name  TEXT NOT NULL                    -- "Risotto", "Stir fry", "Pasta bake"
);

CREATE TABLE template_line (
  id            INTEGER PRIMARY KEY,
  template_id   INTEGER NOT NULL REFERENCES template(id) ON DELETE CASCADE,
  label         TEXT NOT NULL,           -- "Protein", "Veg 1", "Arborio rice"
  ingredient_id INTEGER REFERENCES ingredient(id),            -- NULL = slot to fill; set = fixed line
  category_id   INTEGER REFERENCES ingredient_category(id),   -- filters the picker for slots (e.g. Meat)
  quantity      REAL,                    -- default quantity
  unit_hint     TEXT,                    -- used when ingredient_id is NULL
  position      INTEGER NOT NULL DEFAULT 0
);

-- Dinner only: one row per item on a day. No meal-slot column.
CREATE TABLE plan_entry (
  id        INTEGER PRIMARY KEY,
  date      TEXT NOT NULL,               -- ISO 'YYYY-MM-DD'
  item_id   INTEGER NOT NULL REFERENCES item(id),
  serves    INTEGER,                     -- required for 'meat', NULL otherwise
  note      TEXT,                        -- "next time try satay sauce / use pork instead"
  position  INTEGER NOT NULL DEFAULT 0   -- ordering within a day
);

CREATE INDEX idx_plan_entry_date ON plan_entry(date);
CREATE INDEX idx_plan_entry_item ON plan_entry(item_id);

CREATE TABLE setting (
  key   TEXT PRIMARY KEY,                -- e.g. 'default_meat_serves', 'week_start'
  value TEXT NOT NULL
);
```

**Design decisions**
- **One unit per ingredient.** Carrots are always `each`, mince is always `g`. No unit conversion; format on output (≥1000 g → kg).
- **Items created from templates are copies.** Editing the Risotto template doesn't change existing risottos.
- **Plan entries link to items live, including past weeks.** Editing an item (e.g. more mince in the spag bol) updates every plan it's on, past and future. Past weeks are a record of *which* item was had on which day, not an exact record of quantities bought.
- **Items are always editable.** No locking, no versioning — change quantities, add/remove ingredients, rename, at any time.
- **Archive, don't delete,** items and ingredients used in plans, so past weeks still render.
- **"Have it" ticks are not stored.** They live in React state for the current summary and reset when it's closed or the range changes.

---

## 5. Screens

### Page 1 — Planner

```
┌───────────────────────────────────────────────┬───────────────────────┐
│ [Today]                          [Summary ▸]  │ 🔍 Search             │
│                                               │ [Meal | Meat | Side]  │
│ ── Week of 29 Sep (past, dimmed) ───────────  │───────────────────────│
│ Mon   Tue    Wed    Thu   Fri    Sat   Sun    │ ▢ Chow mein           │
│ Bol   Steak×3 Chow  …     Saus×2 …     …      │ ▢ Risotto – chicken   │
│       Chips   mein📝      Veg                 │   📝 try lemon & peas │
│ ── Week of 6 Oct (this week) ───────────────  │ ▢ Spag bol            │
│ Risot Kiev×3  …     …     …      …     …      │                       │
│ to    Caesar                                  │                       │
│ ── Week of 13 Oct ──────────────────────────  │                       │
│ …                                             │                       │
└───────────────────────────────────────────────┴───────────────────────┘
```

**Left: calendar (switchable view)**
- Dinners only, 7 day columns. A view switcher in the toolbar picks how much you see; the choice is remembered (`planner_view` setting):
  - **1 week** — one big week, for focused planning of the week ahead.
  - **2 weeks** (default) — a fortnight block, the usual planning window.
  - **Month** — the whole calendar month at a glance, for varying meals over a month or two; days outside the month are muted.
  - **Scroll** — continuous list of weeks, scrolling in both directions; weeks load lazily (±8 at a time).
- 1 week / 2 weeks / Month show a fixed block with **‹ ›** to step by a block (1 week, 2 weeks, 1 month) and a title for the range shown. Switching view keeps you in the same place.
- Opens on the current week; **Today** jumps back.
- Past days are visually dimmed but still editable (so you can record what you actually ended up having).
- A small 📝 icon on any entry that has a note.

**Right: item library**
- Tabs / filter: **Meal**, **Meat**, **Side** (plus search across all).
- Cards show name and colour, plus **"last had 12 days ago"** (or "planned Thu") to help vary meals and avoid repeats.
- Optional sort: **least recently had first**, to surface meals you haven't had in a while.
- If any past plan entry for that item has a note, show the **most recent note** on the card as a reminder.

**Interactions**
- **Drag** a card onto a day → creates a plan entry.
  - Meals and sides drop as-is.
  - Meats drop with the default serves (setting) and show a `− 3 +` stepper on the calendar chip.
- **Drag** a chip between days to move it (including within the same day to reorder).
- **Click** a chip → popover: serves (meats only), **note**, remove, **edit item** (opens the item editor in a side drawer without leaving the planner; changes apply everywhere).
- Optional later: "copy last week to next week".

### Summary (panel on the Planner page)

Opened from the **Summary** button. It **follows the planner view**: with 1 week or 2 weeks showing, the summary covers exactly that block (so you can plan a week or a fortnight and shop for it); in Month view it covers the month; in Scroll view it covers the current week. Quick **1 wk / 2 wk** toggles and ‹ › stepping change the block without leaving the summary, and a custom start date / length is available too. Works for past ranges too, to see what was bought.

```
Shopping summary — 6 Oct to 19 Oct (14 dinners, 22 items)

MEAT
  Beef mince ............ 1.0 kg     (Chow mein, Spag bol)
  Chicken thigh ......... 600 g      (Risotto)
  Steak ................. 750 g      (Steak × 3)
  Sausages .............. 8          (Sausages × 2, × 2)
VEG
  Carrot ................ 7          (Chow mein, Veggies ×2 …)
  Zucchini .............. 3
  Potato ................ 1.5 kg     (Chips, Potato salad)
DAIRY
  Parmesan .............. 80 g
```

- Grouped by ingredient category (sort order), alphabetical within.
- Each row shows the total and expandable list of contributing items/days.
- `each` totals round up to whole items; weights display as g/kg.
- **"Have it" checkboxes** to cross off what's already in the fridge — reset each time.
- **Copy as text** → plain list for pasting into notes / messages (unticked items only).

### Page 2 — Library (create & edit)

Tabs:

1. **Items** — list on the left (filterable by Meal / Meat / Side), editor on the right.
   - **Recipe-style by default:** name, type, colour, item notes, then an ingredient list — like writing out a recipe card, without the method.
   - Ingredient lines: searchable ingredient picker + quantity (unit shown from ingredient). Add / remove / reorder. Every field is editable at any time.
   - For **meat** items, the editor labels quantities as **"per serve"** (e.g. Sausages: 2 each per serve; Steak: 250 g per serve). This is set here and stays until the item is edited.
   - Create a missing ingredient inline (name + category + unit).
   - **"Start from template" toggle** — only shown when type is Meal, **off by default**. When on: pick a template → slots appear with empty pickers (filtered by category, e.g. Protein shows Meat) → fill in → name auto-suggests ("Risotto – chicken & mushroom") → the lines are copied in and become normal editable lines.
   - **Duplicate** button for quick variants.
   - **History:** list of past dates this item was planned, with any notes left on those entries.
2. **Templates** (meals only) — lines can be *fixed* (arborio rice 300 g) or *slots* (Protein — Meat — 500 g). Tucked away as a secondary tab since most items won't use one.
3. **Ingredients** — catalogue table: name, category, unit, used-in count, archive.
4. **Settings** — default meat serves, first day of week.

---

## 6. Summary calculation

Do it in Rust (single source of truth, testable):

```
fn summary(start: Date, end: Date) -> Vec<SummaryRow>
  for each plan_entry in [start, end]:
    item = load item + lines
    factor = if item.type == Meat { entry.serves } else { 1 }
    for line in item.lines:
      totals[line.ingredient_id].quantity += line.quantity * factor
      totals[line.ingredient_id].sources.push((item.name, entry.date, entry.serves))
  round 'each' up; sort by category.sort_order, ingredient.name
```

Unit-test first: meal vs side vs meat, multiple entries per day, same ingredient across types, rounding, empty range.

---

## 7. Tauri commands (backend API)

```
ingredients:  list, create, update, archive
categories:   list, reorder
items:        list(type?, search?), get(id), create, update, duplicate, archive,
              history(id) -> [(date, serves, note)], latest_note(id), last_had(id)
templates:    list, get, create, update, delete, instantiate(template_id, slot_fills) -> item draft
plan:         list(start, end), add(item_id, date, serves?), move(id, date, position),
              update(id, serves?, note?), remove(id), copy_range(src_start, src_end, dest_start)
summary:      get(start, end)
settings:     get, set
backup:       export_json(), import_json(data)
```

---

## 8. Seed data

Ship with ingredient categories (Meat, Seafood, Veg, Fruit, Dairy, Pantry, Other), ~30 common ingredients, and examples of each type: a few plain meals (Chow mein, Spag bol), a couple of meal templates (Risotto, Stir fry), a few meats (Steak 250 g/serve, Sausages 2/serve, Chicken Kiev 1/serve) and sides (Chips, Garden salad, Steamed veg).

---

## 9. Build phases

Keep each phase small enough to finish and review in one session; commit after each.

1. **Scaffold** — Tauri 2 + React/TS + SQLite, migrations, seed data. Two-page navigation shell.
2. **Ingredients + items CRUD** — Page 2 Items and Ingredients tabs, all three types, per-serve labelling for meats.
3. **Summary logic** — Rust `summary()` with unit tests, before any UI depends on it.
4. **Planner calendar** — view switcher (1 week / 2 weeks / month / scroll), prev/next stepping, lazy-loaded scroll view, Today button, show entries.
5. **Drag and drop** — library → day, move between days, remove, serves stepper for meats.
6. **Notes + history** — entry notes in the chip popover, 📝 indicator, latest note and "last had" on library cards, history in item editor, edit-item drawer from the planner.
7. **Summary UI** — range follows the planner view (1 wk / 2 wk / month) with quick toggles, grouping, contributing items, have-it ticks, copy as text.
8. **Templates (meals only)** — Templates tab + "start from template" toggle in the meal editor.
9. **Polish** — search, colours, settings, copy last week, JSON backup/restore.
10. **Android (stretch)** — Tauri Android build; on narrow screens make the library a slide-up drawer; long-press activation for dnd-kit touch.

---

## 10. Settled decisions

- Templates: meals only, optional via a toggle; items are recipe-style by default.
- Meat per-serve amounts are set on the item and only change by editing the item.
- Past weeks are editable and reference the live item.
- Default meat serves when dropping on the calendar: one household setting (adjust with the stepper per night).
