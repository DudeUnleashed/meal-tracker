import {
  addDays,
  addMonths,
  differenceInCalendarWeeks,
  endOfMonth,
  format,
  startOfMonth,
} from "date-fns";
import { fromIso, shiftWeeks, toIso, weekStartOf, type WeekStart } from "./dates";

export type CalendarView = "1w" | "2w" | "month" | "scroll";

export interface BlockRange {
  /** First day of the first week in the calendar grid. */
  firstWeek: string;
  weekCount: number;
  /** `YYYY-MM` for month view (days outside it are muted). */
  month?: string;
  /** The dates the block is *about* (the month itself, not the grid padding). Inclusive ISO. */
  start: string;
  end: string;
  title: string;
}

/** "5 Oct – 18 Oct 2026" */
export function rangeTitle(start: string, end: string): string {
  return `${format(fromIso(start), "d MMM")} – ${format(fromIso(end), "d MMM yyyy")}`;
}

/** The block of days shown (and summarised) for a view around `anchor` (any date in it). */
export function blockRange(view: CalendarView, anchor: string, weekStart: WeekStart): BlockRange {
  const anchorDate = fromIso(anchor);

  if (view === "month") {
    const first = startOfMonth(anchorDate);
    const last = endOfMonth(anchorDate);
    const gridStart = weekStartOf(first, weekStart);
    return {
      firstWeek: toIso(gridStart),
      weekCount: differenceInCalendarWeeks(last, gridStart, { weekStartsOn: weekStart }) + 1,
      month: format(first, "yyyy-MM"),
      start: toIso(first),
      end: toIso(last),
      title: format(first, "MMMM yyyy"),
    };
  }

  // The scroll view has no fixed block; its summary covers one week.
  const weekCount = view === "2w" ? 2 : 1;
  const firstWeek = toIso(weekStartOf(anchorDate, weekStart));
  const end = toIso(addDays(fromIso(shiftWeeks(firstWeek, weekCount - 1)), 6));
  return { firstWeek, weekCount, start: firstWeek, end, title: rangeTitle(firstWeek, end) };
}

/** Anchor for the previous (-1) or next (+1) block. */
export function stepAnchor(
  view: CalendarView,
  anchor: string,
  direction: -1 | 1,
  weekStart: WeekStart,
): string {
  if (view === "month") return toIso(addMonths(startOfMonth(fromIso(anchor)), direction));
  const { firstWeek, weekCount } = blockRange(view, anchor, weekStart);
  return shiftWeeks(firstWeek, direction * weekCount);
}

/** Moves a custom range by its own length (inclusive days). */
export function shiftRange(start: string, end: string, direction: -1 | 1): { start: string; end: string } {
  const days = Math.round((fromIso(end).getTime() - fromIso(start).getTime()) / 86400000) + 1;
  return {
    start: toIso(addDays(fromIso(start), direction * days)),
    end: toIso(addDays(fromIso(end), direction * days)),
  };
}
