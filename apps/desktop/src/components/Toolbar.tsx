import { useState } from "react";
import { useTranslation } from "react-i18next";
import { formatEventDate } from "../logic/format";
import { shortcutLabel } from "../logic/keys";
import { loadThemePref, saveThemePref, type ThemePref } from "../logic/theme";
import type { View } from "../store";
import type { ProjectInfo } from "../types";
import { Icon, type IconName } from "./Icon";
import { MenuButton } from "./Menu";

const VIEWS: { v: View; icon: IconName; key: string; shortcut?: string; hint?: string }[] = [
  { v: "grid", icon: "grid", key: "view.grid", shortcut: "G" },
  { v: "sequences", icon: "stack", key: "view.sequences" },
  { v: "people", icon: "people", key: "view.people" },
  { v: "consistency", icon: "balance", key: "view.consistency", hint: "look.viewHint" },
  { v: "develop", icon: "sliders", key: "view.develop", shortcut: "D" },
  { v: "album", icon: "album", key: "view.album" },
];

/** Barra superior (Fase 14): uma linha — projeto, vistas, ações; importação e o resto em menus. */
export function Toolbar({
  project,
  importBusy,
  onImportFiles,
  onImportFolder,
  onClose,
  onHelp,
  onCompare,
  onAutoEdit,
  onPresets,
  onExport,
  selectedCount = 0,
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
  /** "Editar automaticamente seleção". */
  onAutoEdit?: () => void;
  /** Aplicar preset (seleção, momento, casamento). */
  onPresets?: () => void;
  /** Abrir o diálogo de exportação. */
  onExport?: () => void;
  selectedCount?: number;
  view: View;
  onView: (v: View) => void;
}) {
  const { t, i18n } = useTranslation();
  const [theme, setTheme] = useState<ThemePref>(loadThemePref);
  const pickTheme = (p: ThemePref) => {
    saveThemePref(p);
    setTheme(p);
  };
  return (
    <div className="toolbar" role="toolbar" aria-label={t("toolbar.label")}>
      <div className="toolbar-title">
        <strong title={project.name}>{project.name}</strong>
        <span className="muted">
          {[project.clientName, formatEventDate(project.eventDate, i18n.language), project.location].filter(Boolean).join(" · ")}
        </span>
      </div>
      <span className="segmented views" role="group" aria-label={t("view.label")}>
        {VIEWS.map(({ v, icon, key, shortcut, hint }) => (
          <button
            key={v}
            type="button"
            className={`btn small${view === v ? " active" : ""}`}
            aria-pressed={view === v}
            aria-label={t(key)}
            onClick={() => onView(v)}
            title={`${hint ? t(hint) : t(key)}${shortcut ? ` (${shortcut})` : ""}`}
          >
            <Icon name={icon} />
            <span className="btn-label">{t(key)}</span>
          </button>
        ))}
      </span>
      <span className="spacer" />
      <button type="button" className="btn small act" onClick={onCompare} title={`${t("compare.openHint")} (C)`} aria-label={t("compare.open")}>
        <Icon name="compare" />
        <span className="act-label">{t("compare.open")}</span>
      </button>
      {onPresets && (
        <button type="button" className="btn small act" onClick={onPresets} title={t("presets.applyHint")} aria-label={t("presets.button")}>
          <Icon name="preset" />
          <span className="act-label">{t("presets.button")}</span>
        </button>
      )}
      {onAutoEdit && (
        <button type="button" className="btn small act" disabled={selectedCount === 0} onClick={onAutoEdit} title={t("auto.batchHint")} aria-label={t("auto.batchButton", { count: selectedCount })}>
          <Icon name="wand" />
          <span className="act-label">{t("auto.batchButton", { count: selectedCount })}</span>
        </button>
      )}
      {onExport && (
        <button type="button" className="btn small primary" onClick={onExport} title={`${t("export.buttonHint")} (${shortcutLabel("Mod+Shift+E")})`}>
          <Icon name="export" />
          {t("export.button")}
        </button>
      )}
      <span className="sep" />
      <MenuButton
        className="btn small act"
        label={t("toolbar.import")}
        title={`${t("toolbar.import")} (${shortcutLabel("Mod+Shift+I")})`}
        entries={[
          { label: t("import.filesShort"), icon: "file", onSelect: onImportFiles, disabled: importBusy, shortcut: shortcutLabel("Mod+Shift+I") },
          { label: t("import.folderShort"), icon: "folder", onSelect: onImportFolder, disabled: importBusy },
        ]}
      >
        <Icon name="import" />
        <span className="act-label">{t("toolbar.import")}</span>
        <Icon name="chevronDown" size={12} />
      </MenuButton>
      <MenuButton
        className="btn small ghost icon-only"
        label={t("toolbar.more")}
        entries={[
          { label: t("shortcuts.title"), icon: "keyboard", onSelect: onHelp, shortcut: "?" },
          { kind: "separator" },
          { kind: "heading", label: t("theme.title") },
          { label: t("theme.dark"), onSelect: () => pickTheme("dark"), checked: theme === "dark" },
          { label: t("theme.light"), onSelect: () => pickTheme("light"), checked: theme === "light" },
          { label: t("theme.system"), onSelect: () => pickTheme("system"), checked: theme === "system" },
          { kind: "separator" },
          { label: t("project.close"), icon: "close", onSelect: onClose },
        ]}
      >
        <Icon name="more" />
      </MenuButton>
    </div>
  );
}
