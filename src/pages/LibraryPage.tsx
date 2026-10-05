import { useCallback, useEffect, useState } from "react";
import { api, errorMessage, type Ingredient, type IngredientCategory } from "../lib/api";
import IngredientsTab from "./library/IngredientsTab";
import ItemsTab from "./library/ItemsTab";

const TABS = ["Items", "Templates", "Ingredients", "Settings"] as const;
type Tab = (typeof TABS)[number];

export default function LibraryPage() {
  const [tab, setTab] = useState<Tab>("Items");
  // Archived ingredients are included so existing items can still display them.
  const [ingredients, setIngredients] = useState<Ingredient[]>([]);
  const [categories, setCategories] = useState<IngredientCategory[]>([]);
  const [error, setError] = useState<string | null>(null);

  // Reload on tab change so "used in" counts reflect item edits.
  useEffect(() => {
    Promise.all([api.ingredients.list(true), api.categories.list()])
      .then(([ings, cats]) => {
        setIngredients(ings);
        setCategories(cats);
      })
      .catch((e) => setError(errorMessage(e)));
  }, [tab]);

  /** Insert or replace one ingredient in local state after a create/update/archive. */
  const upsertIngredient = useCallback((saved: Ingredient) => {
    setIngredients((prev) => {
      const exists = prev.some((i) => i.id === saved.id);
      const next = exists ? prev.map((i) => (i.id === saved.id ? saved : i)) : [...prev, saved];
      return next.sort(
        (a, b) =>
          (categories.find((c) => c.id === a.category_id)?.sort_order ?? 0) -
            (categories.find((c) => c.id === b.category_id)?.sort_order ?? 0) ||
          a.name.localeCompare(b.name),
      );
    });
  }, [categories]);

  return (
    <div className="flex h-full flex-col">
      <div className="flex gap-1 border-b border-slate-200 bg-white px-4">
        {TABS.map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`border-b-2 px-3 py-2 text-sm font-medium ${
              tab === t
                ? "border-emerald-600 text-emerald-700"
                : "border-transparent text-slate-500 hover:text-slate-800"
            }`}
          >
            {t}
          </button>
        ))}
      </div>
      {error && <p className="p-4 text-sm text-red-600">Database error: {error}</p>}
      <div className="min-h-0 flex-1">
        {tab === "Items" && (
          <ItemsTab
            ingredients={ingredients}
            categories={categories}
            onIngredientCreated={upsertIngredient}
          />
        )}
        {tab === "Ingredients" && (
          <IngredientsTab
            ingredients={ingredients}
            categories={categories}
            onChanged={upsertIngredient}
          />
        )}
        {(tab === "Templates" || tab === "Settings") && (
          <p className="p-6 text-sm text-slate-500">{tab} tab (coming in a later phase).</p>
        )}
      </div>
    </div>
  );
}
