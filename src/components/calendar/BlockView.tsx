import { useEffect, useState } from "react";
import { api, errorMessage } from "../../lib/api";
import { daysOfWeek, toIso, weekRange, weekStartOf, type WeekStart } from "../../lib/dates";
import { WeekRow, type DayInfos, type Entries } from "./cells";

interface Props {
  /** First day of the first week shown. */
  firstWeek: string;
  weekCount: number;
  /** `YYYY-MM` — mutes days outside that month (month view). */
  month?: string;
  weekStart: WeekStart;
  refreshKey: number;
}

/** A fixed block of weeks (1 week, 2 weeks or a month) that fills the available space. */
export default function BlockView({ firstWeek, weekCount, month, weekStart, refreshKey }: Props) {
  const today = toIso(new Date());
  const currentWeek = toIso(weekStartOf(new Date(), weekStart));
  const weeks = weekRange(firstWeek, weekCount);
  const last = weeks[weeks.length - 1];

  const [entries, setEntries] = useState<Entries>({});
  const [dayInfos, setDayInfos] = useState<DayInfos>({});
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const end = daysOfWeek(last)[6];
    Promise.all([api.plan.list(firstWeek, end), api.dayInfo.list(firstWeek, end)])
      .then(([list, infos]) => {
        if (cancelled) return;
        const byDate: Entries = {};
        for (const e of list) (byDate[e.date] ??= []).push(e);
        setEntries(byDate);
        setDayInfos(Object.fromEntries(infos.map((i) => [i.date, i])));
        setError(null);
      })
      .catch((e) => !cancelled && setError(errorMessage(e)));
    return () => {
      cancelled = true;
    };
  }, [firstWeek, last, refreshKey]);

  return (
    <div className="flex h-full flex-col overflow-y-auto">
      {error && <p className="px-4 py-1 text-sm text-red-600">{error}</p>}
      {weeks.map((w) => (
        <WeekRow
          key={w}
          weekStart={w}
          today={today}
          currentWeek={currentWeek}
          entries={entries}
          dayInfos={dayInfos}
          fill
          month={month}
          // The month grid is dense enough without a heading per week.
          heading={month === undefined}
        />
      ))}
    </div>
  );
}
