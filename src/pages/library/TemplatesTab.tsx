import { useCallback, useEffect, useState } from "react";
import IngredientForm from "../../components/IngredientForm";
import IngredientPicker from "../../components/IngredientPicker";
import QuantityInput from "../../components/QuantityInput";
import { parseQuantity } from "../../lib/quantity";
import {
  api,
  errorMessage,
  UNITS,
  type Ingredient,
  type IngredientCategory,
  type Template,
  type TemplateSummary,
  type TemplateType,
  type Unit,
  TEMPLATE_TYPES,
  typeLabel,
} from "../../lib/api";

interface Props {
  ingredients: Ingredient[];
  categories: IngredientCategory[];
  onIngredientCreated: (ingredient: Ingredient) => void;
}

// null = nothing selected, "new" = creating, number = editing that template.
type Selection = number | "new" | null;

/**
 * Templates are reusable chips with choices: fixed ingredients that are always there, plus slots
 * (e.g. "Veg", "Protein") whose ingredients you pick for each night on the planner.
 */
export default function TemplatesTab({ ingredients, categories, onIngredientCreated }: Props) {
  const [templates, setTemplates] = useState<TemplateSummary[]>([]);
  const [selection, setSelection] = useState<Selection>(null);
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    api.templates
      .list()
      .then(setTemplates)
      .catch((e) => setError(errorMessage(e)));
  }, []);

  useEffect(reload, [reload]);

  function select(next: Selection) {
    if (dirty && !window.confirm("Discard your unsaved changes?")) return;
    setDirty(false);
    setSelection(next);
  }

  const handleDirty = useCallback((d: boolean) => setDirty(d), []);

  return (
    <div className="flex h-full min-h-0">
      <aside className="flex w-72 flex-col border-r border-slate-200 bg-white">
        <div className="flex items-center justify-between border-b border-slate-200 p-3">
          <span className="text-sm font-semibold">Templates</span>
          <button
            onClick={() => select("new")}
            className="rounded bg-emerald-600 px-2 py-1 text-xs font-medium text-white hover:bg-emerald-700"
          >
            + New template
          </button>
        </div>
        <ul className="flex-1 overflow-auto">
          {templates.map((t) => (
            <li key={t.id}>
              <button
                onClick={() => select(t.id)}
                className={`w-full px-3 py-2 text-left text-sm ${
                  selection === t.id ? "bg-emerald-50" : "hover:bg-slate-50"
                }`}
              >
                <div>
                  🧩 {t.name} <span className="text-xs text-slate-400">· {typeLabel(t.item_type)}</span>
                </div>
                <div className="text-xs text-slate-400">
                  {t.fixed_count} fixed · {t.slot_count} {t.slot_count === 1 ? "choice" : "choices"}
                </div>
              </button>
            </li>
          ))}
          {templates.length === 0 && (
            <li className="p-4 text-sm text-slate-400">{error ?? "No templates yet."}</li>
          )}
        </ul>
        <p className="border-t border-slate-100 p-3 text-xs text-slate-400">
          Each template appears in the planner’s library with a 🧩. Drag it onto a day, then pick its
          ingredients for that night. (Meals can still be started from a template in the meal editor.)
        </p>
      </aside>

      <section className="min-w-0 flex-1 bg-slate-50">
        {selection === null ? (
          <p className="p-6 text-sm text-slate-500">
            Select a template to edit it, or create a new one.
          </p>
        ) : (
          <TemplateEditor
            key={selection}
            templateId={selection === "new" ? null : selection}
            ingredients={ingredients}
            categories={categories}
            onIngredientCreated={onIngredientCreated}
            onDirtyChange={handleDirty}
            onSaved={(t) => {
              setDirty(false);
              setSelection(t.id);
              reload();
            }}
            onDeleted={() => {
              setDirty(false);
              setSelection(null);
              reload();
            }}
          />
        )}
      </section>
    </div>
  );
}

interface Option {
  key: number;
  ingredient_id: number | null;
  quantity: string;
}

interface Line {
  key: number;
  /** Existing line being edited (kept so nights that picked from it stay attached). */
  id?: number;
  kind: "fixed" | "slot";
  label: string;
  ingredient_id: number | null;
  category_id: number | null;
  quantity: string;
  unit_hint: Unit | "";
  multi: boolean;
  options: Option[];
}

interface Draft {
  name: string;
  item_type: TemplateType;
  colour: string | null;
  lines: Line[];
}

let nextKey = 1;

const blankLine = (kind: Line["kind"]): Line => ({
  key: nextKey++,
  kind,
  label: "",
  ingredient_id: null,
  category_id: null,
  quantity: "",
  unit_hint: "",
  multi: false,
  options: [],
});

