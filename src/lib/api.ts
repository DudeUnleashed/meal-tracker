import { invoke } from "@tauri-apps/api/core";

// Ingredient category: how ingredients are grouped in the shopping summary.
// (Not to be confused with item type: meal / meat / side.)
export interface IngredientCategory {
  id: number;
  name: string;
  sort_order: number;
}

export const api = {
  categories: {
    list: () => invoke<IngredientCategory[]>("categories_list"),
  },
};
