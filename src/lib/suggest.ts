import type { ItemSummary, ItemType, ItemUsage } from "./api";

/** How many of the longest-since-had items per type we rotate through. */
export const POOL_SIZE = 4;

/**
 * Suggests one meal, one meat and one side that haven't been had for a while: never-had items
 * first, then the longest ago. Items already planned from today onward are skipped.
 *
 * `shuffle` steps through the few best candidates, so pressing "another" cycles rather than
 * repeating (0 = the very longest-since-had).
 */
export function pickSuggestions(
  items: ItemSummary[],
  usage: Record<number, ItemUsage>,
  shuffle: number,
): Partial<Record<ItemType, ItemSummary>> {
  const out: Partial<Record<ItemType, ItemSummary>> = {};
  for (const type of ["meal", "meat", "side"] as const) {
    const pool = items
      .filter((i) => i.type === type && !i.archived && !usage[i.id]?.next_planned)
      .sort((a, b) => {
        const la = usage[a.id]?.last_had ?? ""; // never had sorts first
        const lb = usage[b.id]?.last_had ?? "";
        return la.localeCompare(lb) || a.name.localeCompare(b.name);
      })
      .slice(0, POOL_SIZE);
    if (pool.length > 0) out[type] = pool[shuffle % pool.length];
  }
  return out;
}
