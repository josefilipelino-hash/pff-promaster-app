import { useTranslation } from "react-i18next";
import { formatEventDate } from "../logic/format";
import type { ProjectInfo } from "../types";

/** Barra superior: projeto, importação, ajuda e fecho. Os filtros estão na barra inferior. */
export function Toolbar({
  project,
  importBusy,
  onImportFiles,
  onImportFolder,
  onClose,
  onHelp,
}: {
  project: ProjectInfo;
  importBusy: boolean;
  onImportFiles: () => void;
  onImportFolder: () => void;
  onClose: () => void;
  onHelp: () => void;
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
