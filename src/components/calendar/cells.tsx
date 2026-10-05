import { useDraggable, useDroppable } from "@dnd-kit/core";
import { format } from "date-fns";
import type { DayInfo, PlanEntry } from "../../lib/api";
import { daysOfWeek, fromIso } from "../../lib/dates";
import { usePlannerActions, type DragData } from "./actions";

export type Entries = Record<string, PlanEntry[]>;
export type DayInfos = Record<string, DayInfo>;

/** "1.5" not "1.50"; whole numbers stay whole. */
const fmtScale = (n: number) => String(Number(n.toFixed(2)));

interface WeekRowProps {
  weekStart: string;
  today: string;
  currentWeek: string;
  entries: Entries;
  dayInfos: DayInfos;
  /** Grow to fill the available height (block views) instead of using a minimum cell height. */
  fill?: boolean;
  /** `YYYY-MM`: days outside this month are muted (month view). */
  month?: string;
  /** Show the "Week of …" heading. */
  heading?: boolean;
}

export function WeekRow({
  weekStart,
  today,
  currentWeek,
  entries,
  dayInfos,
  fill,
  month,
  heading = true,
}: WeekRowProps) {
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
            info={dayInfos[d]}
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
  info?: DayInfo;
  outsideMonth?: boolean;
  compact?: boolean;
}

export function DayCell({ date, today, entries, info, outsideMonth, compact }: DayCellProps) {
  const d = fromIso(date);
  const isToday = date === today;
  const isPast = date < today;
  // Show the month on the 1st so the boundary is easy to spot.
  const dayLabel = d.getDate() === 1 ? format(d, "d MMM") : format(d, "d");
  const { openDay } = usePlannerActions();

  const data: DragData = { kind: "day", date, count: entries.length };
  const { setNodeRef, isOver } = useDroppable({ id: `day:${date}`, data });

  return (
    <div
      ref={setNodeRef}
      data-date={date}
      className={`group overflow-y-auto border-b border-r border-slate-200 p-1 ${
        compact ? "min-h-28" : "min-h-40"
      } ${isToday ? "bg-emerald-50/60" : outsideMonth ? "bg-slate-50" : "bg-white"} ${
        isOver ? "ring-2 ring-inset ring-emerald-400" : ""
      }`}
    >
      {/* Dim past / out-of-month days' content but keep the drop highlight crisp. */}
      <div className={isPast || outsideMonth ? "opacity-60" : ""}>
        <div className="mb-1 flex items-center justify-between gap-1">
          {!info?.note ? (
            <button
              type="button"
              onClick={(e) => openDay(date, e.currentTarget.getBoundingClientRect())}
              title="Add a note for this day"
              aria-label={`Add a note for ${date}`}
              className="rounded px-1 text-xs text-slate-400 opacity-0 hover:bg-slate-100 hover:text-slate-700 focus:opacity-100 group-hover:opacity-100"
            >
              ＋ note
            </button>
          ) : (
            <span />
          )}
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

        {info?.note && (
          <button
            type="button"
            onClick={(e) => openDay(date, e.currentTarget.getBoundingClientRect())}
            title={info.note}
            className="mb-1 block w-full rounded bg-amber-50 px-1.5 py-0.5 text-left text-xs leading-snug text-amber-800 hover:bg-amber-100"
          >
            <span className="line-clamp-3">📝 {info.note}</span>
          </button>
        )}

        <div className="space-y-1">
          {info?.leftovers && <LeftoversChip date={date} />}
          {entries.map((e, i) => (
            <EntryChip key={e.id} entry={e} index={i} />
          ))}
        </div>
      </div>
    </div>
  );
}

/** The "Leftovers" marker for a day: no data, just a visible placeholder. Drag it to move it. */
function LeftoversChip({ date }: { date: string }) {
  const data: DragData = { kind: "leftovers-chip", date };
  const { setNodeRef, listeners, attributes, isDragging } = useDraggable({ id: `leftovers:${date}`, data });
  const { removeLeftovers } = usePlannerActions();

  return (
    <div
      ref={setNodeRef}
      {...listeners}
      {...attributes}
      className={`touch-none ${isDragging ? "opacity-30" : ""}`}
    >
      <LeftoversBody onRemove={() => removeLeftovers(date)} />
    </div>
  );
}

