import { addDays, addWeeks, format, parseISO, startOfWeek } from "date-fns";

/** `weekStartsOn`: 0 = Sunday .. 6 = Saturday (same as the `week_start` setting). */
export type WeekStart = 0 | 1 | 2 | 3 | 4 | 5 | 6;

/** Dates travel to and from the backend as local ISO `YYYY-MM-DD` strings. */
export const toIso = (d: Date): string => format(d, "yyyy-MM-dd");

export const fromIso = (s: string): Date => parseISO(s);

export const weekStartOf = (d: Date, weekStartsOn: WeekStart): Date =>
  startOfWeek(d, { weekStartsOn });

/** ISO date `n` weeks after `iso` (negative = before). */
export const shiftWeeks = (iso: string, n: number): string => toIso(addWeeks(fromIso(iso), n));

/** ISO dates of the 7 days in the week beginning `weekStart`. */
export const daysOfWeek = (weekStart: string): string[] => {
  const first = fromIso(weekStart);
  return Array.from({ length: 7 }, (_, i) => toIso(addDays(first, i)));
};

/** ISO start dates of `count` consecutive weeks beginning at `first`. */
export const weekRange = (first: string, count: number): string[] =>
  Array.from({ length: count }, (_, i) => shiftWeeks(first, i));
