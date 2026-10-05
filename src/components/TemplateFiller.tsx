import { useEffect, useState } from "react";
import {
  api,
  errorMessage,
  type Ingredient,
  type IngredientCategory,
  type ItemDraft,
  type Template,
  type TemplateSummary,
} from "../lib/api";
import { formatQuantity } from "../lib/format";
import IngredientForm from "./IngredientForm";
import IngredientPicker from "./IngredientPicker";

interface Props {
  ingredients: Ingredient[];
  categories: IngredientCategory[];
  onIngredientCreated: (ingredient: Ingredient) => void;
  /** Receives the unsaved meal; the caller copies it into the editor. */
  onApply: (draft: ItemDraft, templateName: string) => void;
  onCancel: () => void;
}

/** "Start from template": pick a template, fill its slots, and copy the lines into the meal. */
export default function TemplateFiller({
  ingredients,
  categories,
  onIngredientCreated,
  onApply,
  onCancel,
}: Props) {
  const [templates, setTemplates] = useState<TemplateSummary[]>([]);
  const [template, setTemplate] = useState<Template | null>(null);
  // Slot line id -> chosen ingredient id.
  const [fills, setFills] = useState<Record<number, number>>({});
  const [creating, setCreating] = useState<{ lineId: number; name: string; categoryId: number | null } | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.templates
      .list()
      .then(setTemplates)
      .catch((e) => setError(errorMessage(e)));
  }, []);

  async function choose(id: number) {
    setFills({});
    setError(null);
    if (!id) return setTemplate(null);
    try {
      setTemplate(await api.templates.get(id));
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function apply() {
    if (!template) return;
    try {
      const draft = await api.templates.instantiate(
        template.id,
        Object.entries(fills).map(([line_id, ingredient_id]) => ({ line_id: Number(line_id), ingredient_id })),
      );
      onApply(draft, template.name);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  const slots = template?.lines.filter((l) => l.ingredient_id === null) ?? [];
  const fixed = template?.lines.filter((l) => l.ingredient_id !== null) ?? [];

  return (
    <div className="space-y-3 rounded border border-emerald-200 bg-emerald-50 p-3">
      <div className="flex items-center gap-2">
        <label className="flex flex-1 items-center gap-2 text-sm">
          <span className="text-xs text-slate-500">Template</span>
          <select
            value={template?.id ?? ""}
            onChange={(e) => choose(Number(e.target.value))}
            className="flex-1 rounded border border-slate-300 bg-white px-2 py-1 text-sm"
          >
            <option value="">Choose a template…</option>
            {templates.map((t) => (
              <option key={t.id} value={t.id}>
                {t.name}
              </option>
            ))}
          </select>
        </label>
        <button type="button" onClick={onCancel} className="rounded px-2 py-1 text-sm text-slate-600 hover:bg-white">
          Cancel
        </button>
      </div>

      {templates.length === 0 && !error && (
        <p className="text-xs text-slate-500">No templates yet — create one in Library → Templates.</p>
      )}

      {template && (
        <>
          {fixed.length > 0 && (
            <p className="text-xs text-slate-600">
              Always included:{" "}
              {fixed
                .map((l) => `${l.ingredient_name} ${l.quantity !== null ? formatQuantity(l.quantity, l.ingredient_unit!) : ""}`.trim())
                .join(", ")}
            </p>
          )}
          {slots.length > 0 && (
            <div className="space-y-2">
              {slots.map((l) => (
                <div key={l.id}>
                  <div className="flex items-center gap-2">
                    <span className="w-24 shrink-0 text-sm font-medium">{l.label}</span>
                    <div className="min-w-0 flex-1">
                      <IngredientPicker
                        ingredients={ingredients}
                        categoryId={l.category_id}
                        value={fills[l.id] ?? null}
                        onChange={(i) => setFills({ ...fills, [l.id]: i.id })}
                        onCreate={(name) => setCreating({ lineId: l.id, name, categoryId: l.category_id })}
                      />
                    </div>
                    <span className="w-24 shrink-0 text-xs text-slate-500">
                      {l.category_name ? `${l.category_name} · ` : ""}
                      {l.quantity !== null && l.unit_hint ? `${formatQuantity(l.quantity, l.unit_hint)}` : ""}
                    </span>
                  </div>
                  {creating?.lineId === l.id && (
                    <div className="mt-2 rounded border border-emerald-300 bg-white p-2">
                      <IngredientForm
                        categories={categories}
                        initialName={creating.name}
                        initialCategoryId={creating.categoryId}
                        onSaved={(ing) => {
                          onIngredientCreated(ing);
                          setFills({ ...fills, [l.id]: ing.id });
                          setCreating(null);
                        }}
                        onCancel={() => setCreating(null)}
                      />
                    </div>
                  )}
                </div>
              ))}
              <p className="text-xs text-slate-500">Leave a slot empty to skip it.</p>
            </div>
          )}
          <button
            type="button"
            onClick={apply}
            className="rounded bg-emerald-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-emerald-700"
          >
            Fill in meal
          </button>
        </>
      )}
      {error && <p className="text-sm text-red-600">{error}</p>}
    </div>
  );
}