function toDraft(t: Template): Draft {
  return {
    name: t.name,
    item_type: t.item_type,
    colour: t.colour,
    lines: t.lines.map((l) => ({
      key: nextKey++,
      id: l.id,
      kind: l.ingredient_id === null ? "slot" : "fixed",
      label: l.label,
      ingredient_id: l.ingredient_id,
      category_id: l.category_id,
      quantity: l.quantity === null ? "" : String(l.quantity),
      unit_hint: l.unit_hint ?? "",
      multi: l.multi,
      options: l.options.map((o) => ({
        key: nextKey++,
        ingredient_id: o.ingredient_id,
        quantity: o.quantity === null ? "" : String(o.quantity),
      })),
    })),
  };
}

const snapshot = (d: Draft): string =>
  JSON.stringify({
    ...d,
    lines: d.lines.map(({ key: _k, options, ...l }) => ({
      ...l,
      options: options.map(({ key: _o, ...o }) => o),
    })),
  });

interface EditorProps {
  templateId: number | null;
  ingredients: Ingredient[];
  categories: IngredientCategory[];
  onIngredientCreated: (ingredient: Ingredient) => void;
  onDirtyChange: (dirty: boolean) => void;
  onSaved: (template: Template) => void;
  onDeleted: () => void;
}

function TemplateEditor({
  templateId,
  ingredients,
  categories,
  onIngredientCreated,
  onDirtyChange,
  onSaved,
  onDeleted,
}: EditorProps) {
  const fresh: Draft = { name: "", item_type: "meal", colour: null, lines: [blankLine("slot")] };
  const [draft, setDraft] = useState<Draft | null>(templateId === null ? fresh : null);
  const [saved, setSaved] = useState(templateId === null ? snapshot(fresh) : "");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // The line (and, for a choice, which option) that is creating a new ingredient.
  const [creating, setCreating] = useState<{ key: number; optKey?: number; name: string } | null>(null);

  useEffect(() => {
    if (templateId === null) return;
    let cancelled = false;
    api.templates
      .get(templateId)
      .then((t) => {
        if (cancelled) return;
        const d = toDraft(t);
        setDraft(d);
        setSaved(snapshot(d));
      })
      .catch((e) => !cancelled && setError(errorMessage(e)));
    return () => {
      cancelled = true;
    };
  }, [templateId]);

  // A brand-new template counts as dirty as soon as anything is typed.
  const dirty = draft !== null && snapshot(draft) !== saved;
  useEffect(() => {
    onDirtyChange(dirty);
  }, [dirty, onDirtyChange]);
  useEffect(() => () => onDirtyChange(false), [onDirtyChange]);

  if (!draft) return <p className="p-4 text-sm text-slate-500">{error ?? "Loading…"}</p>;

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
  const updateOption = (lineKey: number, optKey: number, patch: Partial<Option>) =>
    update({
      lines: draft.lines.map((l) =>
        l.key === lineKey ? { ...l, options: l.options.map((o) => (o.key === optKey ? { ...o, ...patch } : o)) } : l,
      ),
    });

  function buildInput() {
    if (!draft) return null;
    if (!draft.name.trim()) {
      setError("Give the template a name");
      return null;
    }
    const lines = [];
    for (const l of draft.lines) {
      if (!l.label.trim()) {
        setError("Every line needs a label (e.g. “Protein” or “Arborio rice”)");
        return null;
      }
      let quantity: number | null = null;
      if (l.quantity.trim() !== "") {
        const parsed = parseQuantity(l.quantity, l.kind === "fixed" ? unitOf(l.ingredient_id) : l.unit_hint);
        if (!parsed.ok) {
          setError(`“${l.label}”: ${parsed.error}`);
          return null;
        }
        quantity = parsed.value;
      }
      if (l.kind === "fixed") {
        if (l.ingredient_id === null || quantity === null) {
          setError(`“${l.label}” is a fixed line: pick an ingredient and a quantity`);
          return null;
        }
        lines.push({
          id: l.id,
          label: l.label,
          multi: false,
          options: [],
          ingredient_id: l.ingredient_id,
          category_id: null,
          quantity,
          unit_hint: null,
        });
      } else {
        const options = [];
        for (const o of l.options) {
          if (o.ingredient_id === null) {
            setError(`“${l.label}”: pick an ingredient for every choice (or remove the empty one)`);
            return null;
          }
          let amount: number | null = null;
          if (o.quantity.trim() !== "") {
            const parsed = parseQuantity(o.quantity, unitOf(o.ingredient_id));
            if (!parsed.ok) {
              setError(`“${l.label}”: ${parsed.error}`);
              return null;
            }
            amount = parsed.value;
          }
          options.push({ ingredient_id: o.ingredient_id, quantity: amount });
        }
        lines.push({
          id: l.id,
          label: l.label,
          multi: l.multi,
          options,
          ingredient_id: null,
          category_id: l.category_id,
          quantity,
          unit_hint: l.unit_hint === "" ? null : l.unit_hint,
        });
      }
    }
    return { name: draft.name, item_type: draft.item_type, colour: draft.colour, lines };
  }

  async function save() {
    const input = buildInput();
    if (!input) return;
    setBusy(true);
    setError(null);
    try {
      onSaved(
        templateId === null
          ? await api.templates.create(input)
          : await api.templates.update(templateId, input),
      );
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (templateId === null) return;
    if (
      !window.confirm(
        `Delete “${draft?.name}”? Nights already planned with it stay on the calendar and shopping list; meals made from it keep their ingredients.`,
      )
    )
      return;
    try {
      await api.templates.delete(templateId);
      onDeleted();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  const field = "rounded border border-slate-300 px-2 py-1 text-sm";

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 space-y-4 overflow-auto p-4">
        <label className="block text-xs text-slate-500">
          Template name
          <input
            value={draft.name}
            onChange={(e) => update({ name: e.target.value })}
            placeholder="e.g. Risotto"
            className={`${field} w-full text-base text-slate-900`}
          />
        </label>

        <div className="flex flex-wrap items-end gap-4">
          <label className="text-xs text-slate-500">
            Counts as
            <select
              value={draft.item_type}
              onChange={(e) => update({ item_type: e.target.value as TemplateType })}
              className={`${field} block text-slate-900`}
            >
              {TEMPLATE_TYPES.map((t) => (
                <option key={t} value={t}>
                  {typeLabel(t)}
                </option>
              ))}
            </select>
          </label>
          <label className="text-xs text-slate-500">
            Colour
            <div className="flex items-center gap-1">
              <input
                type="color"
                value={draft.colour ?? "#16a34a"}
                onChange={(e) => update({ colour: e.target.value })}
                className="h-8 w-10 cursor-pointer rounded border border-slate-300"
              />
              {draft.colour && (
                <button type="button" onClick={() => update({ colour: null })} className="text-xs text-slate-400 hover:text-slate-700">
                  clear
                </button>
              )}
            </div>
          </label>
        </div>

        <section>
          <div className="mb-1 flex items-baseline justify-between">
            <h3 className="text-sm font-semibold">Lines</h3>
            <span className="text-xs text-slate-500">
              <b>Fixed</b> lines are always included; a <b>choice</b> (e.g. Veg, Protein) is picked for
              each night on the planner.
            </span>
          </div>

          <div className="space-y-2">
            {draft.lines.map((line, index) => (
              <div key={line.key} className="rounded border border-slate-200 bg-white p-2">
                <div className="flex items-center gap-2">
                  <input
                    value={line.label}
                    onChange={(e) => updateLine(line.key, { label: e.target.value })}
                    placeholder={line.kind === "slot" ? "Label, e.g. Protein" : "Label"}
                    className={`${field} w-36`}
                  />
                  <select
                    value={line.kind}
                    onChange={(e) =>
                      updateLine(line.key, {
                        kind: e.target.value as Line["kind"],
                        // Each kind owns different fields; don't leave stale ones behind.
                        ingredient_id: null,
                        category_id: null,
                        unit_hint: "",
                      })
                    }
                    className={field}
                  >
                    <option value="fixed">Fixed ingredient</option>
                    <option value="slot">Choice per night</option>
                  </select>

                  {line.kind === "fixed" ? (
                    <div className="min-w-0 flex-1">
                      <IngredientPicker
                        ingredients={ingredients}
                        value={line.ingredient_id}
                        onChange={(i) => updateLine(line.key, { ingredient_id: i.id })}
                        onCreate={(name) => setCreating({ key: line.key, name })}
                      />
                    </div>
                  ) : (
                    <select
                      value={line.category_id ?? ""}
                      onChange={(e) =>
                        updateLine(line.key, { category_id: e.target.value === "" ? null : Number(e.target.value) })
                      }
                      className={`${field} min-w-0 flex-1`}
                      aria-label="Ingredient category for this slot"
                    >
                      <option value="">Any category</option>
                      {categories.map((c) => (
                        <option key={c.id} value={c.id}>
                          {c.name} only
                        </option>
                      ))}
                    </select>
                  )}

                  <QuantityInput
                    value={line.quantity}
                    unit={line.kind === "fixed" ? unitOf(line.ingredient_id) : line.unit_hint}
                    onChange={(text) => updateLine(line.key, { quantity: text })}
                    placeholder={line.kind === "slot" ? "Default" : "Qty"}
                    className={`${field} w-24 text-right`}
                  />
                  {line.kind === "fixed" ? (
                    <span className="w-14 text-xs text-slate-500">{unitOf(line.ingredient_id)}</span>
                  ) : (
                    <select
                      value={line.unit_hint}
                      onChange={(e) => updateLine(line.key, { unit_hint: e.target.value as Unit | "" })}
                      className={`${field} w-16`}
                      aria-label="Unit for the default quantity"
                    >
                      <option value="">unit</option>
                      {UNITS.map((u) => (
                        <option key={u} value={u}>
                          {u}
                        </option>
                      ))}
                    </select>
                  )}

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
                {creating?.key === line.key && creating.optKey === undefined && (
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

                {line.kind === "slot" && (
                  <div className="mt-2 space-y-2 border-t border-slate-100 pt-2">
                    <label className="flex items-center gap-2 text-xs text-slate-600">
                      <input
                        type="checkbox"
                        checked={line.multi}
                        onChange={(e) => updateLine(line.key, { multi: e.target.checked })}
                      />
                      Tick several each night (e.g. steamed veg: carrot <i>and</i> corn)
                    </label>
                    <div className="space-y-1">
                      <div className="text-xs text-slate-500">
                        Choices <span className="text-slate-400">— each with its usual amount. Leave empty to allow any
                        ingredient in the category.</span>
                      </div>
                      {line.options.map((o) => (
                        <div key={o.key}>
                          <div className="flex items-center gap-2">
                            <div className="min-w-0 flex-1">
                              <IngredientPicker
                                ingredients={ingredients}
                                value={o.ingredient_id}
                                onChange={(i) => updateOption(line.key, o.key, { ingredient_id: i.id })}
                                onCreate={(name) => setCreating({ key: line.key, optKey: o.key, name })}
                              />
                            </div>
                            <QuantityInput
                              value={o.quantity}
                              unit={unitOf(o.ingredient_id)}
                              onChange={(text) => updateOption(line.key, o.key, { quantity: text })}
                              placeholder="Amount"
                              className={`${field} w-24 text-right`}
                            />
                            <span className="w-10 text-xs text-slate-500">{unitOf(o.ingredient_id)}</span>
                            <button
                              type="button"
                              onClick={() =>
                                updateLine(line.key, { options: line.options.filter((x) => x.key !== o.key) })
                              }
                              title="Remove choice"
                              className="px-1 text-slate-400 hover:text-red-600"
                            >
                              ✕
                            </button>
                          </div>
                          {creating?.key === line.key && creating.optKey === o.key && (
                            <div className="mt-1 rounded border border-emerald-200 bg-emerald-50 p-2">
                              <IngredientForm
                                categories={categories}
                                initialName={creating.name}
                                initialCategoryId={line.category_id}
                                onSaved={(ing) => {
                                  onIngredientCreated(ing);
                                  updateOption(line.key, o.key, { ingredient_id: ing.id });
                                  setCreating(null);
                                }}
                                onCancel={() => setCreating(null)}
                              />
                            </div>
                          )}
                        </div>
                      ))}
                      <button
                        type="button"
                        onClick={() =>
                          updateLine(line.key, {
                            options: [...line.options, { key: nextKey++, ingredient_id: null, quantity: "" }],
                          })
                        }
                        className="text-xs font-medium text-emerald-700 hover:underline"
                      >
                        + Add choice
                      </button>
                    </div>
                  </div>
                )}
              </div>
            ))}
          </div>

          <div className="mt-2 flex gap-4">
            <button
              type="button"
              onClick={() => update({ lines: [...draft.lines, blankLine("fixed")] })}
              className="text-sm font-medium text-emerald-700 hover:underline"
            >
              + Fixed line
            </button>
            <button
              type="button"
              onClick={() => update({ lines: [...draft.lines, blankLine("slot")] })}
              className="text-sm font-medium text-emerald-700 hover:underline"
            >
              + Choice per night
            </button>
          </div>
        </section>
      </div>

      <footer className="flex items-center gap-2 border-t border-slate-200 bg-white px-4 py-3">
        <button
          type="button"
          onClick={save}
          disabled={busy || !dirty}
          className="rounded bg-emerald-600 px-4 py-1.5 text-sm font-medium text-white hover:bg-emerald-700 disabled:opacity-50"
        >
          {templateId === null ? "Create template" : "Save"}
        </button>
        {templateId !== null && (
          <button
            type="button"
            onClick={remove}
            disabled={busy}
            className="rounded border border-slate-300 px-3 py-1.5 text-sm text-red-600 hover:bg-red-50"
          >
            Delete
          </button>
        )}
        {dirty && <span className="text-xs text-amber-600">Unsaved changes</span>}
        {error && <span className="ml-auto text-sm text-red-600">{error}</span>}
      </footer>
    </div>
  );
}
