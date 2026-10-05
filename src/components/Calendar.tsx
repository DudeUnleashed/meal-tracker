import { format } from "date-fns";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { api, errorMessage, type PlanEntry } from "../lib/api";
import {
  daysOfWeek,
  fromIso,
  shiftWeeks,
  toIso,
  weekRange,
  weekStartOf,
  type WeekStart,
} from "../lib/dates";

/** Weeks loaded on first render either side of the current week, and per scroll extension. */
const INITIAL_WEEKS = 8;
const CHUNK_WEEKS = 8;

interface Props {
  weekStart: WeekStart;
  /** Bump to reload entries from the database (e.g. after an edit elsewhere). */
  refreshKey?: number;
}

export default function Calendar({ weekStart, refreshKey = 0 }: Props) {
  const today = toIso(new Date());
  const currentWeek = toIso(weekStartOf(new Date(), weekStart));

  const [weeks, setWeeks] = useState<string[]>(() => [
    ...weekRange(shiftWeeks(currentWeek, -INITIAL_WEEKS), INITIAL_WEEKS),
    ...weekRange(currentWeek, INITIAL_WEEKS + 1),
  ]);
  const [entries, setEntries] = useState<Record<string, PlanEntry[]>>({});
  const [error, setError] = useState<string | null>(null);
  const [initialLoaded, setInitialLoaded] = useState(false);

  const scroller = useRef<HTMLDivElement>(null);
  const topSentinel = useRef<HTMLDivElement>(null);
  const bottomSentinel = useRef<HTMLDivElement>(null);
  const ready = useRef(false); // true once the initial scroll to "this week" is done
  const prependedFrom = useRef<number | null>(null); // scrollHeight before a prepend

  /** Fetches [firstWeek, lastWeek + 6 days] and replaces those days in state. */
  const loadRange = useCallback(async (firstWeek: string, lastWeek: string) => {
    const days = daysOfWeek(lastWeek);
    try {
      const list = await api.plan.list(firstWeek, days[6]);
      setEntries((prev) => {
        const next = { ...prev };
        // Clear every loaded day first so removed entries disappear.
        for (const w of weekRange(firstWeek, weeksBetween(firstWeek, lastWeek) + 1)) {
          for (const d of daysOfWeek(w)) delete next[d];
        }
        for (const e of list) (next[e.date] ??= []).push(e);
        return next;
      });
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  // Initial load, and reload of everything on refresh.
  useEffect(() => {
    loadRange(weeks[0], weeks[weeks.length - 1]).then(() => setInitialLoaded(true));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshKey]);

  const scrollToCurrentWeek = useCallback((smooth: boolean) => {
    const el = scroller.current?.querySelector(`[data-week="${currentWeek}"]`);
    el?.scrollIntoView({ block: "start", behavior: smooth ? "smooth" : "auto" });
  }, [currentWeek]);

  // Open on the current week.
  useLayoutEffect(() => {
    scrollToCurrentWeek(false);
    ready.current = true;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Entries make earlier weeks taller, which pushes "this week" down; re-anchor once the
  // first load has rendered so the calendar really opens on the current week.
  useLayoutEffect(() => {
    if (initialLoaded) scrollToCurrentWeek(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialLoaded]);

  // After weeks are prepended, keep the viewport anchored on what the user was looking at.
  useLayoutEffect(() => {
    const el = scroller.current;
    if (el && prependedFrom.current !== null) {
      el.scrollTop += el.scrollHeight - prependedFrom.current;
      prependedFrom.current = null;
    }
  }, [weeks]);

  const extendPast = useCallback(() => {
    const el = scroller.current;
    if (!ready.current || !el || prependedFrom.current !== null) return;
    const first = shiftWeeks(weeks[0], -CHUNK_WEEKS);
    prependedFrom.current = el.scrollHeight;
    setWeeks((w) => [...weekRange(first, CHUNK_WEEKS), ...w]);
    loadRange(first, shiftWeeks(weeks[0], -1));
  }, [weeks, loadRange]);

  const extendFuture = useCallback(() => {
    if (!ready.current) return;
    const next = shiftWeeks(weeks[weeks.length - 1], 1);
    setWeeks((w) => [...w, ...weekRange(next, CHUNK_WEEKS)]);
    loadRange(next, shiftWeeks(next, CHUNK_WEEKS - 1));
  }, [weeks, loadRange]);

  // Load more weeks whenever either end scrolls into view.
  useEffect(() => {
    const root = scroller.current;
    if (!root) return;
    const observer = new IntersectionObserver(
      (hits) => {
        for (const h of hits) {
          if (!h.isIntersecting) continue;
          if (h.target === topSentinel.current) extendPast();
          else extendFuture();
        }
      },
      { root, rootMargin: "300px 0px" },
    );
    if (topSentinel.current) observer.observe(topSentinel.current);
    if (bottomSentinel.current) observer.observe(bottomSentinel.current);
    return () => observer.disconnect();
  }, [extendPast, extendFuture]);

  const dayNames = daysOfWeek(currentWeek).map((d) => format(fromIso(d), "EEE"));

  return (
    <div className="flex h-full min-w-0 flex-col">
      <div className="flex items-center gap-3 border-b border-slate-200 bg-white px-4 py-2">
        <button
          onClick={() => scrollToCurrentWeek(true)}
          className="rounded border border-slate-300 px-3 py-1 text-sm font-medium hover:bg-slate-50"
        >
          Today
        </button>
        <span className="text-sm text-slate-500">{format(new Date(), "EEEE d MMMM yyyy")}</span>
        {error && <span className="ml-auto text-sm text-red-600">{error}</span>}
      </div>

      <div className="grid grid-cols-7 border-b border-slate-200 bg-slate-100 text-center text-xs font-medium text-slate-500">
        {dayNames.map((n) => (
          <div key={n} className="py-1">
            {n}
          </div>
        ))}
      </div>

      <div ref={scroller} className="min-h-0 flex-1 overflow-y-auto">
        <div ref={topSentinel} className="h-px" />
        {weeks.map((w) => (
          <WeekRow key={w} weekStart={w} today={today} currentWeek={currentWeek} entries={entries} />
        ))}
        <div ref={bottomSentinel} className="h-px" />
      </div>
    </div>
  );
}

function weeksBetween(first: string, last: string): number {
  return Math.round((fromIso(last).getTime() - fromIso(first).getTime()) / (7 * 24 * 3600 * 1000));
}

interface WeekRowProps {
  weekStart: string;
  today: string;
  currentWeek: string;
  entries: Record<string, PlanEntry[]>;
}

function WeekRow({ weekStart, today, currentWeek, entries }: WeekRowProps) {
  const days = daysOfWeek(weekStart);
  const isCurrent = weekStart === currentWeek;
  const label = isCurrent ? "this week" : weekStart < currentWeek ? "past" : null;

  return (
    <section data-week={weekStart}>
      <h3
        className={`sticky top-0 z-10 border-y border-slate-200 px-3 py-1 text-xs font-medium ${
          isCurrent ? "bg-emerald-50 text-emerald-800" : "bg-slate-50 text-slate-500"
        }`}
      >
        Week of {format(fromIso(weekStart), "d MMM")}
        {label && <span className="ml-1 font-normal">({label})</span>}
      </h3>
      <div className="grid grid-cols-7">
        {days.map((d) => (
          <DayCell key={d} date={d} today={today} entries={entries[d] ?? []} />
        ))}
      </div>
    </section>
  );
}

function DayCell({ date, today, entries }: { date: string; today: string; entries: PlanEntry[] }) {
  const d = fromIso(date);
  const isToday = date === today;
  const isPast = date < today;
  // Show the month on the 1st (and the first visible day of each week row stays uncluttered).
  const dayLabel = d.getDate() === 1 ? format(d, "d MMM") : format(d, "d");

  return (
    <div
      className={`min-h-28 border-b border-r border-slate-200 p-1 ${
        isToday ? "bg-emerald-50/60" : "bg-white"
      } ${isPast ? "opacity-60" : ""}`}
    >
      <div className="mb-1 flex justify-end">
        <span
          className={`text-xs ${
            isToday
              ? "rounded-full bg-emerald-600 px-1.5 py-0.5 font-semibold text-white"
              : "text-slate-400"
          }`}
        >
          {dayLabel}
        </span>
      </div>
      <div className="space-y-1">
        {entries.map((e) => (
          <EntryChip key={e.id} entry={e} />
        ))}
      </div>
    </div>
  );
}

function EntryChip({ entry }: { entry: PlanEntry }) {
  const colour = entry.item_colour ?? "#94a3b8";
  return (
    <div
      className="flex items-center gap-1 rounded border-l-4 bg-slate-50 px-1.5 py-1 text-xs shadow-sm"
      style={{ borderLeftColor: colour }}
      title={entry.item_name}
    >
      <span className={`flex-1 truncate ${entry.item_archived ? "italic text-slate-500" : ""}`}>
        {entry.item_name}
      </span>
      {entry.serves !== null && (
        <span className="shrink-0 font-medium text-slate-600">×{entry.serves}</span>
      )}
      {entry.note && (
        <span className="shrink-0" title={entry.note}>
          📝
        </span>
      )}
    </div>
  );
}
