import { useCallback, useEffect, useState } from "react";
import { api, errorMessage, type DishCategory } from "../../lib/api";

/**
 * The categories you can file a dish under (Beef, Chicken, Soups…). They become the collapsible
 * headings in the planner and Library lists. Add your own, rename them, or delete ones you don't use.
 */
export default function DishCategoriesSection({ className }: { className: string }) {
  const [cats, setCats] = useState<DishCategory[]>([]);
  const [newName, setNewName] = useState("");
  const [editing, setEditing] = useState<{ id: number; name: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    api.dishCategories
      .list()
      .then(setCats)
      .catch((e) => setError(errorMessage(e)));
  }, []);
  useEffect(reload, [reload]);

  async function run(action: () => Promise<unknown>) {
    setError(null);
    try {
      await action();
      reload();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  const add = () =>
    run(async () => {
      if (!newName.trim()) return;
      await api.dishCategories.create(newName);
      setNewName("");
    });

  const saveRename = () =>
    run(async () => {
      if (!editing) return;
      await api.dishCategories.rename(editing.id, editing.name);
      setEditing(null);
    });

  function remove(c: DishCategory) {
    const used =
      c.item_count > 0
        ? ` ${c.item_count} ${c.item_count === 1 ? "dish is" : "dishes are"} filed under it; they stay, just uncategorised.`
        : "";
    if (!window.confirm(`Delete the “${c.name}” category?${used}`)) return;
    run(() => api.dishCategories.delete(c.id));
  }

  const field = "rounded border border-slate-300 px-2 py-1 text-sm";

  return (
    <section className={className}>
      <h2 className="font-semibold">Dish categories</h2>
      <p className="mb-3 text-xs text-slate-500">
        The choices in each dish’s <b>Category</b> box, shown as folding headings in the planner and Library
        lists. Add whatever helps you find things — Soups, Desserts, Slow cooker…
      </p>
      <ul className="mb-3 divide-y divide-slate-100">
        {cats.map((c) => (
          <li key={c.id} className="flex items-center gap-2 py-1.5 text-sm">
            {editing?.id === c.id ? (
              <>
                <input
                  autoFocus
                  value={editing.name}
                  onChange={(e) => setEditing({ id: c.id, name: e.target.value })}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") saveRename();
                    if (e.key === "Escape") setEditing(null);
                  }}
                  className={`${field} flex-1`}
                />
                <button onClick={saveRename} className="rounded bg-emerald-600 px-2 py-1 text-xs text-white">
                  Save
                </button>
                <button onClick={() => setEditing(null)} className="px-1 text-xs text-slate-500">
                  Cancel
                </button>
              </>
            ) : (
              <>
                <span className="flex-1">{c.name}</span>
                <span className="text-xs text-slate-400">
                  {c.item_count} {c.item_count === 1 ? "dish" : "dishes"}
                </span>
                <button
                  onClick={() => setEditing({ id: c.id, name: c.name })}
                  className="px-1 text-xs text-slate-500 hover:text-slate-900"
                >
                  Rename
                </button>
                <button onClick={() => remove(c)} className="px-1 text-xs text-slate-400 hover:text-red-600">
                  Delete
                </button>
              </>
            )}
          </li>
        ))}
        {cats.length === 0 && <li className="py-2 text-sm text-slate-400">No categories yet.</li>}
      </ul>
      <div className="flex gap-2">
        <input
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
          placeholder="New category, e.g. Soups"
          className={`${field} flex-1`}
        />
        <button onClick={add} className="rounded bg-emerald-600 px-3 py-1 text-sm font-medium text-white hover:bg-emerald-700">
          Add
        </button>
      </div>
      {error && <p className="mt-2 text-sm text-red-600">{error}</p>}
    </section>
  );
}
