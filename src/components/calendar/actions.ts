import { createContext, useContext } from "react";
import type { PlanEntry } from "../../lib/api";

/** What a calendar entry chip can ask the planner to do (keeps cells free of data plumbing). */
export interface PlannerActions {
  changeServes: (entry: PlanEntry, serves: number) => void;
  removeEntry: (entry: PlanEntry) => void;
  /** Open the details popover next to the clicked chip. */
  openEntry: (entry: PlanEntry, anchor: DOMRect) => void;
  /** Open the note / leftovers popover for a day. */
  openDay: (date: string, anchor: DOMRect) => void;
  removeLeftovers: (date: string) => void;
}

export const PlannerActionsContext = createContext<PlannerActions>({
  changeServes: () => {},
  removeEntry: () => {},
  openEntry: () => {},
  openDay: () => {},
  removeLeftovers: () => {},
});

export const usePlannerActions = () => useContext(PlannerActionsContext);

/** Payload attached to draggables / droppables so the drop handler needs no lookups. */
export type DragData =
  | { kind: "item"; itemId: number; name: string; colour: string | null }
  | { kind: "entry"; entryId: number; date: string; index: number; entry: PlanEntry }
  | { kind: "day"; date: string; count: number }
  /** The Leftovers tag dragged from the library. */
  | { kind: "leftovers" }
  /** A Leftovers tag already on a day, being moved. */
  | { kind: "leftovers-chip"; date: string };
