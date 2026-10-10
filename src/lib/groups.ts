import type { ItemSummary } from "./api";

export interface ItemGroup {
  ingredient_id: number;
  name: string;
  items: ItemSummary[];
}

/**
 * Splits items into collapsible groups (one per ingredient ticked "group by" on a recipe) and
 * the rest. A dish ticked under two ingredients appears under both; a dish with no tick is only
 * in `rest`. Order within each list follows the order of `items`; groups are alphabetical.
 */
export function groupItems(items: ItemSummary[]): { groups: ItemGroup[]; rest: ItemSummary[] } {
  const byIngredient = new Map<number, ItemGroup>();
  const rest: ItemSummary[] = [];
  for (const item of items) {
    if (item.groups.length === 0) rest.push(item);
    for (const g of item.groups) {
      const group = byIngredient.get(g.ingredient_id) ?? { ingredient_id: g.ingredient_id, name: g.name, items: [] };
      group.items.push(item);
      byIngredient.set(g.ingredient_id, group);
    }
  }
  const groups = [...byIngredient.values()].sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
  return { groups, rest };
}
