import { useState } from "react";
import {
  api,
  errorMessage,
  UNITS,
  type Ingredient,
  type IngredientCategory,
  type Unit,
} from "../lib/api";

interface Props {
  categories: IngredientCategory[];
  /** Pre-fills the name (e.g. from a picker search). */
  initialName?: string;
  /** Existing ingredient to edit; omit to create. */
  existing?: Ingredient;
  onSaved: (ingredient: Ingredient) => void;
  onCancel: () => void;
}

/** Name + category + unit. Used for inline creation and the Ingredients tab. */
export default function IngredientForm({
  categories,
  initialName = "",
  existing,
  onSaved,
  onCancel,
}: Props) {
  const [name, setName] = useState(existing?.name ?? initialName);
  const [categoryId, setCategoryId] = useState(
    existing?.category_id ?? categories[0]?.id ?? 0,
  );
  const [unit, setUnit] = useState<Unit>(existing?.unit ?? "g");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    e.stopPropagation();
    setSaving(true);
    setError(null);
    try {
      const input = { name, category_id: categoryId, unit };
      const saved = existing
        ? await api.ingredients.update(existing.id, input)
        : await api.ingredients.create(input);
      onSaved(saved);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <form onSubmit={submit} className="flex flex-wrap items-end gap-2">
      <label className="flex flex-col text-xs text-slate-500">
        Name
        <input
          autoFocus
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="w-44 rounded border border-slate-300 px-2 py-1 text-sm text-slate-900"
        />
      </label>
      <label className="flex flex-col text-xs text-slate-500">
        Category
        <select
          value={categoryId}
          onChange={(e) => setCategoryId(Number(e.target.value))}
          className="rounded border border-slate-300 px-2 py-1 text-sm text-slate-900"
        >
          {categories.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
      </label>
      <label className="flex flex-col text-xs text-slate-500">
        Unit
        <select
          value={unit}
          onChange={(e) => setUnit(e.target.value as Unit)}
          className="rounded border border-slate-300 px-2 py-1 text-sm text-slate-900"
        >
          {UNITS.map((u) => (
            <option key={u} value={u}>
              {u}
            </option>
          ))}
        </select>
      </label>
      <button
        type="submit"
        disabled={saving}
        className="rounded bg-emerald-600 px-3 py-1 text-sm font-medium text-white hover:bg-emerald-700 disabled:opacity-50"
      >
        {existing ? "Save" : "Add"}
      </button>
      <button
        type="button"
        onClick={onCancel}
        className="rounded px-3 py-1 text-sm text-slate-600 hover:bg-slate-100"
      >
        Cancel
      </button>
      {error && <p className="w-full text-sm text-red-600">{error}</p>}
    </form>
  );
}
