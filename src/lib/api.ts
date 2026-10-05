import { invoke } from "@tauri-apps/api/core";

// Ingredient category: how ingredients are grouped in the shopping summary.
// (Not to be confused with item type: meal / meat / side.)
export interface IngredientCategory {
  id: number;
  name: string;
  sort_order: number;
}

export type Unit = "g" | "ml" | "each";
export const UNITS: Unit[] = ["g", "ml", "each"];

export interface Ingredient {
  id: number;
  name: string;
  category_id: number;
  category_name: string;
  unit: Unit;
  archived: boolean;
  used_in_count: number;
}

export interface IngredientInput {
  name: string;
  category_id: number;
  unit: Unit;
}

// Item type: what an item *is* on the calendar.
export type ItemType = "meal" | "meat" | "side";
export const ITEM_TYPES: ItemType[] = ["meal", "meat", "side"];

export interface ItemSummary {
  id: number;
  name: string;
  type: ItemType;
  colour: string | null;
  archived: boolean;
  ingredient_count: number;
}

export interface ItemLine {
  ingredient_id: number;
  ingredient_name: string;
  unit: Unit;
  /** Per serve for meat items; the fixed amount otherwise. */
  quantity: number;
}

export interface Item {
  id: number;
  name: string;
  type: ItemType;
  colour: string | null;
  notes: string | null;
  template_id: number | null;
  archived: boolean;
  lines: ItemLine[];
}

export interface ItemInput {
  name: string;
  type: ItemType;
  colour: string | null;
  notes: string | null;
  template_id: number | null;
  lines: { ingredient_id: number; quantity: number }[];
}

export interface SummarySource {
  item_id: number;
  item_name: string;
  date: string;
  /** Serves on the entry (meat only). */
  serves: number | null;
  /** Size multiplier on a meal or side entry; only present when it isn't the usual 1. */
  scale: number | null;
  /** What this entry adds, already scaled by serves or size. */
  quantity: number;
}

export interface SummaryRow {
  ingredient_id: number;
  ingredient_name: string;
  category_id: number;
  category_name: string;
  unit: Unit;
  /** `each` totals are already rounded up to whole items. */
  quantity: number;
  sources: SummarySource[];
}

export interface Summary {
  start: string;
  end: string;
  entry_count: number;
  dinner_days: number;
  /** Sorted by category order, then ingredient name. */
  rows: SummaryRow[];
}

export interface PlanEntry {
  id: number;
  /** ISO `YYYY-MM-DD`. */
  date: string;
  item_id: number;
  item_name: string;
  item_type: ItemType;
  item_colour: string | null;
  item_archived: boolean;
  /** Meat entries only. */
  serves: number | null;
  note: string | null;
  position: number;
  /** Size multiplier for meal and side entries (1 = one night's amount). Meat uses serves. */
  scale: number;
}

/** A note and/or the Leftovers marker on a day (not tied to any dinner). */
export interface DayInfo {
  date: string;
  note: string | null;
  leftovers: boolean;
}

/** When an item was last had / is next planned, and the note most recently left on it. */
export interface ItemUsage {
  item_id: number;
  /** Most recent date before today. */
  last_had: string | null;
  /** Earliest date today or later. */
  next_planned: string | null;
  latest_note: string | null;
  latest_note_date: string | null;
}

export interface HistoryEntry {
  entry_id: number;
  date: string;
  serves: number | null;
  note: string | null;
}

export type Settings = Record<string, string>;

export interface CopyResult {
  copied: number;
  /** Left out because their item has been archived since. */
  skipped_archived: number;
}

export interface SampleCounts {
  items: number;
  ingredients: number;
  templates: number;
  /** Planned dinners that use a sample item (these go with it). */
  planned_entries: number;
}

export interface RemovedSample {
  items: number;
  templates: number;
  ingredients: number;
  plan_entries: number;
  /** Sample ingredients kept because your own meals or templates use them. */
  ingredients_kept: number;
}

export interface Erased {
  items: number;
  ingredients: number;
  templates: number;
  plan_entries: number;
  day_notes: number;
}

export interface WipeReport<T> {
  result: T;
  backup_path: string;
}

export interface ImportSummary {
  items: number;
  ingredients: number;
  templates: number;
  plan_entries: number;
}

export interface TemplateSummary {
  id: number;
  name: string;
  fixed_count: number;
  slot_count: number;
}

/** Fixed line: has an ingredient. Slot: `ingredient_id` is null and is filled in per meal. */
export interface TemplateLine {
  id: number;
  label: string;
  ingredient_id: number | null;
  ingredient_name: string | null;
  ingredient_unit: Unit | null;
  /** Filters the ingredient picker for slots (e.g. Meat). */
  category_id: number | null;
  category_name: string | null;
  quantity: number | null;
  unit_hint: Unit | null;
}

export interface Template {
  id: number;
  name: string;
  lines: TemplateLine[];
}

export interface TemplateInput {
  name: string;
  lines: {
    label: string;
    ingredient_id: number | null;
    category_id: number | null;
    quantity: number | null;
    unit_hint: Unit | null;
  }[];
}

export interface DraftLine {
  ingredient_id: number;
  ingredient_name: string;
  unit: Unit;
  /** Null when the template default doesn't apply (e.g. grams default but the pick is counted "each"). */
  quantity: number | null;
  label: string;
}

/** Unsaved meal built from a template; every line is plain and editable. */
export interface ItemDraft {
  name: string;
  template_id: number;
  lines: DraftLine[];
}

