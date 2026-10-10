import type { ItemSummary } from "./api";

export interface ItemGroup {
  category_id: number;
  name: string;
  items: ItemSummary[];
}

/**
 * Splits items into collapsible groups (one per dish category, picked by hand on each item) and
 * the rest (items with no category). Order within each list follows the order of `items`;
 * groups are alphabetical.
 */
export function groupItems(items: ItemSummary[]): { groups: ItemGroup[]; rest: ItemSummary[] } {
  const byCategory = new Map<number, ItemGroup>();
  const rest: ItemSummary[] = [];
  for (const item of items) {
    if (item.category_id === null || item.category_name === null) {
      rest.push(item);
      continue;
    }
    const group = byCategory.get(item.category_id) ?? { category_id: item.category_id, name: item.category_name, items: [] };
    group.items.push(item);
    byCategory.set(item.category_id, group);
  }
  const groups = [...byCategory.values()].sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
  return { groups, rest };
}
