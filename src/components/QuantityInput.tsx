import type { Unit } from "../lib/api";
import { parseQuantity } from "../lib/quantity";

interface Props {
  value: string;
  /** The ingredient's unit, or "" if none is chosen yet. Decides what `kg` / `L` convert to. */
  unit: Unit | "";
  onChange: (text: string) => void;
  placeholder?: string;
  className?: string;
}

/**
 * A quantity box that understands how people write amounts: `250`, `1.5kg`, `500ml`, `1l`.
 * On leaving the box it turns `1.5kg` into `1500` (for a gram ingredient) so what is stored is
 * always plain and visible.
 */
export default function QuantityInput({ value, unit, onChange, placeholder = "Qty", className }: Props) {
  return (
    <input
      type="text"
      inputMode="decimal"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      onBlur={() => {
        const r = parseQuantity(value, unit);
        if (r.ok && String(r.value) !== value.trim()) onChange(String(r.value));
      }}
      placeholder={placeholder}
      title="e.g. 250, 1.5kg, 500ml"
      className={className ?? "w-24 rounded border border-slate-300 px-2 py-1 text-right text-sm"}
    />
  );
}
