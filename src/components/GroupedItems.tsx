import { useState, type ReactNode } from "react";
import type { ItemSummary } from "../lib/api";
import { groupItems } from "../lib/groups";

interface Props {
  items: ItemSummary[];
  /** false = a plain flat list (used while searching, so results are never hidden in a fold). */
  grouped: boolean;
  /** Where the open/closed folds are remembered. */
  storageKey: string;
  /** Renders one item as an `<li>`. `scope` keeps ids unique when a dish shows in two groups. */
  renderItem: (item: ItemSummary, scope: string) => ReactNode;
}

function loadOpen(key: string): number[] {
  try {
    const raw = JSON.parse(localStorage.getItem(`groups-open:${key}`) ?? "[]");
    return Array.isArray(raw) ? raw.filter((n) => typeof n === "number") : [];
  } catch {
    return [];
  }
}

/** Items under collapsible ingredient headings (ticked by hand on the recipe), then the rest. */
export default function GroupedItems({ items, grouped, storageKey, renderItem }: Props) {
  const [open, setOpen] = useState<number[]>(() => loadOpen(storageKey));

  function toggle(id: number) {
    const next = open.includes(id) ? open.filter((x) => x !== id) : [...open, id];
    setOpen(next);
    try {
      localStorage.setItem(`groups-open:${storageKey}`, JSON.stringify(next));
    } catch {
      /* remembering folds is a convenience only */
    }
  }

  if (!grouped) return <>{items.map((i) => renderItem(i, "flat"))}</>;

  const { groups, rest } = groupItems(items);
  return (
    <>
      {groups.map((g) => {
        const isOpen = open.includes(g.ingredient_id);
        return (
          <li key={`group:${g.ingredient_id}`} className="list-none">
            <button
              type="button"
              onClick={() => toggle(g.ingredient_id)}
              aria-expanded={isOpen}
              className="flex w-full items-center gap-1.5 rounded bg-slate-100 px-2 py-1.5 text-left text-sm font-medium text-slate-700 hover:bg-slate-200"
            >
              <span className="w-3 text-xs text-slate-500">{isOpen ? "▾" : "▸"}</span>
              <span className="flex-1 truncate">{g.name}</span>
              <span className="text-xs font-normal text-slate-500">{g.items.length}</span>
            </button>
            {isOpen && (
              <ul className="mt-1.5 space-y-1.5 border-l-2 border-slate-200 pl-2">
                {g.items.map((i) => renderItem(i, `g${g.ingredient_id}`))}
              </ul>
            )}
          </li>
        );
      })}
      {groups.length > 0 && rest.length > 0 && (
        <li className="list-none px-1 pt-1 text-[10px] font-semibold uppercase tracking-wide text-slate-400">
          Everything else
        </li>
      )}
      {rest.map((i) => renderItem(i, "rest"))}
    </>
  );
}
