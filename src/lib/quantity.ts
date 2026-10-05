import type { Unit } from "./api";

export type QuantityResult = { ok: true; value: number } | { ok: false; error: string };

// What people type -> multiplier into the ingredient's own unit (grams / millilitres).
const WEIGHT: Record<string, number> = {
  g: 1, gm: 1, gram: 1, grams: 1,
  kg: 1000, kgs: 1000, kilo: 1000, kilos: 1000, kilogram: 1000, kilograms: 1000,
};
const VOLUME: Record<string, number> = {
  ml: 1, millilitre: 1, millilitres: 1, milliliter: 1, milliliters: 1,
  l: 1000, lt: 1000, ltr: 1000, litre: 1000, litres: 1000, liter: 1000, liters: 1000,
};

const SHAPE = /^(\d+(?:\.\d*)?|\.\d+)\s*([a-z]*)$/;

/**
 * Reads a quantity the way people write it and converts it into the ingredient's unit:
 * `250` stays 250, `1.5kg` becomes 1500 for a gram ingredient, `1l` becomes 1000 for ml.
 * `unit` is the ingredient's unit, or "" if no ingredient is chosen yet.
 */
export function parseQuantity(text: string, unit: Unit | ""): QuantityResult {
  const t = text.trim().toLowerCase().replace(/^(\d+),(\d+)(?!\d*,)/, "$1.$2"); // "1,5kg" -> "1.5kg"
  if (t === "") return { ok: false, error: "Enter a quantity" };
  const m = SHAPE.exec(t);
  if (!m) return { ok: false, error: `“${text.trim()}” isn't a quantity — try 250, 1.5kg or 500ml` };

  const number = Number(m[1]);
  const suffix = m[2];
  let value = number;

  if (suffix !== "") {
    const table = unit === "g" ? WEIGHT : unit === "ml" ? VOLUME : null;
    if (unit === "") {
      return { ok: false, error: "Pick the ingredient first so I know what unit to convert to" };
    }
    if (unit === "each") {
      return { ok: false, error: "This ingredient is counted in whole items — enter just a number" };
    }
    const factor = table?.[suffix];
    if (factor === undefined) {
      const wants = unit === "g" ? "g or kg" : "ml or L";
      return { ok: false, error: `This ingredient is measured in ${unit} — use ${wants}` };
    }
    value = number * factor;
  }

  value = Math.round(value * 10000) / 10000; // tidy floating point noise
  if (!Number.isFinite(value) || value <= 0) return { ok: false, error: "Quantity must be more than zero" };
  return { ok: true, value };
}
