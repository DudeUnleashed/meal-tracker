import {
  DndContext,
  DragOverlay,
  MouseSensor,
  TouchSensor,
  pointerWithin,
  useSensor,
  useSensors,
  type CollisionDetection,
  type DragEndEvent,
  type DragStartEvent,
} from "@dnd-kit/core";
import { useCallback, useEffect, useMemo, useState } from "react";
import Calendar, { VIEWS, type CalendarView } from "../components/Calendar";
import { PlannerActionsContext, type DragData, type PlannerActions } from "../components/calendar/actions";
import { ChipBody } from "../components/calendar/cells";
import ItemDrawer from "../components/ItemDrawer";
import ItemLibrary, { CardPreview } from "../components/ItemLibrary";
import PlanEntryPopover from "../components/PlanEntryPopover";
import SummaryPanel, { type SummaryMode } from "../components/SummaryPanel";
import { api, errorMessage, type PlanEntry } from "../lib/api";
import { addDays } from "date-fns";
import { fromIso, shiftWeeks, toIso, type WeekStart } from "../lib/dates";
import { blockRange, shiftRange, stepAnchor } from "../lib/ranges";

const DEFAULT_VIEW: CalendarView = "2w";

/**
 * Chips sit inside day cells and are both targets. Prefer the chip under the pointer
 * (drop = insert at its position); otherwise fall back to the day (drop = append).
 */
const collisionDetection: CollisionDetection = (args) => {
  const hits = pointerWithin(args);
  const kindOf = (id: string | number) =>
    (args.droppableContainers.find((c) => c.id === id)?.data.current as DragData | undefined)?.kind;
  const chip = hits.filter((h) => kindOf(h.id) === "entry");
  return chip.length ? chip : hits;
};

