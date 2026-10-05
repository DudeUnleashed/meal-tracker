import { open, save } from "@tauri-apps/plugin-dialog";
import { format } from "date-fns";
import { useEffect, useState } from "react";
import { api, errorMessage, type IngredientCategory } from "../../lib/api";
import DataSection from "./DataSection";

const DAYS = [
  { value: "1", label: "Monday" },
  { value: "2", label: "Tuesday" },
  { value: "3", label: "Wednesday" },
  { value: "4", label: "Thursday" },
  { value: "5", label: "Friday" },
  { value: "6", label: "Saturday" },
  { value: "0", label: "Sunday" },
];

const JSON_FILTER = [{ name: "Meal Planner backup", extensions: ["json"] }];

export default function SettingsTab() {
  const [serves, setServes] = useState("3");
  const [weekStart, setWeekStart] = useState("1");
  const [categories, setCategories] = useState<IngredientCategory[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [backupMessage, setBackupMessage] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    Promise.all([api.settings.get(), api.categories.list()])
      .then(([s, cats]) => {
        setServes(s.default_meat_serves ?? "3");
        setWeekStart(s.week_start ?? "1");
        setCategories(cats);
        setLoaded(true);
      })
      .catch((e) => setError(errorMessage(e)));
  }, []);

  async function saveSetting(key: string, value: string, label: string) {
    setError(null);
    try {
      await api.settings.set(key, value);
      setSaved(`${label} saved`);
      setTimeout(() => setSaved(null), 1500);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function move(index: number, delta: -1 | 1) {
    const ids = categories.map((c) => c.id);
    const target = index + delta;
    if (target < 0 || target >= ids.length) return;
    [ids[index], ids[target]] = [ids[target], ids[index]];
    try {
      setCategories(await api.categories.reorder(ids));
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function exportBackup() {
    setBackupMessage(null);
    try {
      const path = await save({
        title: "Save backup",
        defaultPath: `meal-planner-backup-${format(new Date(), "yyyy-MM-dd")}.json`,
        filters: JSON_FILTER,
      });
      if (!path) return; // cancelled
      await api.backup.export(path);
      setBackupMessage({ ok: true, text: `Backup saved to ${path}` });
    } catch (e) {
      setBackupMessage({ ok: false, text: errorMessage(e) });
    }
  }

  async function importBackup() {
    setBackupMessage(null);
    try {
      const path = await open({ title: "Choose a backup to restore", multiple: false, filters: JSON_FILTER });
      if (!path || Array.isArray(path)) return; // cancelled
      if (
        !window.confirm(
          "Restoring replaces EVERYTHING in the app (items, ingredients, templates, the whole plan and settings) with the contents of this file.\n\nThis can't be undone — export a backup first if you might want the current data.\n\nRestore now?",
        )
      )
        return;
      const r = await api.backup.import(path);
      setBackupMessage({
        ok: true,
        text: `Restored ${r.items} items, ${r.ingredients} ingredients, ${r.templates} templates and ${r.plan_entries} planned dinners. Reloading…`,
      });
      // Every screen cached the old data; start fresh.
      setTimeout(() => window.location.reload(), 1200);
    } catch (e) {
      setBackupMessage({ ok: false, text: errorMessage(e) });
    }
  }

  if (!loaded && !error) return <p className="p-6 text-sm text-slate-500">Loading…</p>;

  const card = "rounded border border-slate-200 bg-white p-4";
  const field = "rounded border border-slate-300 px-2 py-1 text-sm";

  return (
    <div className="h-full overflow-auto bg-slate-50 p-6">
      <div className="mx-auto max-w-2xl space-y-5">
        {error && <p className="text-sm text-red-600">{error}</p>}

        <section className={card}>
          <h2 className="mb-3 flex items-baseline gap-3 font-semibold">
            Planner
            {saved && <span className="text-xs font-normal text-emerald-700">{saved} ✓</span>}
          </h2>
          <div className="space-y-3 text-sm">
            <label className="flex items-center justify-between gap-4">
              <span>
                Default meat serves
                <span className="block text-xs text-slate-500">
                  Serves a meat gets when you drop it on the calendar (adjust per night).
                </span>
              </span>
              <input
                type="number"
                min="1"
                step="1"
                value={serves}
                onChange={(e) => setServes(e.target.value)}
                onBlur={() => saveSetting("default_meat_serves", serves, "Default serves")}
                className={`${field} w-20 text-right`}
              />
            </label>
            <label className="flex items-center justify-between gap-4">
              <span>
                First day of the week
                <span className="block text-xs text-slate-500">Where each calendar week starts.</span>
              </span>
              <select
                value={weekStart}
                onChange={(e) => {
                  setWeekStart(e.target.value);
                  saveSetting("week_start", e.target.value, "First day of week");
                }}
                className={field}
              >
                {DAYS.map((d) => (
                  <option key={d.value} value={d.value}>
                    {d.label}
                  </option>
                ))}
              </select>
            </label>
          </div>
        </section>

        <section className={card}>
          <h2 className="font-semibold">Shopping list order</h2>
          <p className="mb-3 text-xs text-slate-500">
            The order ingredient categories appear in the shopping summary and copied lists — e.g. the
            order you walk round the shop.
          </p>
          <ol className="divide-y divide-slate-100 rounded border border-slate-200 text-sm">
            {categories.map((c, i) => (
              <li key={c.id} className="flex items-center gap-2 px-3 py-1.5">
                <span className="w-5 text-xs text-slate-400">{i + 1}</span>
                <span className="flex-1">{c.name}</span>
                <button
                  onClick={() => move(i, -1)}
                  disabled={i === 0}
                  aria-label={`Move ${c.name} up`}
                  className="px-1 text-slate-400 hover:text-slate-800 disabled:opacity-30"
                >
                  ↑
                </button>
                <button
                  onClick={() => move(i, 1)}
                  disabled={i === categories.length - 1}
                  aria-label={`Move ${c.name} down`}
                  className="px-1 text-slate-400 hover:text-slate-800 disabled:opacity-30"
                >
                  ↓
                </button>
              </li>
            ))}
          </ol>
        </section>

        <section className={card}>
          <h2 className="font-semibold">Backup &amp; restore</h2>
          <p className="mb-3 text-xs text-slate-500">
            Everything stays on this computer. Export a backup file to keep a copy or move to another
            device; restoring replaces all current data with the file’s.
          </p>
          <div className="flex gap-2">
            <button
              onClick={exportBackup}
              className="rounded bg-emerald-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-emerald-700"
            >
              Export backup…
            </button>
            <button
              onClick={importBackup}
              className="rounded border border-slate-300 px-3 py-1.5 text-sm hover:bg-slate-50"
            >
              Restore from backup…
            </button>
          </div>
          {backupMessage && (
            <p className={`mt-3 text-sm ${backupMessage.ok ? "text-emerald-700" : "text-red-600"}`}>
              {backupMessage.text}
            </p>
          )}
        </section>

        <DataSection />
      </div>
    </div>
  );
}
