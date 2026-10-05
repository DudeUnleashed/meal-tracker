import { format } from "date-fns";
import { useEffect, useRef, useState } from "react";
import { api, errorMessage, type DayInfo } from "../lib/api";
import { fromIso } from "../lib/dates";

const WIDTH = 288;

interface Props {
  date: string;
  /** The clicked cell / note's rectangle; the popover opens next to it. */
  anchor: DOMRect;
  /** Current info for the day, if it has any. */
  info?: DayInfo;
  onClose: () => void;
  /** Something changed — reload the calendar. */
  onChanged: () => void;
}

/** A note for the day itself ("out for Sam's birthday", "Alex working late") and the leftovers tag. */
export default function DayNotePopover({ date, anchor, info, onClose, onChanged }: Props) {
  const [saved, setSaved] = useState(info?.note ?? "");
  const [note, setNote] = useState(info?.note ?? "");
  const [leftovers, setLeftovers] = useState(info?.leftovers ?? false);
  const [error, setError] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);

  async function saveNote(): Promise<void> {
    if (note.trim() === saved) return;
    try {
      const result = await api.dayInfo.set(date, { note });
      setSaved(result?.note ?? "");
      setNote(result?.note ?? "");
      onChanged();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function toggleLeftovers() {
    try {
      await api.dayInfo.set(date, { leftovers: !leftovers });
      setLeftovers(!leftovers);
      onChanged();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function closeAndSave() {
    await saveNote();
    onClose();
  }

  // Close on outside click / Escape, saving the note first.
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

  const left = Math.max(8, Math.min(anchor.left, window.innerWidth - WIDTH - 8));
  const below = anchor.bottom + 6;
  const top = below + 230 > window.innerHeight ? Math.max(8, anchor.top - 236) : below;

  return (
    <div
      ref={root}
      style={{ left, top, width: WIDTH }}
      className="fixed z-50 space-y-3 rounded-lg border border-slate-200 bg-white p-3 text-sm shadow-xl"
    >
      <div className="font-semibold">{format(fromIso(date), "EEEE d MMMM")}</div>

      <label className="block text-xs text-slate-500">
        Note for the day
        <textarea
          autoFocus
          value={note}
          onChange={(e) => setNote(e.target.value)}
          onBlur={saveNote}
          rows={3}
          placeholder="e.g. out for Sam's birthday, or Alex working late so more leftovers"
          className="mt-1 w-full rounded border border-slate-300 px-2 py-1 text-sm text-slate-900"
        />
      </label>

      <button
        type="button"
        onClick={toggleLeftovers}
        className={`w-full rounded border px-2 py-1 text-left text-xs ${
          leftovers
            ? "border-teal-400 bg-teal-50 text-teal-800"
            : "border-slate-300 text-slate-600 hover:bg-slate-50"
        }`}
      >
        🍲 {leftovers ? "Leftovers night — click to remove" : "Mark as a leftovers night"}
      </button>

      {error && <p className="text-xs text-red-600">{error}</p>}

      <div className="flex">
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
