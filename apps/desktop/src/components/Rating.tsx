import { useTranslation } from "react-i18next";
import { COLORS, type Color } from "../types";

export function Stars({ value, onChange, size = "md" }: { value: number; onChange?: (n: number) => void; size?: "sm" | "md" }) {
  const { t } = useTranslation();
  return (
    <span className={`stars stars-${size}`} role={onChange ? "radiogroup" : undefined} aria-label={t("rating.stars", { count: value })}>
      {[1, 2, 3, 4, 5].map((n) =>
        onChange ? (
          <button
            key={n}
            type="button"
            className={n <= value ? "star on" : "star"}
            aria-label={t("rating.setStars", { count: n })}
            title={t("rating.setStars", { count: n })}
            onClick={() => onChange(n === value ? 0 : n)}
          >
            ★
          </button>
        ) : (
          <span key={n} className={n <= value ? "star on" : "star"} aria-hidden>
            ★
          </span>
        ),
      )}
    </span>
  );
}

export function FlagButtons({ value, onChange }: { value: number; onChange: (v: -1 | 0 | 1) => void }) {
  const { t } = useTranslation();
  return (
    <span className="flags">
      <button type="button" className={value === 1 ? "flag pick on" : "flag pick"} title={`${t("rating.pick")} (P)`} aria-pressed={value === 1} onClick={() => onChange(value === 1 ? 0 : 1)}>
        ♥ <span>{t("rating.pick")}</span>
      </button>
      <button type="button" className={value === -1 ? "flag reject on" : "flag reject"} title={`${t("rating.reject")} (X)`} aria-pressed={value === -1} onClick={() => onChange(value === -1 ? 0 : -1)}>
        ✕ <span>{t("rating.reject")}</span>
      </button>
    </span>
  );
}

export function ColorButtons({ value, onChange }: { value: Color | null; onChange: (c: Color | null) => void }) {
  const { t } = useTranslation();
  return (
    <span className="colors">
      {COLORS.map((c) => (
        <button
          key={c}
          type="button"
          className={`color-dot c-${c}${value === c ? " on" : ""}`}
          title={t(`color.${c}`)}
          aria-label={t(`color.${c}`)}
          aria-pressed={value === c}
          onClick={() => onChange(value === c ? null : c)}
        />
      ))}
    </span>
  );
}
