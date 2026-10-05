import { useDraggable } from "@dnd-kit/core";
import { useEffect, useState } from "react";
import { api, errorMessage, ITEM_TYPES, type ItemSummary, type ItemType } from "../lib/api";
import type { DragData } from "./calendar/actions";

/** Sidebar of items to drag onto the calendar. */
export default function ItemLibrary() {
  const [items, setItems] = useState<ItemSummary[]>([]);
  const [tab, setTab] = useState<ItemType>("meal");
  const [search, setSearch] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.items
      .list()
      .then(setItems)
      .catch((e) => setError(errorMessage(e)));
  }, []);

  // Searching looks across every type; otherwise show the selected tab.
  const q = search.trim().toLowerCase();
  const shown = q
    ? items.filter((i) => i.name.toLowerCase().includes(q))
    : items.filter((i) => i.type === tab);

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
      </div>
      <ul className="flex-1 space-y-1.5 overflow-auto p-3">
        {shown.map((item) => (
          <LibraryCard key={item.id} item={item} showType={q !== ""} />
        ))}
        {shown.length === 0 && (
          <li className="text-sm text-slate-400">{error ?? "No items — add some in the Library."}</li>
        )}
      </ul>
      <p className="border-t border-slate-100 px-3 py-2 text-xs text-slate-400">
        Drag an item onto a day to plan it.
      </p>
    </aside>
  );
}

function LibraryCard({ item, showType }: { item: ItemSummary; showType: boolean }) {
  const data: DragData = { kind: "item", itemId: item.id, name: item.name, colour: item.colour };
  const { setNodeRef, listeners, attributes, isDragging } = useDraggable({
    id: `item:${item.id}`,
    data,
  });

  return (
    <li
      ref={setNodeRef}
      {...listeners}
      {...attributes}
      className={`flex touch-none cursor-grab items-center gap-2 rounded border border-slate-200 border-l-4 bg-white px-2 py-2 text-sm shadow-sm hover:bg-slate-50 ${
        isDragging ? "opacity-30" : ""
      }`}
      style={{ borderLeftColor: item.colour ?? "#94a3b8" }}
    >
      <span className="flex-1 truncate">{item.name}</span>
      {showType && <span className="text-xs capitalize text-slate-400">{item.type}</span>}
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
