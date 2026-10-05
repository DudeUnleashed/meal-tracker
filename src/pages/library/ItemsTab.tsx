import { useCallback, useEffect, useState } from "react";
import ItemEditor from "../../components/ItemEditor";
import {
  api,
  ITEM_TYPES,
  type Ingredient,
  type IngredientCategory,
  type ItemSummary,
  type ItemType,
} from "../../lib/api";

type Filter = ItemType | "all";
// null = nothing selected, "new" = creating, number = editing that item.
type Selection = number | "new" | null;

interface Props {
  ingredients: Ingredient[];
  categories: IngredientCategory[];
  onIngredientCreated: (ingredient: Ingredient) => void;
}

export default function ItemsTab({ ingredients, categories, onIngredientCreated }: Props) {
  const [filter, setFilter] = useState<Filter>("all");
  const [search, setSearch] = useState("");
  const [showArchived, setShowArchived] = useState(false);
  const [items, setItems] = useState<ItemSummary[]>([]);
  const [selection, setSelection] = useState<Selection>(null);
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      setItems(
        await api.items.list({
          type: filter === "all" ? undefined : filter,
          search,
          includeArchived: showArchived,
        }),
      );
    } catch (e) {
      setError(String(e));
    }
  }, [filter, search, showArchived]);

  useEffect(() => {
    reload();
  }, [reload]);

  /** Moves the selection, asking first if there are unsaved edits. */
  function select(next: Selection) {
    if (dirty && !window.confirm("Discard your unsaved changes?")) return;
    setDirty(false);
    setSelection(next);
  }

  const handleDirty = useCallback((d: boolean) => setDirty(d), []);

  return (
    <div className="flex h-full min-h-0">
      <aside className="flex w-72 flex-col border-r border-slate-200 bg-white">
        <div className="space-y-2 border-b border-slate-200 p-3">
          <div className="flex gap-1">
            {(["all", ...ITEM_TYPES] as Filter[]).map((f) => (
              <button
                key={f}
                onClick={() => setFilter(f)}
                className={`flex-1 rounded px-2 py-1 text-xs font-medium capitalize ${
                  filter === f
                    ? "bg-emerald-600 text-white"
                    : "bg-slate-100 text-slate-600 hover:bg-slate-200"
                }`}
              >
                {f}
              </button>
            ))}
          </div>
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search items…"
            className="w-full rounded border border-slate-300 px-2 py-1 text-sm"
          />
          <div className="flex items-center justify-between">
            <label className="flex items-center gap-1 text-xs text-slate-500">
              <input
                type="checkbox"
                checked={showArchived}
                onChange={(e) => setShowArchived(e.target.checked)}
              />
              Show archived
            </label>
            <button
              onClick={() => select("new")}
              className="rounded bg-emerald-600 px-2 py-1 text-xs font-medium text-white hover:bg-emerald-700"
            >
              + New item
            </button>
          </div>
        </div>
        <ul className="flex-1 overflow-auto">
          {items.map((it) => (
            <li key={it.id}>
              <button
                onClick={() => select(it.id)}
                className={`flex w-full items-center gap-2 px-3 py-2 text-left text-sm ${
                  selection === it.id ? "bg-emerald-50" : "hover:bg-slate-50"
                } ${it.archived ? "opacity-50" : ""}`}
              >
                <span
                  className="h-3 w-3 shrink-0 rounded-full border border-slate-300"
                  style={{ background: it.colour ?? "transparent" }}
                />
                <span className="flex-1 truncate">{it.name}</span>
                <span className="text-xs capitalize text-slate-400">{it.type}</span>
              </button>
            </li>
          ))}
          {items.length === 0 && (
            <li className="p-4 text-sm text-slate-400">{error ?? "No items found."}</li>
          )}
        </ul>
      </aside>

      <section className="min-w-0 flex-1 bg-slate-50">
        {selection === null ? (
          <p className="p-6 text-sm text-slate-500">
            Select an item to edit it, or create a new one.
          </p>
        ) : (
          <ItemEditor
            key={selection}
            itemId={selection === "new" ? null : selection}
            defaultType={filter === "all" ? "meal" : filter}
            ingredients={ingredients}
            categories={categories}
            onIngredientCreated={onIngredientCreated}
            onDirtyChange={handleDirty}
            onSaved={(item) => {
              setSelection(item.id);
              reload();
            }}
            onDuplicated={(item) => {
              setDirty(false);
              setSelection(item.id);
              reload();
            }}
            onArchived={(item) => {
              // Keep it selected if archived items are visible, otherwise it vanishes from the list.
              if (!showArchived && item.archived) setSelection(null);
              reload();
            }}
          />
        )}
      </section>
    </div>
  );
}
