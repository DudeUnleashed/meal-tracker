import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorMessage, type Ingredient, type IngredientCategory } from "../lib/api";
import ItemEditor from "./ItemEditor";

interface Props {
  itemId: number;
  onClose: () => void;
  /** The item was saved, duplicated or archived — reload anything that shows it. */
  onChanged: () => void;
}

/** Slide-over item editor so you can fix a meal without leaving the planner. */
export default function ItemDrawer({ itemId: initialId, onClose, onChanged }: Props) {
  const [itemId, setItemId] = useState(initialId);
  const [ingredients, setIngredients] = useState<Ingredient[]>([]);
  const [categories, setCategories] = useState<IngredientCategory[]>([]);
  const [error, setError] = useState<string | null>(null);
  const dirty = useRef(false);

  useEffect(() => {
    Promise.all([api.ingredients.list(true), api.categories.list()])
      .then(([ings, cats]) => {
        setIngredients(ings);
        setCategories(cats);
      })
      .catch((e) => setError(errorMessage(e)));
  }, []);

  const handleDirty = useCallback((d: boolean) => {
    dirty.current = d;
  }, []);

  function close() {
    if (dirty.current && !window.confirm("Discard your unsaved changes?")) return;
    onClose();
  }

  return (
    <>
      <div className="fixed inset-0 z-40 bg-black/20" onClick={close} />
      <aside className="fixed inset-y-0 right-0 z-40 flex w-[34rem] max-w-full flex-col bg-white shadow-2xl">
        <header className="flex items-center justify-between border-b border-slate-200 px-4 py-3">
          <h2 className="font-semibold">Edit item</h2>
          <button
            onClick={close}
            aria-label="Close"
            className="rounded px-2 py-1 text-slate-500 hover:bg-slate-100"
          >
            ✕
          </button>
        </header>
        {error && <p className="px-4 py-2 text-sm text-red-600">{error}</p>}
        <div className="min-h-0 flex-1 bg-slate-50">
          <ItemEditor
            key={itemId}
            itemId={itemId}
            defaultType="meal"
            ingredients={ingredients}
            categories={categories}
            onIngredientCreated={(ing) => setIngredients((prev) => [...prev, ing])}
            onDirtyChange={handleDirty}
            onSaved={onChanged}
            onDuplicated={(copy) => {
              dirty.current = false;
              setItemId(copy.id);
              onChanged();
            }}
            onArchived={() => {
              onChanged();
              onClose();
            }}
          />
        </div>
      </aside>
    </>
  );
}
