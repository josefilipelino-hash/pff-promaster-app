import { useTranslation } from "react-i18next";
import type { Collection } from "../logic/filters";
import type { View } from "../store";
import type { Label } from "../types";
import { SelectionPanel } from "./SelectionPanel";

const COLLECTIONS: Collection[] = ["all", "picks", "rejects", "unrated"];

/** Secções futuras: visíveis mas desativadas e identificadas com a fase em que chegam. */
const UPCOMING: [string, number][] = [
  ["moments", 5],
  ["people", 5],
  ["export", 6],
];

export function Sidebar({
  active,
  counts,
  onSelect,
  labelCounts,
  activeLabel,
  onLabel,
  view,
  onView,
}: {
  active: Collection;
  counts: Record<Collection, number>;
  onSelect: (c: Collection) => void;
  labelCounts: Record<Label, number>;
  activeLabel: Label | null;
  onLabel: (l: Label | null) => void;
  view: View;
  onView: (v: View) => void;
}) {
  const { t } = useTranslation();
  return (
    <nav className="sidebar" aria-label={t("sidebar.label")}>
      <h3 className="sidebar-title">{t("sidebar.library")}</h3>
      <ul>
        {COLLECTIONS.map((c) => (
          <li key={c}>
            <button type="button" className={`side-item${active === c && view === "grid" && !activeLabel ? " active" : ""}`} aria-current={active === c} onClick={() => onSelect(c)}>
              <span>{t(`collection.${c}`)}</span>
              <span className="count">{counts[c].toLocaleString()}</span>
            </button>
          </li>
        ))}
      </ul>
      <SelectionPanel labelCounts={labelCounts} activeLabel={activeLabel} onLabel={onLabel} view={view} onView={onView} />
      <h3 className="sidebar-title">{t("sidebar.upcoming")}</h3>
      <ul>
        {UPCOMING.map(([k, phase]) => (
          <li key={k}>
            <button type="button" className="side-item" disabled title={t("sidebar.availableIn", { phase })}>
              <span>{t(`sidebar.${k}`)}</span>
              <span className="badge">{t("sidebar.phase", { phase })}</span>
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