export function LeftoversBody({ onRemove }: { onRemove?: () => void }) {
  return (
    <div className="group/left flex cursor-grab items-center gap-1 rounded border border-dashed border-teal-400 bg-teal-50 px-1.5 py-1 text-xs text-teal-800">
      <span className="flex-1 truncate">🍲 Leftovers</span>
      {onRemove && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onRemove();
          }}
          onPointerDown={(e) => e.stopPropagation()}
          aria-label="Remove leftovers"
          title="Remove"
          className="shrink-0 px-0.5 text-teal-400 opacity-0 hover:text-red-600 focus:opacity-100 group-hover/left:opacity-100"
        >
          ✕
        </button>
      )}
    </div>
  );
}

/** A planned entry: draggable to another day / position, and a drop target for reordering. */
function EntryChip({ entry, index }: { entry: PlanEntry; index: number }) {
  const data: DragData = { kind: "entry", entryId: entry.id, date: entry.date, index, entry };
  const drag = useDraggable({ id: `entry:${entry.id}`, data });
  const drop = useDroppable({ id: `entry-drop:${entry.id}`, data });
  const { changeServes, removeEntry, openEntry } = usePlannerActions();

  return (
    <div
      ref={(node) => {
        drag.setNodeRef(node);
        drop.setNodeRef(node);
      }}
      {...drag.listeners}
      {...drag.attributes}
      // A click (not a drag — dnd-kit swallows the click that ends a drag) opens the details popover.
      onClick={(e) => openEntry(entry, e.currentTarget.getBoundingClientRect())}
      className={`touch-none ${drag.isDragging ? "opacity-30" : ""} ${
        drop.isOver && !drag.isDragging ? "ring-2 ring-emerald-400" : ""
      }`}
    >
      <ChipBody
        entry={entry}
        onServes={(n) => changeServes(entry, n)}
        onRemove={() => removeEntry(entry)}
      />
    </div>
  );
}

interface ChipBodyProps {
  entry: PlanEntry;
  /** Omit both callbacks for a static preview (the drag overlay). */
  onServes?: (serves: number) => void;
  onRemove?: () => void;
}

export function ChipBody({ entry, onServes, onRemove }: ChipBodyProps) {
  const colour = entry.item_colour ?? "#94a3b8";
  const sized = entry.item_type !== "meat" && Math.abs(entry.scale - 1) > 1e-9;
  return (
    <div
      className="group cursor-grab rounded border-l-4 bg-slate-50 px-1.5 py-1 text-xs shadow-sm"
      style={{ borderLeftColor: colour }}
      title={entry.item_name}
    >
      <div className="flex items-center gap-1">
        <span className={`flex-1 truncate ${entry.item_archived ? "italic text-slate-500" : ""}`}>
          {entry.item_name}
        </span>
        {sized && (
          <span
            className="shrink-0 rounded bg-sky-100 px-1 font-medium text-sky-800"
            title={`Cooking ${fmtScale(entry.scale)}× the usual amount`}
          >
            ×{fmtScale(entry.scale)}
          </span>
        )}
        {entry.note && (
          <span className="shrink-0" title={entry.note}>
            📝
          </span>
        )}
        {onRemove && (
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              onRemove();
            }}
            aria-label={`Remove ${entry.item_name}`}
            title="Remove"
            className="shrink-0 px-0.5 text-slate-300 opacity-0 hover:text-red-600 focus:opacity-100 group-hover:opacity-100"
          >
            ✕
          </button>
        )}
      </div>
      {entry.serves !== null && (
        <div className="mt-0.5 flex items-center gap-1 text-slate-600">
          {onServes ? (
            <>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  onServes(entry.serves! - 1);
                }}
                disabled={entry.serves <= 1}
                aria-label="Fewer serves"
                className="h-4 w-4 rounded bg-slate-200 leading-none hover:bg-slate-300 disabled:opacity-40"
              >
                −
              </button>
              <span className="min-w-8 text-center font-medium">{entry.serves} serves</span>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  onServes(entry.serves! + 1);
                }}
                aria-label="More serves"
                className="h-4 w-4 rounded bg-slate-200 leading-none hover:bg-slate-300"
              >
                +
              </button>
            </>
          ) : (
            <span className="font-medium">×{entry.serves}</span>
          )}
        </div>
      )}
    </div>
  );
}