export default function PlannerPage() {
  // The calendar waits for settings so it builds around the right first day and saved view.
  const [weekStart, setWeekStart] = useState<WeekStart | null>(null);
  const [view, setView] = useState<CalendarView>(DEFAULT_VIEW);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const [dragging, setDragging] = useState<DragData | null>(null);
  const [popover, setPopover] = useState<{ entry: PlanEntry; anchor: DOMRect } | null>(null);
  const [drawerItemId, setDrawerItemId] = useState<number | null>(null);
  // Any date inside the block on screen; the summary follows it.
  const [anchor, setAnchor] = useState(() => toIso(new Date()));
  const [summaryOpen, setSummaryOpen] = useState(false);
  // A user-picked summary range that no longer follows the calendar (null = follow the view).
  const [custom, setCustom] = useState<{ start: string; end: string } | null>(null);

  const sensors = useSensors(
    useSensor(MouseSensor, { activationConstraint: { distance: 6 } }),
    // Long-press to drag on touch so scrolling still works (Android later).
    useSensor(TouchSensor, { activationConstraint: { delay: 250, tolerance: 5 } }),
  );

  useEffect(() => {
    api.settings
      .get()
      .then((s) => {
        setWeekStart((Number(s.week_start ?? 1) % 7) as WeekStart);
        const saved = VIEWS.find((v) => v.id === s.planner_view);
        if (saved) setView(saved.id);
      })
      .catch((e) => setError(errorMessage(e)));
  }, []);

  function changeView(next: CalendarView) {
    setView(next);
    api.settings.set("planner_view", next).catch((e) => setError(errorMessage(e)));
  }

  /** Runs a database change, then reloads the calendar so it always shows what's stored. */
  const mutate = useCallback(async (action: () => Promise<unknown>) => {
    try {
      setError(null);
      await action();
    } catch (e) {
      setError(errorMessage(e));
    }
    setRefreshKey((k) => k + 1);
  }, []);

  const actions: PlannerActions = useMemo(
    () => ({
      changeServes: (entry: PlanEntry, serves: number) =>
        mutate(() => api.plan.update(entry.id, { serves })),
      removeEntry: (entry: PlanEntry) => mutate(() => api.plan.remove(entry.id)),
      openEntry: (entry: PlanEntry, anchor: DOMRect) => setPopover({ entry, anchor }),
    }),
    [mutate],
  );

  const reload = useCallback(() => setRefreshKey((k) => k + 1), []);

  /** Fills the block on screen with the dinners from the block just before it. */
  async function copyPrevious() {
    if (weekStart === null || (view !== "1w" && view !== "2w")) return;
    const b = blockRange(view, anchor, weekStart);
    const srcStart = shiftWeeks(b.firstWeek, -b.weekCount);
    const srcEnd = toIso(addDays(fromIso(b.firstWeek), -1));
    const noun = view === "1w" ? "week" : "fortnight";
    try {
      setError(null);
      const existing = await api.plan.list(b.start, b.end);
      if (
        existing.length > 0 &&
        !window.confirm(
          `This ${noun} already has ${existing.length} planned ${existing.length === 1 ? "item" : "items"}. Add last ${noun}'s dinners on top of them?`,
        )
      )
        return;
      const r = await api.plan.copyRange(srcStart, srcEnd, b.firstWeek);
      reload();
      setNotice(
        r.copied === 0
          ? `Nothing to copy — last ${noun} has no dinners planned.`
          : `Copied ${r.copied} ${r.copied === 1 ? "dinner" : "dinners"} from last ${noun}` +
              (r.skipped_archived ? ` (${r.skipped_archived} skipped: archived items)` : "") +
              ".",
      );
      setTimeout(() => setNotice(null), 4000);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  // The summary covers the block on screen (the scroll view counts as one week) unless customised.
  const summaryView = view === "scroll" ? "1w" : view;
  const block = weekStart === null ? null : blockRange(summaryView, anchor, weekStart);
  const summaryStart = custom?.start ?? block?.start ?? anchor;
  const summaryEnd = custom?.end ?? block?.end ?? anchor;
  const summaryMode: SummaryMode = custom ? "custom" : summaryView;

  function onSummaryMode(mode: SummaryMode) {
    if (mode === "custom") {
      setCustom({ start: summaryStart, end: summaryEnd });
    } else {
      setCustom(null);
      changeView(mode);
    }
  }

  function onSummaryStep(direction: -1 | 1) {
    if (custom) setCustom(shiftRange(custom.start, custom.end, direction));
    else if (weekStart !== null) {
      if (view === "scroll") changeView("1w");
      setAnchor(stepAnchor(summaryView, anchor, direction, weekStart));
    }
  }

  function onDragStart(e: DragStartEvent) {
    setPopover(null);
    setDragging((e.active.data.current as DragData) ?? null);
  }

  function onDragEnd({ active, over }: DragEndEvent) {
    setDragging(null);
    const from = active.data.current as DragData | undefined;
    const to = over?.data.current as DragData | undefined;
    if (!from || !to || to.kind === "item") return;

    // Dropping on a day appends; dropping on a chip takes that chip's place.
    let date: string;
    let position: number;
    if (to.kind === "day") {
      date = to.date;
      position = to.count;
    } else {
      if (from.kind === "entry" && from.entryId === to.entryId) return; // dropped on itself
      date = to.date;
      position = to.index;
    }

    if (from.kind === "item") {
      mutate(async () => {
        const entry = await api.plan.add(from.itemId, date);
        if (to.kind === "entry") await api.plan.move(entry.id, date, position);
      });
    } else if (from.kind === "entry") {
      mutate(() => api.plan.move(from.entryId, date, position));
    }
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={collisionDetection}
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onDragCancel={() => setDragging(null)}
    >
      <PlannerActionsContext.Provider value={actions}>
        <div className="flex h-full">
          <section className="flex min-w-0 flex-1 flex-col">
            {error && (
              <p className="border-b border-red-200 bg-red-50 px-4 py-1 text-sm text-red-700">{error}</p>
            )}
            {notice && (
              <p className="border-b border-emerald-200 bg-emerald-50 px-4 py-1 text-sm text-emerald-800">
                {notice}
              </p>
            )}
            <div className="min-h-0 flex-1">
              {weekStart === null ? (
                <p className="p-4 text-sm text-slate-500">{error ?? "Loading…"}</p>
              ) : (
                <Calendar
                  key={weekStart}
                  weekStart={weekStart}
                  view={view}
                  onViewChange={(v) => {
                    setCustom(null);
                    changeView(v);
                  }}
                  anchor={anchor}
                  onAnchorChange={setAnchor}
                  refreshKey={refreshKey}
                  summaryOpen={summaryOpen}
                  onToggleSummary={() => setSummaryOpen((o) => !o)}
                  onCopyPrevious={copyPrevious}
                />
              )}
            </div>
          </section>
          {/* Kept mounted (just hidden) so the library keeps its tab and search while summarising. */}
          <div className={summaryOpen ? "hidden" : "flex"}>
            <ItemLibrary refreshKey={refreshKey} />
          </div>
          {summaryOpen && (
            <SummaryPanel
              start={summaryStart}
              end={summaryEnd}
              mode={summaryMode}
              onMode={onSummaryMode}
              onStep={onSummaryStep}
              onCustomRange={(start, end) => setCustom({ start, end })}
              refreshKey={refreshKey}
            />
          )}
        </div>
      </PlannerActionsContext.Provider>

      {popover && (
        <PlanEntryPopover
          key={popover.entry.id}
          entry={popover.entry}
          anchor={popover.anchor}
          onClose={() => setPopover(null)}
          onChanged={reload}
          onEditItem={setDrawerItemId}
        />
      )}
      {drawerItemId !== null && (
        <ItemDrawer itemId={drawerItemId} onClose={() => setDrawerItemId(null)} onChanged={reload} />
      )}

      <DragOverlay dropAnimation={null}>
        {dragging?.kind === "item" && <CardPreview name={dragging.name} colour={dragging.colour} />}
        {dragging?.kind === "entry" && (
          <div className="w-44">
            <ChipBody entry={dragging.entry} />
          </div>
        )}
      </DragOverlay>
    </DndContext>
  );
}
