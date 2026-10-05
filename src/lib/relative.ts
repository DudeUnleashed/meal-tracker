import { differenceInCalendarDays, format } from "date-fns";
import { fromIso } from "./dates";

/** "yesterday", "12 days ago", "3 weeks ago", "4 months ago". `date` is before `today`. */
export function lastHadText(date: string, today: Date): string {
  const days = differenceInCalendarDays(today, fromIso(date));
  if (days <= 1) return "yesterday";
  if (days < 14) return `${days} days ago`;
  if (days < 60) return `${Math.floor(days / 7)} weeks ago`;
  return `${Math.floor(days / 30)} months ago`;
}

/** "today", "tomorrow", "Thu" (within a week) or "12 Nov". `date` is today or later. */
export function plannedText(date: string, today: Date): string {
  const days = differenceInCalendarDays(fromIso(date), today);
  if (days <= 0) return "today";
  if (days === 1) return "tomorrow";
  if (days < 7) return format(fromIso(date), "EEE");
  return format(fromIso(date), "d MMM");
}
