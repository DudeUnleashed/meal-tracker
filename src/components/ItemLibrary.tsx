import { useDraggable } from "@dnd-kit/core";
import { format } from "date-fns";
import { useEffect, useMemo, useState } from "react";
import {
  api,
  errorMessage,
  typeLabel,
  type ItemSummary,
  type ItemUsage,
} from "../lib/api";
import { fromIso, toIso } from "../lib/dates";
import { lastHadText, plannedText } from "../lib/relative";
import { pickSuggestions } from "../lib/suggest";
import type { DragData } from "./calendar/actions";

type Sort = "name" | "least-recent";

interface Props {
  /** Bump to reload items and usage after the plan or an item changed. */
  refreshKey: number;
}

/** Sidebar of items to drag onto the calendar. */
export default function ItemLibrary({ refreshKey }: Props) {
  const [items, setItems] = useState<ItemSummary[]>([]);
  const [usage, setUsage] = useState<Record<number, ItemUsage>>({});
  const [search, setSearch] = useState("");
  const [sort, setSort] = useState<Sort>("name");
  const [shuffle, setShuffle] = useState(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([api.items.list(), api.items.usage(toIso(new Date()))])
      .then(([list, usages]) => {
        setItems(list);
        setUsage(Object.fromEntries(usages.map((u) => [u.item_id, u])));
      })
      .catch((e) => setError(errorMessage(e)));
  }, [refreshKey]);

  // One flat list of everything (each card carries a type badge); search narrows it.
  const q = search.trim().toLowerCase();
  const shown = useMemo(() => {
    const list = q ? items.filter((i) => i.name.toLowerCase().includes(q)) : items;
    if (sort === "name") return list;
    // Least recently had first; never-had items lead.
    return [...list].sort((a, b) => {
      const la = usage[a.id]?.last_had ?? "";
      const lb = usage[b.id]?.last_had ?? "";
      return la.localeCompare(lb) || a.name.localeCompare(b.name);
    });
  }, [items, usage, q, sort]);

  const suggestions = useMemo(() => pickSuggestions(items, usage, shuffle), [items, usage, shuffle]);

  return (
    <aside className="flex w-72 shrink-0 flex-col border-l border-slate-200 bg-white">
      <div className="space-y-2 border-b border-slate-200 bg-emerald-50/50 p-3">
        <div className="flex items-center justify-between">
          <h3 className="text-xs font-semibold uppercase tracking-wide text-emerald-800">
            Haven’t had in a while
          </h3>
          <button
            onClick={() => setShuffle((n) => n + 1)}
            title="Show a different suggestion"
            className="rounded px-1.5 py-0.5 text-xs text-emerald-800 hover:bg-emerald-100"
          >
            ↻ Another
          </button>
        </div>
        {(["meal", "meat", "side"] as const).map((t) => {
          const pick = suggestions[t];
          return pick ? (
            <SuggestionCard key={t} item={pick} usage={usage[pick.id]} />
          ) : null;
        })}
        {!suggestions.meal && !suggestions.meat && !suggestions.side && (
          <p className="text-xs text-slate-400">Nothing to suggest yet — add some items in the Library.</p>
        )}
        <div className="flex gap-2">
          <LeftoversTag />
          <BlankTag />
        </div>
      </div>
      <div className="space-y-2 border-b border-slate-200 p-3">
        <input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="🔍 Search items…"
          className="w-full rounded border border-slate-300 px-2 py-1 text-sm"
        />
        <label className="flex items-center gap-1 text-xs text-slate-500">
          <input
            type="checkbox"
            checked={sort === "least-recent"}
            onChange={(e) => setSort(e.target.checked ? "least-recent" : "name")}
          />
          Least recently had first
        </label>
      </div>
      <ul className="flex-1 space-y-1.5 overflow-auto p-3">
        {shown.map((item) => (
          <LibraryCard key={item.id} item={item} usage={usage[item.id]} />
        ))}
        {shown.length === 0 && (
          <li className="text-sm text-slate-400">{error ?? "No items — add some in the Library."}</li>
        )}
      </ul>
      <p className="border-t border-slate-100 px-3 py-2 text-xs text-slate-400">
        Drag an item onto a day to plan it. Click a planned item to add a note.
      </p>
    </aside>
  );
}

/** A compact, draggable suggestion: drop it on a day like any other card. */
function SuggestionCard({ item, usage }: { item: ItemSummary; usage: ItemUsage | undefined }) {
  const data: DragData = {
    kind: "item",
    itemId: item.id,
    name: item.name,
    colour: item.colour,
    template: item.live_template_id !== null,
  };
  const { setNodeRef, listeners, attributes, isDragging } = useDraggable({ id: `suggest:${item.id}`, data });
  const today = new Date();

  return (
    <div
      ref={setNodeRef}
      {...listeners}
      {...attributes}
      className={`touch-none cursor-grab rounded border border-emerald-200 border-l-4 bg-white px-2 py-1.5 text-sm shadow-sm hover:bg-emerald-50 ${
        isDragging ? "opacity-30" : ""
      }`}
      style={{ borderLeftColor: item.colour ?? "#94a3b8" }}
    >
      <div className="flex items-baseline gap-2">
        <span className="flex-1 truncate">{item.name}</span>
        <span className="text-xs text-slate-400">{typeLabel(item.type)}</span>
      </div>
      <div className="text-xs text-slate-400">
        {usage?.last_had ? `last had ${lastHadText(usage.last_had, today)}` : "not had yet"}
      </div>
    </div>
  );
}

