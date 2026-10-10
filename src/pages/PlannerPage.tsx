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
import { ChipBody, LeftoversBody } from "../components/calendar/cells";
import ItemDrawer from "../components/ItemDrawer";
import DayNotePopover from "../components/DayNotePopover";
import ItemLibrary, { BlankPreview, CardPreview, LeftoversPreview } from "../components/ItemLibrary";
import PlanEntryPopover from "../components/PlanEntryPopover";
import SummaryPanel, { type SummaryMode } from "../components/SummaryPanel";
import { api, errorMessage, type DayInfo, type PlanEntry, type Slot } from "../lib/api";
import { addDays } from "date-fns";
import { fromIso, shiftWeeks, toIso, type WeekStart } from "../lib/dates";
import { blockRange, shiftRange, stepAnchor } from "../lib/ranges";

const DEFAULT_VIEW: CalendarView = "2w";

/**
 * Chips sit inside slots inside day cells, and all are targets. Prefer the chip under the
 * pointer (drop = insert at its position), then the slot (drop = append to it), then the day.
 */
const collisionDetection: CollisionDetection = (args) => {
  const hits = pointerWithin(args);
  const kindOf = (id: string | number) =>
    (args.droppableContainers.find((c) => c.id === id)?.data.current as DragData | undefined)?.kind;
  for (const kind of ["entry", "slot"]) {
    const found = hits.filter((h) => kindOf(h.id) === kind);
    if (found.length) return found;
  }
  return hits;
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
  const [dayPopover, setDayPopover] = useState<{ date: string; anchor: DOMRect; info?: DayInfo } | null>(null);
  // What the Undo button would revert, e.g. "Removed Steak" (null = nothing to undo).
  const [undoLabel, setUndoLabel] = useState<string | null>(null);
  // Any date inside the block on screen; the summary follows it.
  const [anchor, setAnchor] = useState(() => toIso(new Date()));
  const [summaryOpen, setSummaryOpen] = useState(false);
  // Lunch / breakfast / snacks the user added to a day but hasn't put anything in yet.
  const [openSlots, setOpenSlots] = useState<Record<string, Slot[]>>({});
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

  // Re-check what Undo would do whenever the plan may have changed.
  useEffect(() => {
    api.plan
      .undoPeek()
      .then(setUndoLabel)
      .catch(() => setUndoLabel(null));
  }, [refreshKey]);

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
      openDay: async (date: string, anchor: DOMRect) => {
        try {
          const [info] = await api.dayInfo.list(date, date);
          setPopover(null);
          setDayPopover({ date, anchor, info });
        } catch (e) {
          setError(errorMessage(e));
        }
      },
      removeLeftovers: (date: string) => mutate(() => api.dayInfo.set(date, { leftovers: false })),
      openSlots,
      showSlot: (date: string, slot: Slot) =>
        setOpenSlots((prev) => ({ ...prev, [date]: [...(prev[date] ?? []), slot] })),
    }),
    [mutate, openSlots],
  );

  /**
   * Adds something to the plan and, for things that need details (a template's veg, a blank's
   * title), opens its popover straight away on the new chip.
   */
  async function addAndOpen(add: () => Promise<PlanEntry>) {
    try {
      setError(null);
      const entry = await add();
      setRefreshKey((k) => k + 1);
      for (let i = 0; i < 20; i++) {
        await new Promise((r) => setTimeout(r, 100));
        const el = document.querySelector(`[data-entry="${entry.id}"]`);
        if (el) {
          setPopover({ entry, anchor: el.getBoundingClientRect() });
          return;
        }
      }
    } catch (e) {
      setError(errorMessage(e));
      setRefreshKey((k) => k + 1);
    }
  }

  const reload = useCallback(() => setRefreshKey((k) => k + 1), []);

  /** The "whoops" button: reverts the most recent plan change. */
  async function undo() {
    try {
      setError(null);
      setPopover(null);
      setDayPopover(null);
      const undone = await api.plan.undo();
      reload();
      if (undone) {
        setNotice(`Undone: ${undone}`);
        setTimeout(() => setNotice(null), 3000);
      }
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  /** Fills the block on screen with the meals from the block just before it. */
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
          `This ${noun} already has ${existing.length} planned ${existing.length === 1 ? "item" : "items"}. Add last ${noun}'s meals on top of them?`,
        )
      )
        return;
      const r = await api.plan.copyRange(srcStart, srcEnd, b.firstWeek);
      reload();
      setNotice(
        r.copied === 0
          ? `Nothing to copy — last ${noun} has nothing planned.`
          : `Copied ${r.copied} ${r.copied === 1 ? "meal" : "meals"} from last ${noun}` +
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
    setDayPopover(null);
    setDragging((e.active.data.current as DragData) ?? null);
  }

  function onDragEnd({ active, over }: DragEndEvent) {
    setDragging(null);
    const from = active.data.current as DragData | undefined;
    const to = over?.data.current as DragData | undefined;
    if (!from || (to?.kind !== "day" && to?.kind !== "entry" && to?.kind !== "slot")) return;
    const date = to.date;

    // The Leftovers tag: from the library onto a day, or moved from one day to another.
    if (from.kind === "leftovers") {
      mutate(() => api.dayInfo.set(date, { leftovers: true }));
      return;
    }
    if (from.kind === "leftovers-chip") {
      if (from.date !== date) mutate(() => api.dayInfo.moveLeftovers(from.date, date));
      return;
    }

    // Dropping on a chip takes that chip's place; on a slot it goes to the end of that slot; on
    // the bare day, new things go to their usual slot and moved things keep theirs.
    if (to.kind === "entry" && from.kind === "entry" && from.entryId === to.entryId) return; // on itself
    const slot: Slot | undefined = to.kind === "day" ? undefined : to.slot;
    const position = to.kind === "entry" ? to.index : to.kind === "slot" ? to.count : undefined;

    if (from.kind === "item") {
      const add = () => api.plan.add(from.itemId, date, { slot, position: to.kind === "entry" ? position : undefined });
      if (from.template) addAndOpen(add);
      else mutate(add);
    } else if (from.kind === "blank") {
      addAndOpen(() => api.plan.addBlank(date, { slot, position: to.kind === "entry" ? position : undefined }));
    } else if (from.kind === "entry") {
      mutate(() => api.plan.move(from.entryId, date, position ?? 1000, slot));
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
                  undoLabel={undoLabel}
                  onUndo={undo}
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
      {dayPopover && (
        <DayNotePopover
          key={dayPopover.date}
          date={dayPopover.date}
          anchor={dayPopover.anchor}
          info={dayPopover.info}
          onClose={() => setDayPopover(null)}
          onChanged={reload}
        />
      )}
      {drawerItemId !== null && (
        <ItemDrawer itemId={drawerItemId} onClose={() => setDrawerItemId(null)} onChanged={reload} />
      )}

      <DragOverlay dropAnimation={null}>
        {dragging?.kind === "item" && <CardPreview name={dragging.name} colour={dragging.colour} />}
        {dragging?.kind === "blank" && <BlankPreview />}
        {(dragging?.kind === "leftovers" || dragging?.kind === "leftovers-chip") &&
          (dragging.kind === "leftovers" ? (
            <LeftoversPreview />
          ) : (
            <div className="w-40">
              <LeftoversBody />
            </div>
          ))}
        {dragging?.kind === "entry" && (
          <div className="w-44">
            <ChipBody entry={dragging.entry} />
          </div>
        )}
      </DragOverlay>
    </DndContext>
  );
}
