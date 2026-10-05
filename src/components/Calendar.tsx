import {
  addMonths,
  differenceInCalendarWeeks,
  endOfMonth,
  format,
  startOfMonth,
} from "date-fns";
import { useState } from "react";
import { daysOfWeek, fromIso, shiftWeeks, toIso, weekStartOf, type WeekStart } from "../lib/dates";
import BlockView from "./calendar/BlockView";
import ScrollView from "./calendar/ScrollView";

export type CalendarView = "1w" | "2w" | "month" | "scroll";

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
  /** Bump to reload entries from the database (e.g. after an edit elsewhere). */
  refreshKey?: number;
}

export default function Calendar({ weekStart, view, onViewChange, refreshKey = 0 }: Props) {
  // Any date inside the block being shown; switching views keeps you in the same place.
  const [anchor, setAnchor] = useState(() => toIso(new Date()));
  const [todayTick, setTodayTick] = useState(0);

  const anchorDate = fromIso(anchor);
  const blockWeeks = view === "1w" ? 1 : 2;

  // What the block views show.
  let firstWeek: string;
  let weekCount: number;
  let month: string | undefined;
  let title: string;
  if (view === "month") {
    const first = startOfMonth(anchorDate);
    const last = endOfMonth(anchorDate);
    const start = weekStartOf(first, weekStart);
    firstWeek = toIso(start);
    weekCount = differenceInCalendarWeeks(last, start, { weekStartsOn: weekStart }) + 1;
    month = format(first, "yyyy-MM");
    title = format(first, "MMMM yyyy");
  } else {
    firstWeek = toIso(weekStartOf(anchorDate, weekStart));
    weekCount = blockWeeks;
    const lastDay = fromIso(shiftWeeks(firstWeek, blockWeeks - 1));
    lastDay.setDate(lastDay.getDate() + 6);
    title = `${format(fromIso(firstWeek), "d MMM")} – ${format(lastDay, "d MMM yyyy")}`;
  }

  function step(direction: -1 | 1) {
    setAnchor(
      view === "month"
        ? toIso(addMonths(startOfMonth(anchorDate), direction))
        : shiftWeeks(firstWeek, direction * blockWeeks),
    );
  }

  function goToday() {
    setAnchor(toIso(new Date()));
    setTodayTick((n) => n + 1);
  }

  const dayNames = daysOfWeek(firstWeek).map((d) => format(fromIso(d), "EEE"));

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
              onClick={() => step(-1)}
              aria-label="Previous"
              className="rounded px-2 py-1 text-slate-600 hover:bg-slate-100"
            >
              ‹
            </button>
            <button
              onClick={() => step(1)}
              aria-label="Next"
              className="rounded px-2 py-1 text-slate-600 hover:bg-slate-100"
            >
              ›
            </button>
          </div>
        )}
        <span className="text-sm font-medium text-slate-700">
          {view === "scroll" ? format(new Date(), "EEEE d MMMM yyyy") : title}
        </span>

        <div className="ml-auto flex gap-1 rounded bg-slate-100 p-0.5">
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
            firstWeek={firstWeek}
            weekCount={weekCount}
            month={month}
            weekStart={weekStart}
            refreshKey={refreshKey}
          />
        )}
      </div>
    </div>
  );
}
