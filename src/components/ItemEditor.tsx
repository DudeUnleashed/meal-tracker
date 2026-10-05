import { format } from "date-fns";
import { useEffect, useState } from "react";
import { fromIso, toIso } from "../lib/dates";
import {
  api,
  errorMessage,
  ITEM_TYPES,
  type HistoryEntry,
  type Ingredient,
  type IngredientCategory,
  type Item,
  type ItemType,
} from "../lib/api";
import IngredientForm from "./IngredientForm";
import IngredientPicker from "./IngredientPicker";

interface Line {
  key: number;
  ingredient_id: number | null;
  quantity: string;
}

interface Draft {
  name: string;
  type: ItemType;
  colour: string | null;
  notes: string;
  template_id: number | null;
  lines: Line[];
}

interface Props {
  /** Existing item id, or null to create a new one. */
  itemId: number | null;
  /** Type to start a new item as. */
  defaultType: ItemType;
  ingredients: Ingredient[];
  categories: IngredientCategory[];
  onIngredientCreated: (ingredient: Ingredient) => void;
  onSaved: (item: Item) => void;
  onDuplicated: (item: Item) => void;
  onArchived: (item: Item) => void;
  onDirtyChange: (dirty: boolean) => void;
}

const DEFAULT_COLOURS: Record<ItemType, string> = {
  meal: "#f59e0b",
  meat: "#b91c1c",
  side: "#16a34a",
};

let nextKey = 1;

function toDraft(item: Item): Draft {
  return {
    name: item.name,
    type: item.type,
    colour: item.colour,
    notes: item.notes ?? "",
    template_id: item.template_id,
    lines: item.lines.map((l) => ({
      key: nextKey++,
      ingredient_id: l.ingredient_id,
      quantity: String(l.quantity),
    })),
  };
}

function blankDraft(type: ItemType): Draft {
  return {
    name: "",
    type,
    colour: DEFAULT_COLOURS[type],
    notes: "",
    template_id: null,
    lines: [{ key: nextKey++, ingredient_id: null, quantity: "" }],
  };
}

/** Comparable form of a draft, ignoring line keys. */
function snapshot(d: Draft): string {
  return JSON.stringify({ ...d, lines: d.lines.map(({ key: _k, ...l }) => l) });
}

