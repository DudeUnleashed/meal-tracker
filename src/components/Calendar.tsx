import { format } from "date-fns";
import { useState } from "react";
import { daysOfWeek, fromIso, toIso, type WeekStart } from "../lib/dates";
import { blockRange, stepAnchor, type CalendarView } from "../lib/ranges";
import BlockView from "./calendar/BlockView";
import ScrollView from "./calendar/ScrollView";

export type { CalendarView };

export const VIEWS: { id: CalendarView; label: string }[] = [
  { id: "1w", label: "1 week" },
  { id: "2w", label: "2 weeks" },
  { id: "month", label: "Month" },
  { id: "scroll", label: "Scroll" },
];

interface Props {
  weekStart: WeekStart;
  view: CalendarView;
  onViewChange: (view: CalendarView) => void;
  /** Any date inside the block being shown; owned by the planner so the summary can follow it. */
  anchor: string;
  onAnchorChange: (anchor: string) => void;
  /** Bump to reload entries from the database (e.g. after an edit elsewhere). */
  refreshKey?: number;
  summaryOpen: boolean;
  onToggleSummary: () => void;
  /** Fill the block on screen from the block before it (1 and 2 week views). */
  onCopyPrevious: () => void;
}

export default function Calendar({
  weekStart,
  view,
  onViewChange,
  anchor,
  onAnchorChange,
  refreshKey = 0,
  summaryOpen,
  onToggleSummary,
  onCopyPrevious,
}: Props) {
  const [todayTick, setTodayTick] = useState(0);
  const block = blockRange(view, anchor, weekStart);

  function goToday() {
    onAnchorChange(toIso(new Date()));
    setTodayTick((n) => n + 1);
  }

  const dayNames = daysOfWeek(block.firstWeek).map((d) => format(fromIso(d), "EEE"));

  return (
    <div className="flex h-full min-w-0 flex-col">
      <div className="flex items-center gap-3 border-b border-slate-200 bg-white px-4 py-2">
        <button
          onClick={goToday}
          className="rounded border border-slate-300 px-3 py-1 text-sm font-medium hover:bg-slate-50"
        >
          Today
        </button>
        {view !== "scroll" && (
          <div className="flex items-center gap-1">
            <button
              onClick={() => onAnchorChange(stepAnchor(view, anchor, -1, weekStart))}
              aria-label="Previous"
              className="rounded px-2 py-1 text-slate-600 hover:bg-slate-100"
            >
              ‹
            </button>
            <button
              onClick={() => onAnchorChange(stepAnchor(view, anchor, 1, weekStart))}
              aria-label="Next"
              className="rounded px-2 py-1 text-slate-600 hover:bg-slate-100"
            >
              ›
            </button>
          </div>
        )}
        <span className="text-sm font-medium text-slate-700">
          {view === "scroll" ? format(new Date(), "EEEE d MMMM yyyy") : block.title}
        </span>

        <div className="ml-auto flex items-center gap-2">
          {(view === "1w" || view === "2w") && (
            <button
              onClick={onCopyPrevious}
              title={`Copy the dinners from the previous ${view === "1w" ? "week" : "2 weeks"} into this block`}
              className="rounded border border-slate-300 px-2.5 py-1 text-xs font-medium text-slate-600 hover:bg-slate-50"
            >
              Copy {view === "1w" ? "last week" : "last fortnight"}
            </button>
          )}
          <div className="flex gap-1 rounded bg-slate-100 p-0.5">
            {VIEWS.map((v) => (
              <button
                key={v.id}
                onClick={() => onViewChange(v.id)}
                className={`rounded px-2.5 py-1 text-xs font-medium ${
                  view === v.id ? "bg-white text-emerald-700 shadow-sm" : "text-slate-500 hover:text-slate-800"
                }`}
              >
                {v.label}
              </button>
            ))}
          </div>
          <button
            onClick={onToggleSummary}
            className={`rounded px-3 py-1 text-sm font-medium ${
              summaryOpen
                ? "bg-emerald-600 text-white hover:bg-emerald-700"
                : "border border-slate-300 hover:bg-slate-50"
            }`}
          >
            {summaryOpen ? "◂ Library" : "Summary ▸"}
          </button>
        </div>
      </div>

      <div className="grid grid-cols-7 border-b border-slate-200 bg-slate-100 text-center text-xs font-medium text-slate-500">
        {dayNames.map((n, i) => (
          <div key={i} className="py-1">
            {n}
          </div>
        ))}
      </div>

      <div className="min-h-0 flex-1">
        {view === "scroll" ? (
          <ScrollView weekStart={weekStart} refreshKey={refreshKey} todayTick={todayTick} />
        ) : (
          <BlockView
            firstWeek={block.firstWeek}
            weekCount={block.weekCount}
            month={block.month}
            weekStart={weekStart}
            refreshKey={refreshKey}
          />
        )}
      </div>
    </div>
  );
}
