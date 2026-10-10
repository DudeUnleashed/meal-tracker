import { useEffect, useState } from "react";
import {
  api,
  errorMessage,
  type Ingredient,
  type Pick,
  type PickInput,
  type PlanEntry,
  type Template,
  type TemplateLine,
  type Unit,
} from "../lib/api";
import { formatQuantity } from "../lib/format";
import { parseQuantity } from "../lib/quantity";
import IngredientPicker from "./IngredientPicker";
import QuantityInput from "./QuantityInput";

interface Props {
  entry: PlanEntry;
  /** The entry after a change was saved. */
  onChange: (entry: PlanEntry) => void;
}

interface Choice {
  id: number;
  name: string;
  unit: Unit;
}

const asInput = (p: Pick): PickInput => ({
  line_id: p.line_id,
  ingredient_id: p.ingredient_id,
  quantity: p.quantity,
});

/**
 * The per-night choices for a template chip: tick the veg for tonight, set the protein.
 * Each tick saves straight away, so the calendar chip and shopping list follow along.
 */
export default function TemplatePicks({ entry, onChange }: Props) {
  const [template, setTemplate] = useState<Template | null>(null);
  const [ingredients, setIngredients] = useState<Ingredient[]>([]);
  const [error, setError] = useState<string | null>(null);
  // A choice the template has no amount for: ask before adding it.
  const [pending, setPending] = useState<{ line: TemplateLine; choice: Choice; amount: string } | null>(null);

  useEffect(() => {
    if (entry.template_id === null) return;
    Promise.all([api.templates.get(entry.template_id), api.ingredients.list()])
      .then(([t, ings]) => {
        setTemplate(t);
        setIngredients(ings);
      })
      .catch((e) => setError(errorMessage(e)));
  }, [entry.template_id]);

  async function save(next: PickInput[]): Promise<boolean> {
    try {
      setError(null);
      onChange(await api.plan.setPicks(entry.id, next));
      return true;
    } catch (e) {
      setError(errorMessage(e));
      return false;
    }
  }

  /** Adds a choice to a slot (a single-choice slot swaps its previous pick). */
  async function add(line: TemplateLine, choice: Choice, quantity?: number) {
    const keep = entry.picks.filter((p) => p.line_id !== line.id || line.multi).map(asInput);
    const next: PickInput[] = [...keep, { line_id: line.id, ingredient_id: choice.id, quantity }];
    try {
      onChange(await api.plan.setPicks(entry.id, next));
      setError(null);
      setPending(null);
    } catch (e) {
      const msg = errorMessage(e);
      if (msg.startsWith("Set an amount")) setPending({ line, choice, amount: "" });
      else setError(msg);
    }
  }

  const remove = (pick: Pick) => save(entry.picks.filter((p) => p !== pick).map(asInput));

  async function setAmount(pick: Pick, text: string) {
    const r = parseQuantity(text, pick.unit);
    if (!r.ok) return setError(r.error);
    if (r.value === pick.quantity) return;
    await save(entry.picks.map((p) => (p === pick ? { ...asInput(p), quantity: r.value } : asInput(p))));
  }

  if (!template) return <p className="text-xs text-slate-400">{error ?? "Loading choices…"}</p>;

  const slots = template.lines.filter((l) => l.ingredient_id === null);
  const fixed = template.lines.filter((l) => l.ingredient_id !== null);
  const strayPicks = entry.picks.filter((p) => !slots.some((l) => l.id === p.line_id));

  return (
    <div className="space-y-3">
      {slots.map((line) => {
        const mine = entry.picks.filter((p) => p.line_id === line.id);
        return (
          <div key={line.id}>
            <div className="mb-1 text-xs font-medium text-slate-600">
              {line.label}
              {line.multi && <span className="font-normal text-slate-400"> — tick any</span>}
            </div>
            {line.options.length > 0 ? (
              <div className="flex flex-wrap gap-1">
                {line.options.map((o) => {
                  const on = mine.find((p) => p.ingredient_id === o.ingredient_id);
                  return (
                    <button
                      key={o.id}
                      type="button"
                      aria-pressed={!!on}
                      onClick={() =>
                        on
                          ? remove(on)
                          : add(line, { id: o.ingredient_id, name: o.ingredient_name, unit: o.ingredient_unit })
                      }
                      className={`rounded-full border px-2 py-0.5 text-xs ${
                        on
                          ? "border-emerald-500 bg-emerald-50 font-medium text-emerald-800"
                          : "border-slate-300 text-slate-600 hover:bg-slate-50"
                      }`}
                    >
                      {on ? "✓ " : ""}
                      {o.ingredient_name}
                    </button>
                  );
                })}
              </div>
            ) : (
              <IngredientPicker
                ingredients={ingredients}
                categoryId={line.category_id}
                value={null}
                onChange={(i) => add(line, { id: i.id, name: i.name, unit: i.unit })}
              />
            )}

            {mine.map((p) => (
              <PickRow key={p.ingredient_id} pick={p} onAmount={(t) => setAmount(p, t)} onRemove={() => remove(p)} />
            ))}

            {pending?.line.id === line.id && (
              <div className="mt-1 flex items-center gap-1 rounded bg-amber-50 p-1.5 text-xs text-amber-900">
                <span className="flex-1">How much {pending.choice.name}?</span>
                <QuantityInput
                  value={pending.amount}
                  unit={pending.choice.unit}
                  onChange={(amount) => setPending({ ...pending, amount })}
                  className="w-16 rounded border border-slate-300 px-1 py-0.5 text-right text-xs"
                />
                <span className="text-slate-500">{pending.choice.unit}</span>
                <button
                  type="button"
                  onClick={() => {
                    const r = parseQuantity(pending.amount, pending.choice.unit);
                    if (!r.ok) return setError(r.error);
                    add(pending.line, pending.choice, r.value);
                  }}
                  className="rounded bg-emerald-600 px-2 py-0.5 text-white"
                >
                  Add
                </button>
                <button type="button" onClick={() => setPending(null)} className="px-1 text-slate-500">
                  ✕
                </button>
              </div>
            )}
          </div>
        );
      })}

      {strayPicks.map((p) => (
        <PickRow key={p.ingredient_id} pick={p} onAmount={(t) => setAmount(p, t)} onRemove={() => remove(p)} />
      ))}

      {slots.length === 0 && (
        <p className="text-xs text-slate-400">This template has no choices — just its fixed ingredients.</p>
      )}
      {fixed.length > 0 && (
        <p className="text-xs text-slate-400">
          Always included:{" "}
          {fixed.map((l) => `${l.ingredient_name} ${formatQuantity(l.quantity ?? 0, l.ingredient_unit!)}`).join(", ")}
        </p>
      )}
      {error && <p className="text-xs text-red-600">{error}</p>}
    </div>
  );
}

function PickRow({
  pick,
  onAmount,
  onRemove,
}: {
  pick: Pick;
  onAmount: (text: string) => void;
  onRemove: () => void;
}) {
  const [text, setText] = useState(String(pick.quantity));
  useEffect(() => setText(String(pick.quantity)), [pick.quantity]);
  return (
    <div className="mt-1 flex items-center gap-1 text-xs">
      <span className="flex-1 truncate">{pick.ingredient_name}</span>
      <span onBlur={() => onAmount(text)}>
        <QuantityInput
          value={text}
          unit={pick.unit}
          onChange={setText}
          className="w-16 rounded border border-slate-300 px-1 py-0.5 text-right text-xs"
        />
      </span>
      <span className="w-7 text-slate-400">{pick.unit}</span>
      <button type="button" onClick={onRemove} aria-label={`Remove ${pick.ingredient_name}`} className="px-1 text-slate-400 hover:text-red-600">
        ✕
      </button>
    </div>
  );
}
