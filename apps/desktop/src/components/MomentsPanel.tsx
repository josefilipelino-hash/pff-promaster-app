import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import { momentCounts } from "../logic/filters";
import { errorText, useStore } from "../store";
import type { MomentRow } from "../types";

export function momentName(m: MomentRow, t: (k: string) => string): string {
  return m.custom || !m.kind ? (m.name ?? "") : (m.name ?? t(`moment.${m.kind}`));
}

/**
 * Momentos na barra lateral: filtrar, largar fotografias arrastadas da grelha, criar
 * momentos personalizados e abrir a sincronização de câmaras.
 */
export function MomentsPanel({ active, onSelect, onDrop }: { active: number | "unclassified" | null; onSelect: (m: number | "unclassified" | null) => void; onDrop: (ids: number[], moment: number | null) => void }) {
  const { t } = useTranslation();
  const moments = useStore((s) => s.moments);
  const photos = useStore((s) => s.photos);
  const photoMoment = useStore((s) => s.photoMoment);
  const dragIds = useStore((s) => s.dragIds);
  const reloadOrganization = useStore((s) => s.reloadOrganization);
  const toast = useStore((s) => s.toast);
  const setClockOpen = useStore((s) => s.setClockOpen);
  const [showAll, setShowAll] = useState(false);
  const [creating, setCreating] = useState<string | null>(null);
  const counts = useMemo(() => momentCounts(photos, photoMoment), [photos, photoMoment]);
  if (moments.length === 0) return null;
  const dragging = dragIds !== null;
  // Durante o arrasto a lista não muda (os alvos não fogem do cursor); os momentos vazios
  // aparecem com “Mostrar todos”.
  const visible = moments.filter((m) => showAll || m.custom || (counts.get(m.id) ?? 0) > 0);
    const drop = (moment: number | null) => () => {
    const ids = useStore.getState().dragIds;
    if (!ids) return;
    // Não interromper a propagação: a grelha precisa do mouseup para terminar o arrasto.
    useStore.getState().setDrag(null);
    onDrop(ids, moment);
  };
  const create = async () => {
    const name = (creating ?? "").trim();
    setCreating(null);
    if (!name) return;
    try {
      await api.createMoment(name, null);
      await reloadOrganization();
    } catch (e) {
      toast("error", errorText(e, t));
    }
  };
  return (
    <section className={`moments-panel${dragging ? " dragging" : ""}`} aria-label={t("moments.title")}>
      <h3 className="sidebar-title">{t("moments.title")}</h3>
      
      <ul>
        {visible.map((m) => (
          <li key={m.id}>
            <button
              type="button"
              className={`side-item drop-target${active === m.id ? " active" : ""}`}
              data-drop-moment={m.id}
              aria-pressed={active === m.id}
              onClick={() => onSelect(active === m.id ? null : m.id)}
              onMouseUp={drop(m.id)}
            >
              <span>{momentName(m, t)}</span>
              <span className="count">{(counts.get(m.id) ?? 0).toLocaleString()}</span>
            </button>
          </li>
        ))}
        <li>
          <button
            type="button"
            className={`side-item drop-target${active === "unclassified" ? " active" : ""}`}
            data-drop-moment="none"
            aria-pressed={active === "unclassified"}
            onClick={() => onSelect(active === "unclassified" ? null : "unclassified")}
            onMouseUp={drop(null)}
            title={t("moments.unclassifiedHint")}
          >
            <span>{t("moments.unclassified")}</span>
            <span className="count">{(counts.get("unclassified") ?? 0).toLocaleString()}</span>
          </button>
        </li>
      </ul>
      <div className="moments-actions">
        {creating === null ? (
          <button type="button" className="btn small ghost" onClick={() => setCreating("")}>
            + {t("moments.new")}
          </button>
        ) : (
          <input
            autoFocus
            className="moment-input"
            placeholder={t("moments.newPlaceholder")}
            value={creating}
            onChange={(e) => setCreating(e.target.value)}
            onBlur={() => void create()}
            onKeyDown={(e) => {
              if (e.key === "Enter") (e.target as HTMLInputElement).blur();
              if (e.key === "Escape") setCreating(null);
            }}
          />
        )}
        <button type="button" className="btn small ghost" onClick={() => setShowAll(!showAll)}>
          {showAll ? t("moments.hideEmpty") : t("moments.showAll")}
        </button>
        <button type="button" className="btn small ghost" onClick={() => setClockOpen(true)} title={t("clock.hint")}>
          {t("clock.open")}
        </button>
      </div>
    </section>
  );
}
