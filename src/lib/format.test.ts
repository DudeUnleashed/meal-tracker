import { describe, expect, it } from "vitest";
import type { Summary, SummaryRow } from "./api";
import { formatQuantity, groupByCategory, summaryToText } from "./format";
import { blockRange, shiftRange, stepAnchor } from "./ranges";

describe("formatQuantity", () => {
  it("shows small weights in grams and large ones in kg", () => {
    expect(formatQuantity(600, "g")).toBe("600 g");
    expect(formatQuantity(750, "g")).toBe("750 g");
    expect(formatQuantity(1000, "g")).toBe("1 kg");
    expect(formatQuantity(1500, "g")).toBe("1.5 kg");
    expect(formatQuantity(1234, "g")).toBe("1.23 kg");
  });

  it("does not show 1000 g at the boundary", () => {
    expect(formatQuantity(999.96, "g")).toBe("1 kg");
    expect(formatQuantity(999.4, "g")).toBe("999.4 g");
  });

  it("keeps fractional grams to one decimal", () => {
    expect(formatQuantity(187.5, "g")).toBe("187.5 g");
    expect(formatQuantity(62.25, "g")).toBe("62.3 g");
  });

  it("handles volumes", () => {
    expect(formatQuantity(60, "ml")).toBe("60 ml");
    expect(formatQuantity(1000, "ml")).toBe("1 L");
    expect(formatQuantity(1500, "ml")).toBe("1.5 L");
  });

  it("shows each as a bare count", () => {
    expect(formatQuantity(7, "each")).toBe("7");
    expect(formatQuantity(0, "each")).toBe("0");
  });
});

const row = (id: number, name: string, category: string, quantity: number, unit: SummaryRow["unit"]): SummaryRow => ({
  ingredient_id: id,
  ingredient_name: name,
  category_id: 1,
  category_name: category,
  unit,
  quantity,
  sources: [],
});

const summary = (rows: SummaryRow[]): Summary => ({
  start: "2026-10-05",
  end: "2026-10-18",
  entry_count: 3,
  dinner_days: 3,
  rows,
});

const ROWS = [
  row(1, "Beef mince", "Meat", 1000, "g"),
  row(2, "Sausages", "Meat", 4, "each"),
  row(3, "Carrot", "Veg", 6, "each"),
  row(4, "Soy sauce", "Pantry", 60, "ml"),
];

describe("groupByCategory", () => {
  it("groups consecutive rows by category in order", () => {
    const g = groupByCategory(ROWS);
    expect(g.map((x) => x.category)).toEqual(["Meat", "Veg", "Pantry"]);
    expect(g[0].rows.map((r) => r.ingredient_name)).toEqual(["Beef mince", "Sausages"]);
  });

  it("is empty for no rows", () => {
    expect(groupByCategory([])).toEqual([]);
  });
});

describe("summaryToText", () => {
  it("lists every ingredient grouped by category", () => {
    expect(summaryToText(summary(ROWS), "5 Oct – 18 Oct 2026", new Set())).toBe(
      [
        "Shopping list: 5 Oct – 18 Oct 2026",
        "",
        "MEAT",
        "- Beef mince: 1 kg",
        "- Sausages: 4",
        "",
        "VEG",
        "- Carrot: 6",
        "",
        "PANTRY",
        "- Soy sauce: 60 ml",
      ].join("\n"),
    );
  });

  it("leaves out ticked items and any category that becomes empty", () => {
    const text = summaryToText(summary(ROWS), "Week", new Set([3, 2]));
    expect(text).toContain("- Beef mince: 1 kg");
    expect(text).not.toContain("Sausages");
    expect(text).not.toContain("Carrot");
    expect(text).not.toContain("VEG");
    expect(text).toContain("PANTRY");
  });

  it("says so when there is nothing to buy", () => {
    expect(summaryToText(summary(ROWS), "Week", new Set([1, 2, 3, 4]))).toBe(
      "Shopping list: Week\n\nNothing to buy.",
    );
    expect(summaryToText(summary([]), "Week", new Set())).toContain("Nothing to buy.");
  });
});

describe("blockRange", () => {
  // 2026-10-07 is a Wednesday.
  it("1 week and 2 weeks start on the week-start day", () => {
    const one = blockRange("1w", "2026-10-07", 1);
    expect([one.start, one.end, one.weekCount]).toEqual(["2026-10-05", "2026-10-11", 1]);
    const two = blockRange("2w", "2026-10-07", 1);
    expect([two.start, two.end, two.weekCount]).toEqual(["2026-10-05", "2026-10-18", 2]);
    expect(two.title).toBe("5 Oct – 18 Oct 2026");
  });

  it("respects a Sunday week start", () => {
    const r = blockRange("1w", "2026-10-07", 0);
    expect([r.start, r.end]).toEqual(["2026-10-04", "2026-10-10"]);
  });

  it("month covers the calendar month but the grid pads to whole weeks", () => {
    const r = blockRange("month", "2026-10-15", 1);
    expect([r.start, r.end]).toEqual(["2026-10-01", "2026-10-31"]);
    expect(r.firstWeek).toBe("2026-09-28");
    expect(r.weekCount).toBe(5);
    expect(r.month).toBe("2026-10");
    expect(r.title).toBe("October 2026");
  });

  it("the scroll view summarises one week", () => {
    const r = blockRange("scroll", "2026-10-07", 1);
    expect([r.start, r.end]).toEqual(["2026-10-05", "2026-10-11"]);
  });
});

describe("stepAnchor / shiftRange", () => {
  it("steps by the block size", () => {
    expect(stepAnchor("1w", "2026-10-07", 1, 1)).toBe("2026-10-12");
    expect(stepAnchor("2w", "2026-10-07", 1, 1)).toBe("2026-10-19");
    expect(stepAnchor("2w", "2026-10-07", -1, 1)).toBe("2026-09-21");
    expect(stepAnchor("month", "2026-10-15", 1, 1)).toBe("2026-11-01");
    expect(stepAnchor("month", "2026-01-31", -1, 1)).toBe("2025-12-01");
  });

  it("shifts a custom range by its own length", () => {
    expect(shiftRange("2026-10-05", "2026-10-11", 1)).toEqual({ start: "2026-10-12", end: "2026-10-18" });
    expect(shiftRange("2026-10-05", "2026-10-09", -1)).toEqual({ start: "2026-09-30", end: "2026-10-04" });
  });
});
