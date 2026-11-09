import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import { errorText, useStore } from "../store";
import { AutoOptionsForm } from "./develop/AutoPanel";
import { Modal } from "./Modal";

/** "Editar automaticamente seleção": opções e início da fila em segundo plano. */
export function AutoEditDialog({ keys, onClose }: { keys: number[]; onClose: () => void }) {
  const { t } = useTranslation();
  const options = useStore((s) => s.autoOptions);
  const setOptions = useStore((s) => s.setAutoOptions);
  const toast = useStore((s) => s.toast);
  const setProgress = useStore((s) => s.setAutoProgress);
  const [aiAvailable, setAiAvailable] = useState(false);
  useEffect(() => {
    api.aiDenoiseAvailable().then(setAiAvailable).catch(() => setAiAvailable(false));
  }, []);
  const start = async () => {
    try {
      const total = await api.startAutoEdit(keys, aiAvailable ? options : { ...options, aiDenoise: "off" });
      setProgress({ kind: "auto", total, done: 0, failed: 0, paused: false, cancelled: false, finished: false, current: null });
      onClose();
    } catch (e) {
      toast("error", errorText(e, t));
    }
  };
  return (
    <Modal
      title={t("auto.batchTitle", { count: keys.length })}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn" onClick={onClose} data-close>
            {t("common.cancel")}
          </button>
          <button type="button" className="btn primary" disabled={keys.length === 0} onClick={() => void start()}>
            {t("auto.batchStart", { count: keys.length })}
          </button>
        </>
      }
    >
      <p className="small">{t("auto.batchIntro")}</p>
      <AutoOptionsForm options={options} onChange={setOptions} aiAvailable={aiAvailable} />
      {options.aiDenoise !== "off" && aiAvailable && <p className="small muted">{t("auto.aiSlow")}</p>}
      <p className="small muted">{t("auto.nonDestructive")}</p>
    </Modal>
  );
}

/** Fila da edição automática (canto inferior direito): progresso, pausa, retoma, cancelar. */
export function AutoEditQueue() {
  const { t } = useTranslation();
  const p = useStore((s) => s.autoProgress);
  const setProgress = useStore((s) => s.setAutoProgress);
  const toast = useStore((s) => s.toast);
  if (!p) return null;
  const fail = (e: unknown) => toast("error", errorText(e, t));
  const processed = p.done + p.failed;
  const pct = p.total > 0 ? Math.round((processed / p.total) * 100) : 0;
  const state = p.finished ? (p.cancelled ? "cancelled" : "finished") : p.paused ? "paused" : "running";
  return (
    <section className="auto-queue" aria-label={t(p.kind === "sync" ? "sync.queueTitle" : "auto.queueTitle")}>
      <header>
        <strong>{t(p.kind === "sync" ? "sync.queueTitle" : "auto.queueTitle")}</strong>
        <span className="muted small">{t(`auto.queueState.${state}`)}</span>
      </header>
      <div className="progress" role="progressbar" aria-valuemin={0} aria-valuemax={p.total} aria-valuenow={processed} aria-label={t("auto.queueTitle")}>
        <div className="progress-bar" style={{ width: `${pct}%` }} />
      </div>
      <p className="small">
        {t("auto.queueCount", { done: processed, total: p.total })}
        {p.failed > 0 && <span className="warn"> · {t("auto.queueFailed", { count: p.failed })}</span>}
      </p>
      {!p.finished && p.current && <p className="small muted ellipsis">{p.current}</p>}
      <div className="dev-row">
        {!p.finished ? (
          <>
            {p.paused ? (
              <button type="button" className="btn small" onClick={() => api.resumeAutoEdit().catch(fail)}>
                ▶ {t("auto.resume")}
              </button>
            ) : (
              <button type="button" className="btn small" onClick={() => api.pauseAutoEdit().catch(fail)}>
                ‖ {t("auto.pause")}
              </button>
            )}
            <button type="button" className="btn small" onClick={() => api.cancelAutoEdit().catch(fail)}>
              {t("auto.cancel")}
            </button>
          </>
        ) : (
          <button type="button" className="btn small" onClick={() => setProgress(null)}>
            {t("common.close")}
          </button>
        )}
      </div>
    </section>
  );
}
