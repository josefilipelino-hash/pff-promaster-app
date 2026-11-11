import { useTranslation } from "react-i18next";

/**
 * Deslizador de ajuste: `onChange(v, false)` durante o arrasto (render rápido),
 * `onChange(v, true)` ao largar (grava no histórico). Duplo clique no nome repõe o valor.
 */
export function Slider({
  label,
  value,
  min,
  max,
  step = 1,
  def = 0,
  digits = 0,
  onChange,
  gradient,
  format,
  parse,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  def?: number;
  digits?: number;
  onChange: (v: number, done: boolean) => void;
  gradient?: string;
  /** Mostrar o valor noutra unidade (ex.: Kelvin) e ler o que se escreve nessa unidade. */
  format?: (v: number) => string;
  parse?: (text: string) => number | null;
}) {
  const { t } = useTranslation();
  const shown = format ? format(value) : digits > 0 ? value.toFixed(digits) : String(Math.round(value));
  const done = (e: { currentTarget: HTMLInputElement }) => onChange(Number(e.currentTarget.value), true);
  return (
    <div className={`dev-slider${value !== def ? " changed" : ""}`}>
      <label>
        <span className="dev-label" onDoubleClick={() => onChange(def, true)} title={t("develop.resetHint")}>
          {label}
        </span>
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          aria-label={label}
          style={gradient ? { backgroundImage: gradient } : undefined}
          onChange={(e) => onChange(Number(e.target.value), false)}
          onPointerUp={done}
          onKeyUp={done}
          // Largar o rato fora do controlo nem sempre dá `pointerup` no próprio controlo:
          // a perda da captura também grava (Fase 15).
          onLostPointerCapture={done}
        />
      </label>
            <input
        key={shown}
        className="dev-value"
        aria-label={`${label} (${t("develop.value")})`}
        defaultValue={shown}
        onFocus={(e) => e.target.select()}
        onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
        onBlur={(e) => {
          const v = parse ? parse(e.target.value) : Number(e.target.value.replace(",", "."));
          if (e.target.value !== shown && v !== null && Number.isFinite(v)) onChange(Math.min(max, Math.max(min, v)), true);
        }}
      />
    </div>
  );
}
