import { useEffect, useRef, useState } from "react";
import type { Ingredient } from "../lib/api";

interface Props {
  ingredients: Ingredient[];
  value: number | null;
  onChange: (ingredient: Ingredient) => void;
  /** Called with the current search text when the user wants to create a new ingredient. */
  onCreate?: (name: string) => void;
  /** Only offer ingredients from this category (template slots, e.g. Protein → Meat). */
  categoryId?: number | null;
}

/** Searchable ingredient dropdown with a "create new" escape hatch. */
export default function IngredientPicker({ ingredients, value, onChange, onCreate, categoryId }: Props) {
  const selected = ingredients.find((i) => i.id === value) ?? null;
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  const q = query.trim().toLowerCase();
  const matches = ingredients
    .filter(
      (i) =>
        !i.archived &&
        (categoryId == null || i.category_id === categoryId) &&
        i.name.toLowerCase().includes(q),
    )
    .slice(0, 50);
  const exact = ingredients.some((i) => i.name.toLowerCase() === q);

  return (
    <div ref={root} className="relative">
      <input
        value={open ? query : (selected?.name ?? "")}
        placeholder="Search ingredients…"
        onFocus={() => {
          setQuery("");
          setOpen(true);
        }}
        onChange={(e) => setQuery(e.target.value)}
        className="w-full rounded border border-slate-300 px-2 py-1 text-sm"
      />
      {open && (
        <ul className="absolute z-20 mt-1 max-h-60 w-full overflow-auto rounded border border-slate-200 bg-white text-sm shadow-lg">
          {matches.map((i) => (
            <li key={i.id}>
              <button
                type="button"
                onClick={() => {
                  onChange(i);
                  setOpen(false);
                }}
                className="flex w-full justify-between px-2 py-1 text-left hover:bg-emerald-50"
              >
                <span>{i.name}</span>
                <span className="text-xs text-slate-400">
                  {i.category_name} · {i.unit}
                </span>
              </button>
            </li>
          ))}
          {matches.length === 0 && <li className="px-2 py-1 text-slate-400">No matches</li>}
          {onCreate && q && !exact && (
            <li>
              <button
                type="button"
                onClick={() => {
                  onCreate(query.trim());
                  setOpen(false);
                }}
                className="w-full border-t border-slate-100 px-2 py-1 text-left font-medium text-emerald-700 hover:bg-emerald-50"
              >
                + Create “{query.trim()}”
              </button>
            </li>
          )}
        </ul>
      )}
    </div>
  );
}