/** The Leftovers tag: no ingredients, just a visible marker you drop on a day (e.g. the day after a big cook). */
function LeftoversTag() {
  const data: DragData = { kind: "leftovers" };
  const { setNodeRef, listeners, attributes, isDragging } = useDraggable({ id: "leftovers-tag", data });
  return (
    <div
      ref={setNodeRef}
      {...listeners}
      {...attributes}
      title="Drag onto a day to mark it as a leftovers night"
      className={`flex-1 touch-none cursor-grab rounded border border-dashed border-teal-400 bg-teal-50 px-2 py-1.5 text-sm text-teal-800 hover:bg-teal-100 ${
        isDragging ? "opacity-30" : ""
      }`}
    >
      🍲 Leftovers
    </div>
  );
}

/** The Blank token: drop it on a day, then say what it is (takeaway, dinner out…). */
function BlankTag() {
  const data: DragData = { kind: "blank" };
  const { setNodeRef, listeners, attributes, isDragging } = useDraggable({ id: "blank-tag", data });
  return (
    <div
      ref={setNodeRef}
      {...listeners}
      {...attributes}
      title="Drag onto a day for takeaway, eating out, or anything that isn't a recipe"
      className={`flex-1 touch-none cursor-grab rounded border border-dashed border-slate-400 bg-slate-100 px-2 py-1.5 text-sm text-slate-700 hover:bg-slate-200 ${
        isDragging ? "opacity-30" : ""
      }`}
    >
      🥡 Blank
    </div>
  );
}

export function BlankPreview() {
  return (
    <div className="w-40 cursor-grabbing rounded border border-dashed border-slate-400 bg-slate-100 px-2 py-1.5 text-sm text-slate-700 shadow-lg">
      🥡 Blank
    </div>
  );
}

export function LeftoversPreview() {
  return (
    <div className="w-40 cursor-grabbing rounded border border-dashed border-teal-400 bg-teal-50 px-2 py-1.5 text-sm text-teal-800 shadow-lg">
      🍲 Leftovers
    </div>
  );
}

function LibraryCard({
  item,
  usage,
}: {
  item: ItemSummary;
  usage: ItemUsage | undefined;
}) {
  const data: DragData = {
    kind: "item",
    itemId: item.id,
    name: item.name,
    colour: item.colour,
    template: item.live_template_id !== null,
  };
  const { setNodeRef, listeners, attributes, isDragging } = useDraggable({
    id: `item:${item.id}`,
    data,
  });

  const today = new Date();
  const status = [
    usage?.next_planned ? `planned ${plannedText(usage.next_planned, today)}` : null,
    usage?.last_had ? `last had ${lastHadText(usage.last_had, today)}` : null,
  ].filter(Boolean);

  return (
    <li
      ref={setNodeRef}
      {...listeners}
      {...attributes}
      className={`touch-none cursor-grab rounded border border-slate-200 border-l-4 bg-white px-2 py-2 text-sm shadow-sm hover:bg-slate-50 ${
        isDragging ? "opacity-30" : ""
      }`}
      style={{ borderLeftColor: item.colour ?? "#94a3b8" }}
    >
      <div className="flex items-center gap-2">
        <span className="flex-1 truncate">{item.name}</span>
        {item.live_template_id !== null && (
          <span title="Template: choose its ingredients per night" className="text-xs">
            🧩
          </span>
        )}
        <span className="rounded bg-slate-100 px-1.5 text-[10px] text-slate-500">{typeLabel(item.type)}</span>
      </div>
      <div className="text-xs text-slate-400">
        {status.length ? status.join(" · ") : "not had yet"}
      </div>
      {usage?.latest_note && (
        <div
          className="mt-1 truncate rounded bg-amber-50 px-1.5 py-0.5 text-xs text-amber-800"
          title={`${usage.latest_note_date ? format(fromIso(usage.latest_note_date), "d MMM yyyy") + ": " : ""}${usage.latest_note}`}
        >
          📝 {usage.latest_note}
        </div>
      )}
    </li>
  );
}

export function CardPreview({ name, colour }: { name: string; colour: string | null }) {
  return (
    <div
      className="w-56 cursor-grabbing rounded border border-slate-300 border-l-4 bg-white px-2 py-2 text-sm shadow-lg"
      style={{ borderLeftColor: colour ?? "#94a3b8" }}
    >
      {name}
    </div>
  );
}
