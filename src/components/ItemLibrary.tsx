import { useDraggable } from "@dnd-kit/core";
import { format } from "date-fns";
import { useEffect, useMemo, useState } from "react";
import {
  api,
  errorMessage,
  ITEM_TYPES,
  type ItemSummary,
  type ItemType,
  type ItemUsage,
} from "../lib/api";
import { fromIso, toIso } from "../lib/dates";
import { lastHadText, plannedText } from "../lib/relative";
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
  const [tab, setTab] = useState<ItemType>("meal");
  const [search, setSearch] = useState("");
  const [sort, setSort] = useState<Sort>("name");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([api.items.list(), api.items.usage(toIso(new Date()))])
      .then(([list, usages]) => {
        setItems(list);
        setUsage(Object.fromEntries(usages.map((u) => [u.item_id, u])));
      })
      .catch((e) => setError(errorMessage(e)));
  }, [refreshKey]);

  // Searching looks across every type; otherwise show the selected tab.
  const q = search.trim().toLowerCase();
  const shown = useMemo(() => {
    const list = q
      ? items.filter((i) => i.name.toLowerCase().includes(q))
      : items.filter((i) => i.type === tab);
    if (sort === "name") return list;
    // Least recently had first; never-had items lead.
    return [...list].sort((a, b) => {
      const la = usage[a.id]?.last_had ?? "";
      const lb = usage[b.id]?.last_had ?? "";
      return la.localeCompare(lb) || a.name.localeCompare(b.name);
    });
  }, [items, usage, q, tab, sort]);

  return (
    <aside className="flex w-72 shrink-0 flex-col border-l border-slate-200 bg-white">
      <div className="space-y-2 border-b border-slate-200 p-3">
        <input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="🔍 Search all items…"
          className="w-full rounded border border-slate-300 px-2 py-1 text-sm"
        />
        <div className="flex gap-1">
          {ITEM_TYPES.map((t) => (
            <button
              key={t}
              onClick={() => {
                setTab(t);
                setSearch("");
              }}
              className={`flex-1 rounded px-2 py-1 text-xs font-medium capitalize ${
                !q && tab === t
                  ? "bg-emerald-600 text-white"
                  : "bg-slate-100 text-slate-600 hover:bg-slate-200"
              }`}
            >
              {t}
            </button>
          ))}
        </div>
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
          <LibraryCard key={item.id} item={item} usage={usage[item.id]} showType={q !== ""} />
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

function LibraryCard({
  item,
  usage,
  showType,
}: {
  item: ItemSummary;
  usage: ItemUsage | undefined;
  showType: boolean;
}) {
  const data: DragData = { kind: "item", itemId: item.id, name: item.name, colour: item.colour };
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
        {showType && <span className="text-xs capitalize text-slate-400">{item.type}</span>}
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
