import { describe, expect, it } from "vitest";
import type { ItemSummary } from "./api";
import { groupItems } from "./groups";

const item = (id: number, name: string, category?: [number, string]): ItemSummary => ({
  id, name, type: "meal", colour: null, archived: false, ingredient_count: 1, live_template_id: null,
  category_id: category?.[0] ?? null, category_name: category?.[1] ?? null,
});

describe("groupItems", () => {
  it("groups only items that were given a category, and leaves the rest alone", () => {
    const items = [item(1, "Beef rissoles", [10, "Beef"]), item(2, "Carbonara"), item(3, "Beef pie", [10, "Beef"])];
    const { groups, rest } = groupItems(items);
    expect(groups.map((g) => [g.name, g.items.map((i) => i.id)])).toEqual([["Beef", [1, 3]]]);
    expect(rest.map((i) => i.id)).toEqual([2]);
  });

  it("orders groups alphabetically whatever order the items come in", () => {
    const { groups } = groupItems([item(1, "a", [2, "Pork"]), item(2, "b", [1, "beef"]), item(3, "c", [3, "Chicken"])]);
    expect(groups.map((g) => g.name)).toEqual(["beef", "Chicken", "Pork"]);
  });

  it("keeps the incoming order inside a group (so sorting still applies)", () => {
    const { groups } = groupItems([item(2, "b", [1, "X"]), item(1, "a", [1, "X"])]);
    expect(groups[0].items.map((i) => i.id)).toEqual([2, 1]);
  });

  it("returns everything as the rest when nothing is categorised", () => {
    const { groups, rest } = groupItems([item(1, "a"), item(2, "b")]);
    expect(groups).toEqual([]);
    expect(rest).toHaveLength(2);
  });
});
