import { useState } from "react";
import PlannerPage from "./pages/PlannerPage";
import LibraryPage from "./pages/LibraryPage";

type Page = "planner" | "library";

const PAGES: { id: Page; label: string }[] = [
  { id: "planner", label: "Planner" },
  { id: "library", label: "Library" },
];

export default function App() {
  const [page, setPage] = useState<Page>("planner");

  return (
    <div className="flex h-full flex-col bg-slate-50 text-slate-900">
      <header className="flex items-center gap-6 border-b border-slate-200 bg-white px-4">
        <h1 className="py-3 text-lg font-semibold">Meal Planner</h1>
        <nav className="flex gap-1">
          {PAGES.map((p) => (
            <button
              key={p.id}
              onClick={() => setPage(p.id)}
              className={`border-b-2 px-3 py-3 text-sm font-medium ${
                page === p.id
                  ? "border-emerald-600 text-emerald-700"
                  : "border-transparent text-slate-500 hover:text-slate-800"
              }`}
            >
              {p.label}
            </button>
          ))}
        </nav>
      </header>
      <main className="min-h-0 flex-1">
        {page === "planner" ? <PlannerPage /> : <LibraryPage />}
      </main>
    </div>
  );
}
