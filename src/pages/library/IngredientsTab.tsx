import { useState } from "react";
import IngredientForm from "../../components/IngredientForm";
import { api, errorMessage, type Ingredient, type IngredientCategory } from "../../lib/api";

interface Props {
  ingredients: Ingredient[];
  categories: IngredientCategory[];
  onChanged: (ingredient: Ingredient) => void;
}

export default function IngredientsTab({ ingredients, categories, onChanged }: Props) {
  const [adding, setAdding] = useState(false);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [showArchived, setShowArchived] = useState(false);
  const [search, setSearch] = useState("");
  const [error, setError] = useState<string | null>(null);

  const q = search.trim().toLowerCase();
  const rows = ingredients.filter(
    (i) => (showArchived || !i.archived) && i.name.toLowerCase().includes(q),
  );

  async function toggleArchive(i: Ingredient) {
    setError(null);
    try {
      onChanged(await api.ingredients.archive(i.id, !i.archived));
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <div className="h-full overflow-auto p-4">
      <div className="mb-3 flex items-center gap-3">
        <input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search ingredients…"
          className="w-64 rounded border border-slate-300 px-2 py-1 text-sm"
        />
        <label className="flex items-center gap-1 text-xs text-slate-500">
          <input
            type="checkbox"
            checked={showArchived}
            onChange={(e) => setShowArchived(e.target.checked)}
          />
          Show archived
        </label>
        <button
          onClick={() => {
            setEditingId(null);
            setAdding(true);
          }}
          className="ml-auto rounded bg-emerald-600 px-3 py-1 text-sm font-medium text-white hover:bg-emerald-700"
        >
          + New ingredient
        </button>
      </div>

      {adding && (
        <div className="mb-3 rounded border border-emerald-200 bg-emerald-50 p-3">
          <IngredientForm
            categories={categories}
            onSaved={(i) => {
              onChanged(i);
              setAdding(false);
            }}
            onCancel={() => setAdding(false)}
          />
        </div>
      )}
      {error && <p className="mb-2 text-sm text-red-600">{error}</p>}

      <table className="w-full max-w-4xl border-collapse bg-white text-sm">
        <thead>
          <tr className="border-b border-slate-200 text-left text-xs text-slate-500">
            <th className="px-3 py-2">Name</th>
            <th className="px-3 py-2">Category</th>
            <th className="px-3 py-2">Unit</th>
            <th className="px-3 py-2 text-right">Used in</th>
            <th className="px-3 py-2" />
          </tr>
        </thead>
        <tbody>
          {rows.map((i) =>
            editingId === i.id ? (
              <tr key={i.id} className="border-b border-slate-100 bg-emerald-50">
                <td colSpan={5} className="px-3 py-2">
                  <IngredientForm
                    categories={categories}
                    existing={i}
                    onSaved={(saved) => {
                      onChanged(saved);
                      setEditingId(null);
                    }}
                    onCancel={() => setEditingId(null)}
                  />
                </td>
              </tr>
            ) : (
              <tr
                key={i.id}
                className={`border-b border-slate-100 ${i.archived ? "text-slate-400" : ""}`}
              >
                <td className="px-3 py-2">{i.name}</td>
                <td className="px-3 py-2">{i.category_name}</td>
                <td className="px-3 py-2">{i.unit}</td>
                <td className="px-3 py-2 text-right">{i.used_in_count}</td>
                <td className="space-x-3 px-3 py-2 text-right">
                  <button
                    onClick={() => {
                      setAdding(false);
                      setEditingId(i.id);
                    }}
                    className="text-emerald-700 hover:underline"
                  >
                    Edit
                  </button>
                  <button
                    onClick={() => toggleArchive(i)}
                    className="text-slate-500 hover:underline"
                  >
                    {i.archived ? "Restore" : "Archive"}
                  </button>
                </td>
              </tr>
            ),
          )}
        </tbody>
      </table>
      {rows.length === 0 && <p className="p-4 text-sm text-slate-400">No ingredients found.</p>}
    </div>
  );
}
