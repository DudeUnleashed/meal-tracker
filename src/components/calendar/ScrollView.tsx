import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { api, errorMessage } from "../../lib/api";
import {
  daysOfWeek,
  fromIso,
  shiftWeeks,
  toIso,
  weekRange,
  weekStartOf,
  type WeekStart,
} from "../../lib/dates";
import { WeekRow, type DayInfos, type Entries } from "./cells";

/** Weeks loaded on first render either side of the current week, and per scroll extension. */
const INITIAL_WEEKS = 8;
const CHUNK_WEEKS = 8;

interface Props {
  weekStart: WeekStart;
  /** Bump to reload entries from the database. */
  refreshKey: number;
  /** Bump to scroll back to the current week. */
  todayTick: number;
}

function weeksBetween(first: string, last: string): number {
  return Math.round((fromIso(last).getTime() - fromIso(first).getTime()) / (7 * 24 * 3600 * 1000));
}

/** Continuous calendar: weeks load lazily as you scroll back into the past or forward. */
export default function ScrollView({ weekStart, refreshKey, todayTick }: Props) {
  const today = toIso(new Date());
  const currentWeek = toIso(weekStartOf(new Date(), weekStart));

  const [weeks, setWeeks] = useState<string[]>(() => [
    ...weekRange(shiftWeeks(currentWeek, -INITIAL_WEEKS), INITIAL_WEEKS),
    ...weekRange(currentWeek, INITIAL_WEEKS + 1),
  ]);
  const [entries, setEntries] = useState<Entries>({});
  const [dayInfos, setDayInfos] = useState<DayInfos>({});
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
      const [list, infos] = await Promise.all([
        api.plan.list(firstWeek, days[6]),
        api.dayInfo.list(firstWeek, days[6]),
      ]);
      const loadedDays = weekRange(firstWeek, weeksBetween(firstWeek, lastWeek) + 1).flatMap(daysOfWeek);
      setEntries((prev) => {
        const next = { ...prev };
        // Clear every loaded day first so removed entries disappear.
        for (const d of loadedDays) delete next[d];
        for (const e of list) (next[e.date] ??= []).push(e);
        return next;
      });
      setDayInfos((prev) => {
        const next = { ...prev };
        for (const d of loadedDays) delete next[d];
        for (const i of infos) next[i.date] = i;
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

  const scrollToCurrentWeek = useCallback(
    (smooth: boolean) => {
      const el = scroller.current?.querySelector(`[data-week="${currentWeek}"]`);
      el?.scrollIntoView({ block: "start", behavior: smooth ? "smooth" : "auto" });
    },
    [currentWeek],
  );

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

  // Today button.
  useEffect(() => {
    if (todayTick > 0) scrollToCurrentWeek(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [todayTick]);

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

  return (
    <div ref={scroller} data-calendar-scroller className="h-full overflow-y-auto">
      {error && <p className="px-4 py-1 text-sm text-red-600">{error}</p>}
      <div ref={topSentinel} className="h-px" />
      {weeks.map((w) => (
        <WeekRow
          key={w}
          weekStart={w}
          today={today}
          currentWeek={currentWeek}
          entries={entries}
          dayInfos={dayInfos}
        />
      ))}
      <div ref={bottomSentinel} className="h-px" />
    </div>
  );
}