export default function ItemEditor({
  itemId,
  defaultType,
  ingredients,
  categories,
  onIngredientCreated,
  onSaved,
  onDuplicated,
  onArchived,
  onDirtyChange,
}: Props) {
  const [item, setItem] = useState<Item | null>(null);
  const [draft, setDraft] = useState<Draft | null>(itemId === null ? blankDraft(defaultType) : null);
  const [saved, setSaved] = useState<string>(
    itemId === null ? snapshot(blankDraft(defaultType)) : "",
  );
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // Which line is creating a new ingredient, and the name to start with.
  const [creating, setCreating] = useState<{ key: number; name: string } | null>(null);

  // Load the existing item (the parent re-mounts this component per item via `key`).
  useEffect(() => {
    if (itemId === null) return;
    let cancelled = false;
    api.items
      .get(itemId)
      .then((it) => {
        if (cancelled) return;
        const d = toDraft(it);
        setItem(it);
        setDraft(d);
        setSaved(snapshot(d));
      })
      .catch((e) => !cancelled && setError(errorMessage(e)));
    return () => {
      cancelled = true;
    };
  }, [itemId]);

  const [history, setHistory] = useState<HistoryEntry[]>([]);
  useEffect(() => {
    if (itemId === null) return;
    api.items
      .history(itemId)
      .then(setHistory)
      .catch(() => setHistory([]));
  }, [itemId]);

  const dirty = draft !== null && snapshot(draft) !== saved;
  useEffect(() => {
    onDirtyChange(dirty);
  }, [dirty, onDirtyChange]);
  useEffect(() => () => onDirtyChange(false), [onDirtyChange]);

  if (!draft) {
    return <p className="p-4 text-sm text-slate-500">{error ?? "Loading…"}</p>;
  }

  const update = (patch: Partial<Draft>) => setDraft({ ...draft, ...patch });
  const updateLine = (key: number, patch: Partial<Line>) =>
    update({ lines: draft.lines.map((l) => (l.key === key ? { ...l, ...patch } : l)) });
  const moveLine = (index: number, delta: number) => {
    const lines = [...draft.lines];
    const target = index + delta;
    if (target < 0 || target >= lines.length) return;
    [lines[index], lines[target]] = [lines[target], lines[index]];
    update({ lines });
  };
  const unitOf = (id: number | null) => ingredients.find((i) => i.id === id)?.unit ?? "";

  function buildInput() {
    if (!draft) return null;
    if (!draft.name.trim()) {
      setError("Give the item a name");
      return null;
    }
    const lines: { ingredient_id: number; quantity: number }[] = [];
    for (const l of draft.lines) {
      if (l.ingredient_id === null && l.quantity.trim() === "") continue; // untouched blank row
      const quantity = Number(l.quantity);
      if (l.ingredient_id === null) {
        setError("Pick an ingredient for every line (or remove the empty one)");
        return null;
      }
      if (!Number.isFinite(quantity) || quantity <= 0) {
        setError("Every ingredient needs a quantity greater than zero");
        return null;
      }
      lines.push({ ingredient_id: l.ingredient_id, quantity });
    }
    return {
      name: draft.name,
      type: draft.type,
      colour: draft.colour,
      notes: draft.notes.trim() === "" ? null : draft.notes,
      template_id: draft.template_id,
      lines,
    };
  }

  async function run(action: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const save = () =>
    run(async () => {
      const input = buildInput();
      if (!input) return;
      const result =
        itemId === null ? await api.items.create(input) : await api.items.update(itemId, input);
      const d = toDraft(result);
      setItem(result);
      setDraft(d);
      setSaved(snapshot(d));
      onSaved(result);
    });

  const duplicate = () =>
    run(async () => {
      if (itemId !== null) onDuplicated(await api.items.duplicate(itemId));
    });

  const toggleArchive = () =>
    run(async () => {
      if (itemId !== null && item) onArchived(await api.items.archive(itemId, !item.archived));
    });

  const isMeat = draft.type === "meat";
  const field = "w-full rounded border border-slate-300 px-2 py-1 text-sm";

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 space-y-4 overflow-auto p-4">
        {item?.archived && (
          <p className="rounded bg-amber-50 px-3 py-2 text-sm text-amber-800">
            This item is archived — hidden from the planner library but still shown on past plans.
          </p>
        )}

        <div className="flex gap-3">
          <label className="flex-1 text-xs text-slate-500">
            Name
            <input
              value={draft.name}
              onChange={(e) => update({ name: e.target.value })}
              placeholder={isMeat ? "e.g. Sausages" : "e.g. Chow mein"}
              className={`${field} text-base text-slate-900`}
            />
          </label>
          <label className="text-xs text-slate-500">
            Type
            <select
              value={draft.type}
              onChange={(e) => update({ type: e.target.value as ItemType })}
              className={`${field} capitalize text-slate-900`}
            >
              {ITEM_TYPES.map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </select>
          </label>
          <label className="text-xs text-slate-500">
            Colour
            <div className="flex items-center gap-1">
              <input
                type="color"
                value={draft.colour ?? DEFAULT_COLOURS[draft.type]}
                onChange={(e) => update({ colour: e.target.value })}
                className="h-8 w-10 cursor-pointer rounded border border-slate-300"
              />
              {draft.colour && (
                <button
                  type="button"
                  onClick={() => update({ colour: null })}
                  title="Clear colour"
                  className="px-1 text-slate-400 hover:text-slate-700"
                >
                  ✕
                </button>
              )}
            </div>
          </label>
        </div>

        <label className="block text-xs text-slate-500">
          Notes
          <textarea
            value={draft.notes}
            onChange={(e) => update({ notes: e.target.value })}
            rows={2}
            placeholder="Permanent notes about this item"
            className={`${field} text-slate-900`}
          />
        </label>

        <section>
          <div className="mb-1 flex items-baseline justify-between">
            <h3 className="text-sm font-semibold">Ingredients</h3>
            <span className="text-xs text-slate-500">
              {isMeat
                ? "Quantities are per serve — scaled by serves on the calendar"
                : "Quantities are the fixed amount you cook"}
            </span>
          </div>

          <div className="space-y-2">
            {draft.lines.map((line, index) => (
              <div key={line.key}>
                <div className="flex items-center gap-2">
                  <div className="flex-1">
                    <IngredientPicker
                      ingredients={ingredients}
                      value={line.ingredient_id}
                      onChange={(i) => updateLine(line.key, { ingredient_id: i.id })}
                      onCreate={(name) => setCreating({ key: line.key, name })}
                    />
                  </div>
                  <input
                    type="number"
                    min="0"
                    step="any"
                    value={line.quantity}
                    onChange={(e) => updateLine(line.key, { quantity: e.target.value })}
                    placeholder="Qty"
                    className="w-24 rounded border border-slate-300 px-2 py-1 text-right text-sm"
                  />
                  <span className="w-20 text-xs text-slate-500">
                    {unitOf(line.ingredient_id)}
                    {isMeat && unitOf(line.ingredient_id) ? " / serve" : ""}
                  </span>
                  <button
                    type="button"
                    onClick={() => moveLine(index, -1)}
                    disabled={index === 0}
                    title="Move up"
                    className="px-1 text-slate-400 hover:text-slate-800 disabled:opacity-30"
                  >
                    ↑
                  </button>
                  <button
                    type="button"
                    onClick={() => moveLine(index, 1)}
                    disabled={index === draft.lines.length - 1}
                    title="Move down"
                    className="px-1 text-slate-400 hover:text-slate-800 disabled:opacity-30"
                  >
                    ↓
                  </button>
                  <button
                    type="button"
                    onClick={() => update({ lines: draft.lines.filter((l) => l.key !== line.key) })}
                    title="Remove"
                    className="px-1 text-slate-400 hover:text-red-600"
                  >
                    ✕
                  </button>
                </div>
                {creating?.key === line.key && (
                  <div className="mt-2 rounded border border-emerald-200 bg-emerald-50 p-2">
                    <IngredientForm
                      categories={categories}
                      initialName={creating.name}
                      onSaved={(ing) => {
                        onIngredientCreated(ing);
                        updateLine(line.key, { ingredient_id: ing.id });
                        setCreating(null);
                      }}
                      onCancel={() => setCreating(null)}
                    />
                  </div>
                )}
              </div>
            ))}
          </div>

          <button
            type="button"
            onClick={() =>
              update({ lines: [...draft.lines, { key: nextKey++, ingredient_id: null, quantity: "" }] })
            }
            className="mt-2 text-sm font-medium text-emerald-700 hover:underline"
          >
            + Add ingredient
          </button>
        </section>

        {itemId !== null && (
          <section>
            <h3 className="mb-1 text-sm font-semibold">History</h3>
            {history.length === 0 ? (
              <p className="text-sm text-slate-400">Not planned yet.</p>
            ) : (
              <ul className="max-h-56 divide-y divide-slate-100 overflow-auto rounded border border-slate-200 bg-white text-sm">
                {history.map((h) => (
                  <li key={h.entry_id} className="px-3 py-1.5">
                    <div className="flex items-baseline gap-2">
                      <span>{format(fromIso(h.date), "EEE d MMM yyyy")}</span>
                      {h.serves !== null && (
                        <span className="text-xs text-slate-500">{h.serves} serves</span>
                      )}
                      {h.date >= toIso(new Date()) && (
                        <span className="rounded bg-emerald-50 px-1.5 text-xs text-emerald-700">
                          planned
                        </span>
                      )}
                    </div>
                    {h.note && <div className="text-xs text-amber-800">📝 {h.note}</div>}
                  </li>
                ))}
              </ul>
            )}
          </section>
        )}
      </div>

      <footer className="flex items-center gap-2 border-t border-slate-200 bg-white px-4 py-3">
        <button
          type="button"
          onClick={save}
          disabled={busy || !dirty}
          className="rounded bg-emerald-600 px-4 py-1.5 text-sm font-medium text-white hover:bg-emerald-700 disabled:opacity-50"
        >
          {itemId === null ? "Create item" : "Save"}
        </button>
        {itemId !== null && (
          <>
            <button
              type="button"
              onClick={duplicate}
              disabled={busy || dirty}
              title={dirty ? "Save your changes first" : undefined}
              className="rounded border border-slate-300 px-3 py-1.5 text-sm hover:bg-slate-50 disabled:opacity-50"
            >
              Duplicate
            </button>
            <button
              type="button"
              onClick={toggleArchive}
              disabled={busy || dirty}
              title={dirty ? "Save your changes first" : undefined}
              className="rounded border border-slate-300 px-3 py-1.5 text-sm hover:bg-slate-50 disabled:opacity-50"
            >
              {item?.archived ? "Restore" : "Archive"}
            </button>
          </>
        )}
        {dirty && <span className="text-xs text-amber-600">Unsaved changes</span>}
        {error && <span className="ml-auto text-sm text-red-600">{error}</span>}
      </footer>
    </div>
  );
}
