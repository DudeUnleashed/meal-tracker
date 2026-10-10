import { createContext, useContext } from "react";
import type { PlanEntry, Slot } from "../../lib/api";

/** What a calendar entry chip can ask the planner to do (keeps cells free of data plumbing). */
export interface PlannerActions {
  changeServes: (entry: PlanEntry, serves: number) => void;
  removeEntry: (entry: PlanEntry) => void;
  /** Open the details popover next to the clicked chip. */
  openEntry: (entry: PlanEntry, anchor: DOMRect) => void;
  /** Open the note / leftovers popover for a day. */
  openDay: (date: string, anchor: DOMRect) => void;
  removeLeftovers: (date: string) => void;
  /** Extra slots (lunch etc.) the user has opened on each day, even while they are empty. */
  openSlots: Record<string, Slot[]>;
  showSlot: (date: string, slot: Slot) => void;
}

export const PlannerActionsContext = createContext<PlannerActions>({
  changeServes: () => {},
  removeEntry: () => {},
  openEntry: () => {},
  openDay: () => {},
  removeLeftovers: () => {},
  openSlots: {},
  showSlot: () => {},
});

export const usePlannerActions = () => useContext(PlannerActionsContext);

/** Payload attached to draggables / droppables so the drop handler needs no lookups. */
export type DragData =
  | { kind: "item"; itemId: number; name: string; colour: string | null; template: boolean }
  | { kind: "entry"; entryId: number; date: string; slot: Slot; index: number; entry: PlanEntry }
  | { kind: "day"; date: string }
  /** One part of a day (breakfast, lunch, dinner, snacks); `count` = entries already in it. */
  | { kind: "slot"; date: string; slot: Slot; count: number }
  /** The Blank (takeaway etc.) token dragged from the library. */
  | { kind: "blank" }
  /** The Leftovers tag dragged from the library. */
  | { kind: "leftovers" }
  /** A Leftovers tag already on a day, being moved. */
  | { kind: "leftovers-chip"; date: string };
