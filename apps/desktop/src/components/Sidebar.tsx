import { useTranslation } from "react-i18next";
import type { Collection } from "../logic/filters";
import type { View } from "../store";
import type { Label } from "../types";
import { MomentsPanel } from "./MomentsPanel";
import { PeoplePanel } from "./PeoplePanel";
import { SelectionPanel } from "./SelectionPanel";

const COLLECTIONS: Collection[] = ["all", "picks", "rejects", "unrated"];

/** Secções futuras: visíveis mas desativadas e identificadas com a fase em que chegam. */
const UPCOMING: [string, number][] = [["export", 6]];

export function Sidebar({
  active,
  counts,
  onSelect,
  labelCounts,
  activeLabel,
  onLabel,
    view,
  onView,
  activeMoment,
  onMoment,
  onDropMoment,
  activePersons,
  onPersons,
}: {
  active: Collection;
  counts: Record<Collection, number>;
  onSelect: (c: Collection) => void;
  labelCounts: Record<Label, number>;
  activeLabel: Label | null;
  onLabel: (l: Label | null) => void;
    view: View;
  onView: (v: View) => void;
  activeMoment: number | "unclassified" | null;
  onMoment: (m: number | "unclassified" | null) => void;
  onDropMoment: (ids: number[], moment: number | null) => void;
  activePersons: number[];
  onPersons: (ids: number[]) => void;
}) {
  const { t } = useTranslation();
  return (
    <nav className="sidebar" aria-label={t("sidebar.label")}>
      <h3 className="sidebar-title">{t("sidebar.library")}</h3>
      <ul>
        {COLLECTIONS.map((c) => (
          <li key={c}>
            <button type="button" className={`side-item${active === c && view === "grid" && !activeLabel && activeMoment == null && activePersons.length === 0 ? " active" : ""}`} aria-current={active === c} onClick={() => onSelect(c)}>
              <span>{t(`collection.${c}`)}</span>
              <span className="count">{counts[c].toLocaleString()}</span>
            </button>
          </li>
        ))}
      </ul>
            <MomentsPanel active={activeMoment} onSelect={onMoment} onDrop={onDropMoment} />
      <PeoplePanel active={activePersons} onPersons={onPersons} view={view} onView={onView} />
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
