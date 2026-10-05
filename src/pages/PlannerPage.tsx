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
import ItemLibrary, { CardPreview } from "../components/ItemLibrary";
import { api, errorMessage, type PlanEntry } from "../lib/api";
import type { WeekStart } from "../lib/dates";

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
  const [refreshKey, setRefreshKey] = useState(0);
  const [dragging, setDragging] = useState<DragData | null>(null);

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
    }),
    [mutate],
  );

  function onDragStart(e: DragStartEvent) {
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
            <div className="min-h-0 flex-1">
              {weekStart === null ? (
                <p className="p-4 text-sm text-slate-500">{error ?? "Loading…"}</p>
              ) : (
                <Calendar
                  key={weekStart}
                  weekStart={weekStart}
                  view={view}
                  onViewChange={changeView}
                  refreshKey={refreshKey}
                />
              )}
            </div>
          </section>
          <ItemLibrary />
        </div>
      </PlannerActionsContext.Provider>

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
