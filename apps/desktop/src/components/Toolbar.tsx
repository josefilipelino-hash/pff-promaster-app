import { useTranslation } from "react-i18next";
import { formatEventDate } from "../logic/format";
import type { View } from "../store";
import type { ProjectInfo } from "../types";

/** Barra superior: projeto, importação, ajuda e fecho. Os filtros estão na barra inferior. */
export function Toolbar({
  project,
  importBusy,
  onImportFiles,
  onImportFolder,
  onClose,
  onHelp,
  onCompare,
  view,
  onView,
}: {
  project: ProjectInfo;
  importBusy: boolean;
  onImportFiles: () => void;
  onImportFolder: () => void;
  onClose: () => void;
  onHelp: () => void;
  onCompare: () => void;
  view: View;
  onView: (v: View) => void;
}) {
  const { t, i18n } = useTranslation();
  return (
    <div className="toolbar">
      <div className="toolbar-title">
        <strong>{project.name}</strong>
        <span className="muted">
          {[project.clientName, formatEventDate(project.eventDate, i18n.language), project.location].filter(Boolean).join(" · ")}
        </span>
      </div>
      <span className="spacer" />
      <span className="segmented" role="group" aria-label={t("view.label")}>
        <button type="button" className={`btn small${view === "grid" ? " active" : ""}`} aria-pressed={view === "grid"} onClick={() => onView("grid")}>
          {t("view.grid")}
        </button>
        <button type="button" className={`btn small${view === "sequences" ? " active" : ""}`} aria-pressed={view === "sequences"} onClick={() => onView("sequences")}>
                    {t("view.sequences")}
        </button>
                <button type="button" className={`btn small${view === "people" ? " active" : ""}`} aria-pressed={view === "people"} onClick={() => onView("people")}>
          {t("view.people")}
        </button>
        <button type="button" className={`btn small${view === "develop" ? " active" : ""}`} aria-pressed={view === "develop"} onClick={() => onView("develop")} title={`${t("view.develop")} (D)`}>
          {t("view.develop")}
        </button>
      </span>
      <button type="button" className="btn" onClick={onCompare} title={`${t("compare.openHint")} (C)`}>
        {t("compare.open")}
      </button>
      <span className="sep" />
      <button type="button" className="btn" disabled={importBusy} onClick={onImportFiles} title={t("import.filesHint")}>
        {t("import.files")}
      </button>
      <button type="button" className="btn" disabled={importBusy} onClick={onImportFolder} title={t("import.folderHint")}>
        {t("import.folder")}
      </button>
      <span className="sep" />
      <button type="button" className="btn ghost" onClick={onHelp} title={t("shortcuts.title")}>
        ?
      </button>
      <button type="button" className="btn ghost" onClick={onClose}>
        {t("project.close")}
      </button>
    </div>
  );
}
