import { format } from "date-fns";
import { useEffect, useState } from "react";
import { api, errorMessage, type Summary } from "../lib/api";
import { fromIso } from "../lib/dates";
import { formatQuantity, groupByCategory, summaryToText } from "../lib/format";
import { rangeTitle } from "../lib/ranges";

export type SummaryMode = "1w" | "2w" | "month" | "custom";

const MODES: { id: SummaryMode; label: string }[] = [
  { id: "1w", label: "1 wk" },
  { id: "2w", label: "2 wk" },
  { id: "month", label: "Month" },
  { id: "custom", label: "Custom" },
];

interface Props {
  start: string;
  end: string;
  mode: SummaryMode;
  onMode: (mode: SummaryMode) => void;
  onStep: (direction: -1 | 1) => void;
  onCustomRange: (start: string, end: string) => void;
  /** Bump when the plan changes so the totals reload. */
  refreshKey: number;
}

/** Shopping summary for the date range, with "have it" ticks that live only while it's open. */
export default function SummaryPanel({
  start,
  end,
  mode,
  onMode,
  onStep,
  onCustomRange,
  refreshKey,
}: Props) {
  const [summary, setSummary] = useState<Summary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [have, setHave] = useState<Set<number>>(new Set());
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  const [copied, setCopied] = useState(false);

  const invalidRange = end < start;

  // Ticks are per range: changing the range starts a fresh shop.
  useEffect(() => {
    setHave(new Set());
    setExpanded(new Set());
  }, [start, end]);

  useEffect(() => {
    if (invalidRange) return;
    let cancelled = false;
    api.summary
      .get(start, end)
      .then((s) => {
        if (cancelled) return;
        setSummary(s);
        setError(null);
      })
      .catch((e) => !cancelled && setError(errorMessage(e)));
    return () => {
      cancelled = true;
    };
  }, [start, end, refreshKey, invalidRange]);

  const toggle = (set: Set<number>, id: number): Set<number> => {
    const next = new Set(set);
    if (!next.delete(id)) next.add(id);
    return next;
  };

  const title = rangeTitle(start, end);

  async function copy() {
    if (!summary) return;
    const text = summaryToText(summary, title, have);
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // Clipboard API unavailable: fall back to a temporary textarea.
      const area = document.createElement("textarea");
      area.value = text;
      document.body.appendChild(area);
      area.select();
      document.execCommand("copy");
      area.remove();
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  const groups = summary ? groupByCategory(summary.rows) : [];
  const remaining = summary ? summary.rows.filter((r) => !have.has(r.ingredient_id)).length : 0;

  return (
    <aside className="flex w-80 shrink-0 flex-col border-l border-slate-200 bg-white xl:w-96">
      <div className="space-y-2 border-b border-slate-200 p-3">
        <h2 className="font-semibold">Shopping summary</h2>
        <div className="flex gap-1 rounded bg-slate-100 p-0.5">
          {MODES.map((m) => (
            <button
              key={m.id}
              onClick={() => onMode(m.id)}
              className={`flex-1 rounded px-2 py-1 text-xs font-medium ${
                mode === m.id ? "bg-white text-emerald-700 shadow-sm" : "text-slate-500 hover:text-slate-800"
              }`}
            >
              {m.label}
            </button>
          ))}
        </div>
        {mode === "custom" ? (
          <div className="flex items-center gap-1 text-sm">
            <button
              onClick={() => onStep(-1)}
              aria-label="Previous range"
              className="rounded px-2 py-1 text-slate-600 hover:bg-slate-100"
            >
              ‹
            </button>
            <input
              type="date"
              value={start}
              onChange={(e) => e.target.value && onCustomRange(e.target.value, end)}
              className="min-w-0 flex-1 rounded border border-slate-300 px-1 py-0.5 text-xs"
            />
            <span className="text-slate-400">to</span>
            <input
              type="date"
              value={end}
              onChange={(e) => e.target.value && onCustomRange(start, e.target.value)}
              className="min-w-0 flex-1 rounded border border-slate-300 px-1 py-0.5 text-xs"
            />
            <button
              onClick={() => onStep(1)}
              aria-label="Next range"
              className="rounded px-2 py-1 text-slate-600 hover:bg-slate-100"
            >
              ›
            </button>
          </div>
        ) : (
          <div className="flex items-center gap-1 text-sm">
            <button
              onClick={() => onStep(-1)}
              aria-label="Previous range"
              className="rounded px-2 py-1 text-slate-600 hover:bg-slate-100"
            >
              ‹
            </button>
            <span className="flex-1 text-center font-medium text-slate-700">{title}</span>
            <button
              onClick={() => onStep(1)}
              aria-label="Next range"
              className="rounded px-2 py-1 text-slate-600 hover:bg-slate-100"
            >
              ›
            </button>
          </div>
        )}
        {summary && !invalidRange && (
          <p className="text-xs text-slate-500">
            {summary.dinner_days} {summary.dinner_days === 1 ? "day" : "days"} planned,{" "}
            {summary.entry_count} {summary.entry_count === 1 ? "item" : "items"}
            {summary.rows.length > 0 && ` · ${remaining} of ${summary.rows.length} still to buy`}
          </p>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        {invalidRange && <p className="p-4 text-sm text-red-600">The start date must be on or before the end date.</p>}
        {error && !invalidRange && <p className="p-4 text-sm text-red-600">{error}</p>}
        {summary && !invalidRange && summary.rows.length === 0 && (
          <p className="p-4 text-sm text-slate-400">
            {summary.entry_count === 0
              ? "Nothing planned in this range."
              : "The planned items have no ingredients."}
          </p>
        )}
        {!invalidRange &&
          groups.map((g) => (
            <section key={g.category}>
              <h3 className="sticky top-0 bg-slate-50 px-3 py-1 text-xs font-semibold tracking-wide text-slate-500">
                {g.category.toUpperCase()}
              </h3>
              <ul>
                {g.rows.map((r) => {
                  const ticked = have.has(r.ingredient_id);
                  const open = expanded.has(r.ingredient_id);
                  return (
                    <li key={r.ingredient_id} className="border-b border-slate-100">
                      <div className="flex items-start gap-2 px-3 py-1.5">
                        <input
                          type="checkbox"
                          checked={ticked}
                          onChange={() => setHave(toggle(have, r.ingredient_id))}
                          aria-label={`Already have ${r.ingredient_name}`}
                          className="mt-1"
                        />
                        <button
                          onClick={() => setExpanded(toggle(expanded, r.ingredient_id))}
                          aria-expanded={open}
                          className={`min-w-0 flex-1 text-left ${ticked ? "text-slate-400 line-through" : ""}`}
                        >
                          <span className="flex items-baseline justify-between gap-2 text-sm">
                            <span>{r.ingredient_name}</span>
                            <span className="font-medium tabular-nums">
                              {formatQuantity(r.quantity, r.unit)}
                            </span>
                          </span>
                          {!open && (
                            <span className="block truncate text-xs text-slate-400">{sourceHint(r.sources)}</span>
                          )}
                        </button>
                      </div>
                      {open && (
                        <ul className="mb-1.5 ml-9 mr-3 space-y-0.5 text-xs text-slate-500">
                          {r.sources.map((s, i) => (
                            <li key={i} className="flex justify-between gap-2">
                              <span className="truncate">
                                {s.item_name}
                                {s.serves !== null && ` ×${s.serves}`}
                                {s.scale !== null && ` ×${Number(s.scale.toFixed(2))}`}
                                <span className="text-slate-400"> · {format(fromIso(s.date), "EEE d MMM")}</span>
                              </span>
                              <span className="tabular-nums">{formatQuantity(s.quantity, r.unit)}</span>
                            </li>
                          ))}
                        </ul>
                      )}
                    </li>
                  );
                })}
              </ul>
            </section>
          ))}
      </div>

      <div className="flex items-center gap-2 border-t border-slate-200 p-3">
        <button
          onClick={copy}
          disabled={!summary || summary.rows.length === 0}
          className="rounded bg-emerald-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-emerald-700 disabled:opacity-50"
        >
          {copied ? "Copied ✓" : "Copy as text"}
        </button>
        {have.size > 0 && (
          <button
            onClick={() => setHave(new Set())}
            className="rounded px-2 py-1.5 text-sm text-slate-500 hover:bg-slate-100"
          >
            Clear ticks
          </button>
        )}
        <span className="ml-auto text-xs text-slate-400">Ticked items are left out</span>
      </div>
    </aside>
  );
}

/** "Steak ×3, Spag bol (2×)" — which planned items contribute, collapsed to one line. */
function sourceHint(sources: Summary["rows"][number]["sources"]): string {
  const counts = new Map<string, number>();
  for (const s of sources) {
    const label =
      s.serves !== null
        ? `${s.item_name} ×${s.serves}`
        : s.scale !== null
          ? `${s.item_name} ×${Number(s.scale.toFixed(2))}`
          : s.item_name;
    counts.set(label, (counts.get(label) ?? 0) + 1);
  }
  return [...counts].map(([label, n]) => (n > 1 ? `${label} (${n}×)` : label)).join(", ");
}
