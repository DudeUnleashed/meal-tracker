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

export const api = {
  categories: {
    list: () => invoke<IngredientCategory[]>("categories_list"),
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
  },
};

/** Tauri rejects with the Rust error string. */
export function errorMessage(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}
