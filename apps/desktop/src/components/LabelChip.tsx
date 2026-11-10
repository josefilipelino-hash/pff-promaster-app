import { useTranslation } from "react-i18next";
import type { Label } from "../types";

/** Etiqueta da seleção assistida: MELHOR / ALTERNATIVA / REVER / REJEITAR SUGERIDO / DUPLICADA. */
export function LabelChip({ label, compact, protectedByUser }: { label: Label; compact?: boolean; protectedByUser?: boolean }) {
  const { t } = useTranslation();
  // A sugestão nunca se sobrepõe ao utilizador: uma favorita ou com estrelas não aparece como
  // "Rejeitar?"/"Duplicada", mas como protegida.
  if (protectedByUser && (label === "reject" || label === "duplicate")) {
    return (
      <span className={`label-chip l-protected${compact ? " compact" : ""}`} title={t("label.protectedHint")}>
        {compact ? t("label.protectedShort") : t("label.protected")}
      </span>
    );
  }
  return (
    <span className={`label-chip l-${label}${compact ? " compact" : ""}`} title={t(`label.${label}Hint`)}>
      {compact ? t(`label.${label}Short`) : t(`label.${label}`)}
    </span>
  );
}
