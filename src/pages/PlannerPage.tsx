import { useEffect, useState } from "react";
import Calendar from "../components/Calendar";
import { api, errorMessage } from "../lib/api";
import type { WeekStart } from "../lib/dates";

export default function PlannerPage() {
  // The calendar waits for settings so it builds its weeks around the right first day.
  const [weekStart, setWeekStart] = useState<WeekStart | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.settings
      .get()
      .then((s) => setWeekStart((Number(s.week_start ?? 1) % 7) as WeekStart))
      .catch((e) => setError(errorMessage(e)));
  }, []);

  return (
    <div className="flex h-full">
      <section className="min-w-0 flex-1">
        {weekStart === null ? (
          <p className="p-4 text-sm text-slate-500">{error ?? "Loading…"}</p>
        ) : (
          <Calendar key={weekStart} weekStart={weekStart} />
        )}
      </section>
      <aside className="w-72 border-l border-slate-200 bg-white p-4">
        <p className="text-slate-500">Item library goes here (phase 5).</p>
      </aside>
    </div>
  );
}
