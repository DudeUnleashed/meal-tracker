import { describe, expect, it } from "vitest";
import type { ItemSummary } from "./api";
import { groupItems } from "./groups";

const item = (id: number, name: string, groups: [number, string][] = []): ItemSummary => ({
  id, name, type: "meal", colour: null, archived: false, ingredient_count: 1, live_template_id: null,
  groups: groups.map(([ingredient_id, n]) => ({ ingredient_id, name: n })),
});

describe("groupItems", () => {
  it("groups only items that were ticked, and leaves the rest alone", () => {
    const items = [item(1, "Beef rissoles", [[10, "Beef mince"]]), item(2, "Carbonara"), item(3, "Beef pie", [[10, "Beef mince"]])];
    const { groups, rest } = groupItems(items);
    expect(groups.map((g) => [g.name, g.items.map((i) => i.id)])).toEqual([["Beef mince", [1, 3]]]);
    expect(rest.map((i) => i.id)).toEqual([2]);
  });

  it("lists a dish under every ingredient it is ticked for, groups alphabetically", () => {
    const { groups, rest } = groupItems([item(1, "Surf n turf", [[2, "Steak"], [1, "Prawns"]])]);
    expect(groups.map((g) => g.name)).toEqual(["Prawns", "Steak"]);
    expect(rest).toEqual([]);
  });

  it("keeps the incoming order inside a group (so sorting still applies)", () => {
    const { groups } = groupItems([item(2, "b", [[1, "X"]]), item(1, "a", [[1, "X"]])]);
    expect(groups[0].items.map((i) => i.id)).toEqual([2, 1]);
  });

  it("returns everything as the rest when nothing is ticked", () => {
    const { groups, rest } = groupItems([item(1, "a"), item(2, "b")]);
    expect(groups).toEqual([]);
    expect(rest).toHaveLength(2);
  });
});
