import { format } from "date-fns";
import { useEffect, useRef, useState } from "react";
import { api, errorMessage, type PlanEntry } from "../lib/api";
import { fromIso } from "../lib/dates";

const WIDTH = 288;

/** Quick sizes for a bigger (or smaller) night; the summary scales the ingredients by this. */
const SIZES = [0.5, 1, 1.5, 2, 3];
const SIZE_LABELS: Record<number, string> = { 0.5: "½×", 1: "1×", 1.5: "1.5×", 2: "2×", 3: "3×" };

interface Props {
  entry: PlanEntry;
  /** The clicked chip's on-screen rectangle; the popover opens next to it. */
  anchor: DOMRect;
  onClose: () => void;
  /** Something about the plan changed — reload the calendar. */
  onChanged: () => void;
  onEditItem: (itemId: number) => void;
}

/** Click a chip to adjust serves, leave a note for next time, remove it, or edit the item. */
export default function PlanEntryPopover({ entry: initial, anchor, onClose, onChanged, onEditItem }: Props) {
  const [entry, setEntry] = useState(initial);
  const [note, setNote] = useState(initial.note ?? "");
  const [error, setError] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);

  async function saveNote(): Promise<void> {
    if (note.trim() === (entry.note ?? "")) return;
    try {
      const updated = await api.plan.update(entry.id, { note });
      setEntry(updated);
      setNote(updated.note ?? "");
      onChanged();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function changeServes(serves: number) {
    try {
      setEntry(await api.plan.update(entry.id, { serves }));
      onChanged();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function changeScale(scale: number) {
    try {
      setEntry(await api.plan.update(entry.id, { scale }));
      onChanged();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function remove() {
    try {
      await api.plan.remove(entry.id);
      onChanged();
      onClose();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function closeAndSave() {
    await saveNote();
    onClose();
  }

  // Close on outside click / Escape, saving the note first. Refs keep the handlers current.
  const latest = useRef(closeAndSave);
  latest.current = closeAndSave;
  useEffect(() => {
    const outside = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) latest.current();
    };
    const escape = (e: KeyboardEvent) => e.key === "Escape" && latest.current();
    document.addEventListener("mousedown", outside);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("mousedown", outside);
      document.removeEventListener("keydown", escape);
    };
  }, []);

  // Below the chip, clamped to the window; flipped above when there is no room.
  const left = Math.max(8, Math.min(anchor.left, window.innerWidth - WIDTH - 8));
  const below = anchor.bottom + 6;
  const top = below + 330 > window.innerHeight ? Math.max(8, anchor.top - 336) : below;

  return (
    <div
      ref={root}
      style={{ left, top, width: WIDTH }}
      className="fixed z-50 space-y-3 rounded-lg border border-slate-200 bg-white p-3 text-sm shadow-xl"
    >
      <div>
        <div className="flex items-center gap-2">
          <span
            className="h-3 w-3 shrink-0 rounded-full"
            style={{ background: entry.item_colour ?? "#94a3b8" }}
          />
          <span className="flex-1 truncate font-semibold">{entry.item_name}</span>
          <span className="text-xs capitalize text-slate-400">{entry.item_type}</span>
        </div>
        <div className="text-xs text-slate-500">{format(fromIso(entry.date), "EEEE d MMMM")}</div>
      </div>

      {entry.serves !== null && (
        <div className="flex items-center gap-2">
          <span className="text-xs text-slate-500">Serves</span>
          <button
            onClick={() => changeServes(entry.serves! - 1)}
            disabled={entry.serves <= 1}
            aria-label="Fewer serves"
            className="h-6 w-6 rounded bg-slate-100 hover:bg-slate-200 disabled:opacity-40"
          >
            −
          </button>
          <span className="w-6 text-center font-medium">{entry.serves}</span>
          <button
            onClick={() => changeServes(entry.serves! + 1)}
            aria-label="More serves"
            className="h-6 w-6 rounded bg-slate-100 hover:bg-slate-200"
          >
            +
          </button>
        </div>
      )}

      {entry.item_type !== "meat" && (
        <div>
          <div className="mb-1 text-xs text-slate-500">
            Size <span className="text-slate-400">— 1 is the usual one night’s amount</span>
          </div>
          <div className="flex gap-1">
            {SIZES.map((n) => (
              <button
                key={n}
                onClick={() => changeScale(n)}
                aria-pressed={Math.abs(entry.scale - n) < 1e-9}
                className={`flex-1 rounded border px-1 py-1 text-xs ${
                  Math.abs(entry.scale - n) < 1e-9
                    ? "border-sky-400 bg-sky-50 font-semibold text-sky-800"
                    : "border-slate-300 text-slate-600 hover:bg-slate-50"
                }`}
              >
                {SIZE_LABELS[n]}
              </button>
            ))}
          </div>
        </div>
      )}

      <label className="block text-xs text-slate-500">
        Note
        <textarea
          autoFocus
          value={note}
          onChange={(e) => setNote(e.target.value)}
          onBlur={saveNote}
          rows={3}
          placeholder="e.g. try a different sauce next time"
          className="mt-1 w-full rounded border border-slate-300 px-2 py-1 text-sm text-slate-900"
        />
        <span className="text-slate-400">Shown as a reminder next time this is planned.</span>
      </label>

      {error && <p className="text-xs text-red-600">{error}</p>}

      <div className="flex items-center gap-2">
        <button onClick={remove} className="rounded px-2 py-1 text-xs text-red-600 hover:bg-red-50">
          Remove
        </button>
        <button
          onClick={async () => {
            await saveNote();
            onEditItem(entry.item_id);
            onClose();
          }}
          className="rounded px-2 py-1 text-xs text-emerald-700 hover:bg-emerald-50"
        >
          Edit item
        </button>
        <button
          onClick={closeAndSave}
          className="ml-auto rounded bg-slate-100 px-3 py-1 text-xs font-medium hover:bg-slate-200"
        >
          Done
        </button>
      </div>
    </div>
  );
}
