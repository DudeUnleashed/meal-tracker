import { useEffect, useState } from "react";
import { api, type IngredientCategory } from "../lib/api";

const TABS = ["Items", "Templates", "Ingredients", "Settings"] as const;

export default function LibraryPage() {
  const [tab, setTab] = useState<(typeof TABS)[number]>("Items");
  const [categories, setCategories] = useState<IngredientCategory[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Smoke test that the Rust/SQLite backend is reachable and seeded.
  useEffect(() => {
    api.categories
      .list()
      .then(setCategories)
      .catch((e) => setError(String(e)));
  }, []);

  return (
    <div className="flex h-full flex-col p-4">
      <div className="flex gap-1 border-b border-slate-200">
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
      <div className="flex-1 py-4 text-sm text-slate-500">
        <p>{tab} tab (coming in a later phase).</p>
        <p className="mt-4">
          {error
            ? `Database error: ${error}`
            : categories
              ? `Database connected. Categories: ${categories.map((c) => c.name).join(", ")}`
              : "Connecting to database…"}
        </p>
      </div>
    </div>
  );
}
