import type { Summary, SummaryRow, Unit } from "./api";

/** Rounds to `dp` decimals and drops trailing zeros: 1.50 -> "1.5", 2.00 -> "2". */
const trim = (n: number, dp: number): string => String(Number(n.toFixed(dp)));

/**
 * Human quantity for a shopping list: weights as g / kg, volumes as ml / L,
 * `each` as a bare count.
 */
export function formatQuantity(quantity: number, unit: Unit): string {
  if (unit === "each") return trim(quantity, 0);
  const rounded = Math.round(quantity * 10) / 10; // so 999.96 g becomes 1 kg, not "1000 g"
  const [big, small] = unit === "g" ? ["kg", "g"] : ["L", "ml"];
  return rounded >= 1000 ? `${trim(rounded / 1000, 2)} ${big}` : `${trim(rounded, 1)} ${small}`;
}

/** Rows grouped by ingredient category, keeping the (already sorted) order. */
export function groupByCategory(rows: SummaryRow[]): { category: string; rows: SummaryRow[] }[] {
  const groups: { category: string; rows: SummaryRow[] }[] = [];
  for (const row of rows) {
    const last = groups[groups.length - 1];
    if (last && last.category === row.category_name) last.rows.push(row);
    else groups.push({ category: row.category_name, rows: [row] });
  }
  return groups;
}

/** Plain-text shopping list for pasting into notes or messages; ticked ("have it") rows are left out. */
export function summaryToText(summary: Summary, title: string, have: ReadonlySet<number>): string {
  const groups = groupByCategory(summary.rows.filter((r) => !have.has(r.ingredient_id)));
  if (groups.length === 0) {
    return `Shopping list: ${title}\n\nNothing to buy.`;
  }
  const body = groups.map(
    (g) =>
      `${g.category.toUpperCase()}\n` +
      g.rows.map((r) => `- ${r.ingredient_name}: ${formatQuantity(r.quantity, r.unit)}`).join("\n"),
  );
  return `Shopping list: ${title}\n\n${body.join("\n\n")}`;
}
