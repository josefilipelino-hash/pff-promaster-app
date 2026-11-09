import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import { errorText, useStore, type View } from "../store";
import { LABELS, SELECTION_MODES, type Label, type SelectionMode } from "../types";

/** “Seleção automática” na barra lateral: análise, modo, objetivo e contagens por etiqueta. */
export function SelectionPanel({
  labelCounts,
  activeLabel,
  onLabel,
  view,
  onView,
}: {
  labelCounts: Record<Label, number>;
  activeLabel: Label | null;
  onLabel: (l: Label | null) => void;
  view: View;
  onView: (v: View) => void;
}) {
  const { t } = useTranslation();
  const analysis = useStore((s) => s.analysis);
  const progress = useStore((s) => s.analysisProgress);
  const groups = useStore((s) => s.groups);
  const toast = useStore((s) => s.toast);
  const reloadSelection = useStore((s) => s.reloadSelection);
  const [target, setTarget] = useState("");
  const [confirmReanalyze, setConfirmReanalyze] = useState(false);
  const fail = (e: unknown) => toast("error", errorText(e, t));

  useEffect(() => {
    setTarget(analysis?.settings.target != null ? String(analysis.settings.target) : "");
  }, [analysis?.settings.target]);

  if (!analysis) return null;
  const { counts, running, settings, modelsAvailable } = analysis;
  const analyzed = counts.analyzed > 0;
  const sequences = groups.filter((g) => g.kind === "sequence").length;

  const start = async (reanalyze: boolean) => {
    setConfirmReanalyze(false);
    try {
      await api.startAnalysis(reanalyze);
      await reloadSelection();
    } catch (e) {
      fail(e);
    }
  };
  const apply = async (mode: SelectionMode, tgt: number | null) => {
    try {
      await api.setSelectionSettings({ mode, target: tgt });
      await reloadSelection();
    } catch (e) {
      fail(e);
    }
  };
  const commitTarget = () => {
    const n = Number.parseInt(target, 10);
    const next = Number.isFinite(n) && n > 0 ? n : null;
    if (next !== settings.target) void apply(settings.mode, next);
    else setTarget(next != null ? String(next) : "");
  };

  return (
    <section className="selection-panel" aria-label={t("selection.title")}>
      <h3 className="sidebar-title">{t("selection.title")}</h3>
      {running ? (
        <div className="analysis-progress">
          <span className="small">{t("selection.analyzing", { done: progress?.done ?? 0, total: progress?.total ?? counts.pending })}</span>
          <progress max={Math.max(1, progress?.total ?? 1)} value={progress?.done ?? 0} />
          <button type="button" className="btn small ghost" onClick={() => api.cancelAnalysis().catch(fail)}>
            {t("selection.stop")}
          </button>
        </div>
      ) : !analyzed ? (
        <div className="analysis-intro">
          <p className="small muted">{t("selection.intro")}</p>
          <button type="button" className="btn primary small" disabled={counts.total === 0} onClick={() => void start(false)}>
            {t("selection.analyze", { count: counts.pending })}
          </button>
        </div>
      ) : counts.pending > 0 ? (
        <div className="analysis-intro">
          <span className="small muted">{t("selection.pending", { count: counts.pending })}</span>
          <button type="button" className="btn small" onClick={() => void start(false)}>
            {t("selection.analyzePending")}
          </button>
        </div>
      ) : null}
      {!modelsAvailable && <p className="notice warn small">{t("selection.noModels")}</p>}
      {counts.failed > 0 && <p className="small muted">{t("selection.failed", { count: counts.failed })}</p>}

      {analyzed && (
        <>
          <label className="field compact">
            <span className="small muted">{t("selection.mode")}</span>
            <select value={settings.mode} onChange={(e) => void apply(e.target.value as SelectionMode, settings.target)} title={t(`selection.modeHint.${settings.mode}`)}>
              {SELECTION_MODES.map((m) => (
                <option key={m} value={m}>
                  {t(`selection.modes.${m}`)}
                </option>
              ))}
            </select>
          </label>
          <label className="field compact">
            <span className="small muted">{t("selection.target")}</span>
            <input
              type="number"
              min={1}
              inputMode="numeric"
              placeholder={t("selection.noTarget")}
              value={target}
              onChange={(e) => setTarget(e.target.value)}
              onBlur={commitTarget}
              onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
            />
          </label>
          <ul>
            {LABELS.map((l) => (
              <li key={l}>
                <button type="button" className={`side-item${activeLabel === l ? " active" : ""}`} aria-pressed={activeLabel === l} onClick={() => onLabel(activeLabel === l ? null : l)} title={t(`label.${l}Hint`)}>
                  <span>
                    <span className={`label-dot l-${l}`} aria-hidden /> {t(`label.${l}`)}
                  </span>
                  <span className="count">{labelCounts[l].toLocaleString()}</span>
                </button>
              </li>
            ))}
            <li>
              <button type="button" className={`side-item${view === "sequences" ? " active" : ""}`} aria-pressed={view === "sequences"} onClick={() => onView(view === "sequences" ? "grid" : "sequences")}>
                <span>{t("selection.sequences")}</span>
                <span className="count">{sequences.toLocaleString()}</span>
              </button>
            </li>
          </ul>
          {!running &&
            (confirmReanalyze ? (
              <div className="confirm-inline small">
                <span>{t("selection.reanalyzeConfirm")}</span>
                <button type="button" className="btn small" onClick={() => void start(true)}>
                  {t("selection.reanalyzeYes")}
                </button>
                <button type="button" className="btn small ghost" onClick={() => setConfirmReanalyze(false)}>
                  {t("common.cancel")}
                </button>
              </div>
            ) : (
              <button type="button" className="btn small ghost" onClick={() => setConfirmReanalyze(true)}>
                {t("selection.reanalyze")}
              </button>
            ))}
          <p className="small muted privacy-note">{t("selection.local")}</p>
        </>
      )}
    </section>
  );
}
