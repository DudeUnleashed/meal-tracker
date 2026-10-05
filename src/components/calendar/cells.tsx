import { format } from "date-fns";
import type { PlanEntry } from "../../lib/api";
import { daysOfWeek, fromIso } from "../../lib/dates";

export type Entries = Record<string, PlanEntry[]>;

interface WeekRowProps {
  weekStart: string;
  today: string;
  currentWeek: string;
  entries: Entries;
  /** Grow to fill the available height (block views) instead of using a minimum cell height. */
  fill?: boolean;
  /** Days outside this month are muted (month view). */
  month?: string;
  /** Show the "Week of …" heading. */
  heading?: boolean;
}

export function WeekRow({ weekStart, today, currentWeek, entries, fill, month, heading = true }: WeekRowProps) {
  const days = daysOfWeek(weekStart);
  const isCurrent = weekStart === currentWeek;
  const label = isCurrent ? "this week" : weekStart < currentWeek ? "past" : null;

  return (
    <section data-week={weekStart} className={fill ? "flex min-h-0 flex-1 flex-col" : undefined}>
      {heading && (
        <h3
          className={`sticky top-0 z-10 border-y border-slate-200 px-3 py-1 text-xs font-medium ${
            isCurrent ? "bg-emerald-50 text-emerald-800" : "bg-slate-50 text-slate-500"
          }`}
        >
          Week of {format(fromIso(weekStart), "d MMM")}
          {label && <span className="ml-1 font-normal">({label})</span>}
        </h3>
      )}
      <div className={`grid grid-cols-7 ${fill ? "min-h-0 flex-1" : ""}`}>
        {days.map((d) => (
          <DayCell
            key={d}
            date={d}
            today={today}
            entries={entries[d] ?? []}
            outsideMonth={month !== undefined && d.slice(0, 7) !== month}
            compact={!fill || month !== undefined}
          />
        ))}
      </div>
    </section>
  );
}

interface DayCellProps {
  date: string;
  today: string;
  entries: PlanEntry[];
  outsideMonth?: boolean;
  compact?: boolean;
}

export function DayCell({ date, today, entries, outsideMonth, compact }: DayCellProps) {
  const d = fromIso(date);
  const isToday = date === today;
  const isPast = date < today;
  // Show the month on the 1st so the boundary is easy to spot.
  const dayLabel = d.getDate() === 1 ? format(d, "d MMM") : format(d, "d");

  return (
    <div
      data-date={date}
      className={`overflow-y-auto border-b border-r border-slate-200 p-1 ${
        compact ? "min-h-28" : "min-h-40"
      } ${isToday ? "bg-emerald-50/60" : outsideMonth ? "bg-slate-50" : "bg-white"} ${
        isPast || outsideMonth ? "opacity-60" : ""
      }`}
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

export function EntryChip({ entry }: { entry: PlanEntry }) {
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
