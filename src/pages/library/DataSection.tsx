import { useEffect, useState } from "react";
import { api, errorMessage, type SampleCounts } from "../../lib/api";

type Pending = "sample" | "erase" | null;

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** Clear out the demo content, or wipe everything for a clean start. Both save a backup first. */
export default function DataSection() {
  const [counts, setCounts] = useState<SampleCounts | null>(null);
  const [pending, setPending] = useState<Pending>(null);
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  const refresh = () =>
    api.data
      .sampleCounts()
      .then(setCounts)
      .catch((e) => setMessage({ ok: false, text: errorMessage(e) }));

  useEffect(() => {
    refresh();
  }, []);

  function cancel() {
    setPending(null);
    setTyped("");
  }

  async function removeSample() {
    setBusy(true);
    try {
      const { result: r, backup_path } = await api.data.removeSample();
      const kept = r.ingredients_kept
        ? ` ${plural(r.ingredients_kept, "sample ingredient was", "sample ingredients were")} kept because your own meals use them.`
        : "";
      setMessage({
        ok: true,
        text:
          `Removed ${plural(r.items, "dish/portioned/side/sweet item", "dishes, portioned items, sides and sweets")}, ${plural(r.templates, "template", "templates")}, ` +
          `${plural(r.ingredients, "ingredient", "ingredients")} and ${plural(r.plan_entries, "planned meal", "planned meals")}.${kept} ` +
          `A backup was saved first: ${backup_path}`,
      });
    } catch (e) {
      setMessage({ ok: false, text: errorMessage(e) });
    } finally {
      setBusy(false);
      cancel();
      refresh();
    }
  }

  async function eraseAll() {
    setBusy(true);
    try {
      const { result: r, backup_path } = await api.data.eraseAll();
      setMessage({
        ok: true,
        text:
          `Erased ${plural(r.items, "item", "items")}, ${plural(r.ingredients, "ingredient", "ingredients")}, ` +
          `${plural(r.templates, "template", "templates")}, ${plural(r.plan_entries, "planned meal", "planned meals")} ` +
          `and ${plural(r.day_notes, "day note", "day notes")}. A backup was saved first: ${backup_path}`,
      });
    } catch (e) {
      setMessage({ ok: false, text: errorMessage(e) });
    } finally {
      setBusy(false);
      cancel();
      refresh();
    }
  }

  const nothingSample = counts !== null && counts.items + counts.ingredients + counts.templates === 0;

  return (
    <section className="rounded border border-slate-200 bg-white p-4">
      <h2 className="font-semibold">Clear data</h2>
      <p className="mb-3 text-xs text-slate-500">
        Start fresh. Both options save a backup file first, and you can bring it back with “Restore from
        backup…”.
      </p>

      <div className="space-y-4 text-sm">
        <div>
          <div className="font-medium">Remove sample data</div>
          <p className="mb-2 text-xs text-slate-500">
            {counts === null
              ? "Checking…"
              : nothingSample
                ? "No sample data left — everything here is yours."
                : `Deletes the demo content that came with the app: ${plural(counts.items, "dish/portioned/side/sweet item", "dishes, portioned items, sides and sweets")}, ${plural(counts.templates, "template", "templates")} and ${plural(counts.ingredients, "ingredient", "ingredients")}` +
                  (counts.planned_entries
                    ? `, plus ${plural(counts.planned_entries, "planned meal", "planned meals")} that use them`
                    : "") +
                  ". Anything you made yourself stays."}
          </p>
          {pending === "sample" ? (
            <div className="flex items-center gap-2">
              <button
                onClick={removeSample}
                disabled={busy}
                className="rounded bg-red-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-red-700 disabled:opacity-50"
              >
                Yes, remove sample data
              </button>
              <button onClick={cancel} className="rounded px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-100">
                Cancel
              </button>
            </div>
          ) : (
            <button
              onClick={() => setPending("sample")}
              disabled={busy || counts === null || nothingSample}
              className="rounded border border-slate-300 px-3 py-1.5 text-sm hover:bg-slate-50 disabled:opacity-50"
            >
              Remove sample data…
            </button>
          )}
        </div>

        <div className="border-t border-slate-100 pt-4">
          <div className="font-medium text-red-700">Erase everything</div>
          <p className="mb-2 text-xs text-slate-500">
            Deletes every meal, ingredient, template, planned meal and day note — including your own.
            Settings and the ingredient categories stay.
          </p>
          {pending === "erase" ? (
            <div className="flex flex-wrap items-center gap-2">
              <input
                autoFocus
                value={typed}
                onChange={(e) => setTyped(e.target.value)}
                placeholder="Type ERASE to confirm"
                className="w-48 rounded border border-slate-300 px-2 py-1.5 text-sm"
              />
              <button
                onClick={eraseAll}
                disabled={busy || typed.trim() !== "ERASE"}
                className="rounded bg-red-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-red-700 disabled:opacity-40"
              >
                Erase everything
              </button>
              <button onClick={cancel} className="rounded px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-100">
                Cancel
              </button>
            </div>
          ) : (
            <button
              onClick={() => setPending("erase")}
              disabled={busy}
              className="rounded border border-red-300 px-3 py-1.5 text-sm text-red-700 hover:bg-red-50 disabled:opacity-50"
            >
              Erase everything…
            </button>
          )}
        </div>
      </div>

      {message && (
        <p className={`mt-4 break-words text-sm ${message.ok ? "text-emerald-700" : "text-red-600"}`}>
          {message.text}
        </p>
      )}
    </section>
  );
}
