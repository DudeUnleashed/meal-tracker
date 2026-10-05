import { describe, expect, it } from "vitest";
import type { ItemSummary, ItemType, ItemUsage } from "./api";
import { parseQuantity } from "./quantity";
import { pickSuggestions } from "./suggest";

const value = (text: string, unit: Parameters<typeof parseQuantity>[1]) => {
  const r = parseQuantity(text, unit);
  return r.ok ? r.value : `error: ${r.error}`;
};

describe("parseQuantity", () => {
  it("passes plain numbers through in the ingredient's own unit", () => {
    expect(value("250", "g")).toBe(250);
    expect(value("  2.5 ", "each")).toBe(2.5);
    expect(value(".5", "ml")).toBe(0.5);
    expect(value("3", "")).toBe(3);
  });

  it("converts kilograms to grams", () => {
    expect(value("1.5kg", "g")).toBe(1500);
    expect(value("1.5 kg", "g")).toBe(1500);
    expect(value("2KG", "g")).toBe(2000);
    expect(value("0.25 kilos", "g")).toBe(250);
    expect(value("750g", "g")).toBe(750);
    expect(value("1 gram", "g")).toBe(1);
  });

  it("converts litres to millilitres", () => {
    expect(value("1l", "ml")).toBe(1000);
    expect(value("1.5 L", "ml")).toBe(1500);
    expect(value("2 litres", "ml")).toBe(2000);
    expect(value("250ml", "ml")).toBe(250);
  });

  it("accepts a comma as the decimal separator", () => {
    expect(value("1,5kg", "g")).toBe(1500);
    expect(value("0,25", "g")).toBe(0.25);
  });

  it("avoids floating point noise", () => {
    expect(value("0.1kg", "g")).toBe(100);
    expect(value("1.1kg", "g")).toBe(1100);
  });

  it("rejects units that don't fit the ingredient", () => {
    expect(value("1.5kg", "ml")).toMatch(/error: .*ml or L/);
    expect(value("1l", "g")).toMatch(/error: .*g or kg/);
    expect(value("2kg", "each")).toMatch(/error: .*whole items/);
    expect(value("1.5kg", "")).toMatch(/error: Pick the ingredient first/);
  });

  it("rejects blanks, junk and non-positive amounts", () => {
    expect(value("", "g")).toBe("error: Enter a quantity");
    expect(value("   ", "g")).toBe("error: Enter a quantity");
    expect(value("lots", "g")).toMatch(/error: .*isn't a quantity/);
    expect(value("1.5.2", "g")).toMatch(/error/);
    expect(value("-5", "g")).toMatch(/error/);
    expect(value("0", "g")).toMatch(/error: Quantity must be more than zero/);
    expect(value("0kg", "g")).toMatch(/error: Quantity must be more than zero/);
  });
});

const item = (id: number, name: string, type: ItemType, archived = false): ItemSummary => ({
  id, name, type, colour: null, archived, ingredient_count: 1,
});
const used = (lastHad: string | null, nextPlanned: string | null = null): ItemUsage => ({
  item_id: 0, last_had: lastHad, next_planned: nextPlanned, latest_note: null, latest_note_date: null,
});

describe("pickSuggestions", () => {
  const items = [
    item(1, "Chow mein", "meal"),
    item(2, "Spag bol", "meal"),
    item(3, "Risotto", "meal"),
    item(4, "Steak", "meat"),
    item(5, "Sausages", "meat"),
    item(6, "Chips", "side"),
  ];

  it("suggests the one that has gone longest, picking one of each type", () => {
    const usage = { 1: used("2026-09-20"), 2: used("2026-08-01"), 3: used("2026-09-30"), 4: used("2026-09-01"), 5: used("2026-09-10"), 6: used("2026-10-01") };
    const s = pickSuggestions(items, usage, 0);
    expect([s.meal?.name, s.meat?.name, s.side?.name]).toEqual(["Spag bol", "Steak", "Chips"]);
  });

  it("puts never-had items first", () => {
    const usage = { 1: used("2026-09-20"), 2: used("2026-08-01") }; // Risotto never had
    expect(pickSuggestions(items, usage, 0).meal?.name).toBe("Risotto");
  });

  it("skips items that are already planned from today on", () => {
    const usage = { 2: used("2026-08-01", "2026-10-09"), 1: used("2026-09-20"), 3: used("2026-09-30") };
    expect(pickSuggestions(items, usage, 0).meal?.name).toBe("Chow mein");
  });

  it("cycles through the best candidates instead of repeating", () => {
    const usage = { 1: used("2026-09-01"), 2: used("2026-09-02"), 3: used("2026-09-03") };
    const names = [0, 1, 2, 3].map((n) => pickSuggestions(items, usage, n).meal?.name);
    expect(names).toEqual(["Chow mein", "Spag bol", "Risotto", "Chow mein"]);
  });

  it("leaves out a type with nothing to suggest and archived items", () => {
    const s = pickSuggestions([item(1, "Chow mein", "meal", true), item(6, "Chips", "side")], {}, 0);
    expect(s.meal).toBeUndefined();
    expect(s.meat).toBeUndefined();
    expect(s.side?.name).toBe("Chips");
  });
});
