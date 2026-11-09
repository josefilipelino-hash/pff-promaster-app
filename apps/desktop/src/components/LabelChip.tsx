import { useTranslation } from "react-i18next";
import type { Label } from "../types";

/** Etiqueta da seleção assistida: MELHOR / ALTERNATIVA / REVER / REJEITAR SUGERIDO / DUPLICADA. */
export function LabelChip({ label, compact }: { label: Label; compact?: boolean }) {
  const { t } = useTranslation();
  return (
    <span className={`label-chip l-${label}${compact ? " compact" : ""}`} title={t(`label.${label}Hint`)}>
      {compact ? t(`label.${label}Short`) : t(`label.${label}`)}
    </span>
  );
}