export const api = {
  categories: {
    list: () => invoke<IngredientCategory[]>("categories_list"),
    /** `ids` = every category id, in the new order (also the order of the shopping summary). */
    reorder: (ids: number[]) => invoke<IngredientCategory[]>("categories_reorder", { ids }),
  },
  ingredients: {
    list: (includeArchived = false) =>
      invoke<Ingredient[]>("ingredients_list", { includeArchived }),
    create: (input: IngredientInput) =>
      invoke<Ingredient>("ingredients_create", { input }),
    update: (id: number, input: IngredientInput) =>
      invoke<Ingredient>("ingredients_update", { id, input }),
    archive: (id: number, archived: boolean) =>
      invoke<Ingredient>("ingredients_archive", { id, archived }),
  },
  items: {
    list: (opts: { type?: ItemType; search?: string; includeArchived?: boolean } = {}) =>
      invoke<ItemSummary[]>("items_list", {
        itemType: opts.type ?? null,
        search: opts.search ?? null,
        includeArchived: opts.includeArchived ?? false,
      }),
    get: (id: number) => invoke<Item>("items_get", { id }),
    create: (input: ItemInput) => invoke<Item>("items_create", { input }),
    update: (id: number, input: ItemInput) => invoke<Item>("items_update", { id, input }),
    duplicate: (id: number) => invoke<Item>("items_duplicate", { id }),
    archive: (id: number, archived: boolean) =>
      invoke<Item>("items_archive", { id, archived }),
    /** Usage for every item that has been planned; `today` is a local ISO date. */
    usage: (today: string) => invoke<ItemUsage[]>("items_usage", { today }),
    /** Every plan entry for the item, newest first. */
    history: (id: number) => invoke<HistoryEntry[]>("items_history", { id }),
  },
  templates: {
    list: () => invoke<TemplateSummary[]>("templates_list"),
    get: (id: number) => invoke<Template>("templates_get", { id }),
    create: (input: TemplateInput) => invoke<Template>("templates_create", { input }),
    update: (id: number, input: TemplateInput) =>
      invoke<Template>("templates_update", { id, input }),
    delete: (id: number) => invoke<void>("templates_delete", { id }),
    /** Unfilled slots are skipped. */
    instantiate: (templateId: number, slotFills: { line_id: number; ingredient_id: number }[]) =>
      invoke<ItemDraft>("templates_instantiate", { templateId, slotFills }),
  },
  plan: {
    list: (start: string, end: string) => invoke<PlanEntry[]>("plan_list", { start, end }),
    /** `position` (0-based) inserts before that entry; omit to append to the day. */
    add: (itemId: number, date: string, serves?: number, position?: number) =>
      invoke<PlanEntry>("plan_add", { itemId, date, serves: serves ?? null, position: position ?? null }),
    /** Moves to `date` at 0-based `position` in that day (clamped to the end). */
    move: (id: number, date: string, position: number) =>
      invoke<PlanEntry>("plan_move", { id, date, position }),
    /** Omitted fields are unchanged; a blank note clears it. */
    update: (id: number, patch: { serves?: number; note?: string; scale?: number }) =>
      invoke<PlanEntry>("plan_update", {
        id,
        serves: patch.serves ?? null,
        note: patch.note ?? null,
        scale: patch.scale ?? null,
      }),
    /** Reverts the most recent plan change; resolves to what was undone, or null if nothing was. */
    undo: () => invoke<string | null>("plan_undo"),
    /** What Undo would revert right now (e.g. "Removed Steak"), or null. */
    undoPeek: () => invoke<string | null>("plan_undo_peek"),
    /** Copies the range so it starts on `destStart`; serves, size and leftovers are kept, notes are not. */
    copyRange: (srcStart: string, srcEnd: string, destStart: string) =>
      invoke<CopyResult>("plan_copy_range", { srcStart, srcEnd, destStart }),
    remove: (id: number) => invoke<void>("plan_remove", { id }),
  },
  settings: {
    get: () => invoke<Settings>("settings_get"),
    set: (key: string, value: string) => invoke<void>("settings_set", { key, value }),
  },
  dayInfo: {
    list: (start: string, end: string) => invoke<DayInfo[]>("day_info_list", { start, end }),
    /** Omitted fields are unchanged; a blank note clears it. */
    set: (date: string, patch: { note?: string; leftovers?: boolean }) =>
      invoke<DayInfo | null>("day_info_set", {
        date,
        note: patch.note ?? null,
        leftovers: patch.leftovers ?? null,
      }),
    /** Moves the Leftovers marker between days in one undoable step. */
    moveLeftovers: (from: string, to: string) => invoke<void>("day_leftovers_move", { from, to }),
  },
  data: {
    sampleCounts: () => invoke<SampleCounts>("data_sample_counts"),
    /** Saves a safety backup, then removes the demo content. */
    removeSample: () => invoke<WipeReport<RemovedSample>>("data_remove_sample"),
    /** Saves a safety backup, then deletes everything you've entered. */
    eraseAll: () => invoke<WipeReport<Erased>>("data_erase_all"),
  },
  backup: {
    /** Writes the whole database as JSON to `path`. */
    export: (path: string) => invoke<void>("backup_export", { path }),
    /** Replaces ALL data with the backup file's. All-or-nothing. */
    import: (path: string) => invoke<ImportSummary>("backup_import", { path }),
  },
  summary: {
    /** Dates are ISO `YYYY-MM-DD`, inclusive. */
    get: (start: string, end: string) => invoke<Summary>("summary_get", { start, end }),
  },
};

/** Tauri rejects with the Rust error string. */
export function errorMessage(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}
