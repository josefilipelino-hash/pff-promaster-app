import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import { errorText, useStore } from "../store";
import type { LightroomReport } from "../types";
import { Modal } from "./Modal";

/** "Classificações para o Lightroom": Melhor 5★, Alternativa 3★, Rever 2★ em `.xmp` ao lado
 *  dos RAW (que nunca são alterados). Pede confirmação e mostra o relatório. */
export function LightroomDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const toast = useStore((s) => s.toast);
  const reloadPhotos = useStore((s) => s.reloadPhotos);
  const [keepUserStars, setKeep] = useState(true);
  const [applyInApp, setApply] = useState(true);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<LightroomReport | null>(null);

  const run = async () => {
    setBusy(true);
    try {
      const r = await api.exportLightroomRatings({ keepUserStars, applyInApp });
      setReport(r);
      if (applyInApp) await reloadPhotos();
    } catch (e) {
      toast("error", errorText(e, t));
    } finally {
      setBusy(false);
    }
  };

  const footer = report ? (
    <button type="button" className="btn primary" data-close onClick={onClose}>
      {t("common.close")}
    </button>
  ) : (
    <>
      <button type="button" className="btn" data-close onClick={onClose}>
        {t("common.cancel")}
      </button>
      <button type="button" className="btn primary" disabled={busy} onClick={() => void run()}>
        {busy ? t("lightroom.running") : t("lightroom.run")}
      </button>
    </>
  );

  return (
    <Modal title={t("lightroom.title")} onClose={onClose} footer={footer}>
      {!report ? (
        <div className="lightroom-dialog">
          <ul className="lr-map" aria-label={t("lightroom.mapping")}>
            <li><span className="label-chip l-best">{t("label.best")}</span> → ★★★★★</li>
            <li><span className="label-chip l-alternative">{t("label.alternative")}</span> → ★★★</li>
            <li><span className="label-chip l-review">{t("label.review")}</span> → ★★</li>
          </ul>
          <p className="small">{t("lightroom.how")}</p>
          <p className="small muted">{t("lightroom.notCatalog")}</p>
          <label className="check">
            <input type="checkbox" checked={keepUserStars} onChange={(e) => setKeep(e.target.checked)} />
            <span>{t("lightroom.keepUserStars")}</span>
          </label>
          <label className="check">
            <input type="checkbox" checked={applyInApp} onChange={(e) => setApply(e.target.checked)} />
            <span>{t("lightroom.applyInApp")}</span>
          </label>
          <p className="notice small">{t("lightroom.safety")}</p>
        </div>
      ) : (
        <div className="lightroom-dialog" role="status">
          <p>
            <strong>{t("lightroom.done", { count: report.created + report.updated })}</strong>
          </p>
          <ul className="lr-map">
            <li>★★★★★ {report.byStars[2]}</li>
            <li>★★★ {report.byStars[1]}</li>
            <li>★★ {report.byStars[0]}</li>
          </ul>
          {report.updated > 0 && <p className="small muted">{t("lightroom.updated", { count: report.updated })}</p>}
          {report.starsInApp > 0 && <p className="small muted">{t("lightroom.inApp", { count: report.starsInApp })}</p>}
          {report.notRaw.length > 0 && <p className="notice warn small">{t("lightroom.notRaw", { count: report.notRaw.length })}</p>}
          {report.unavailable.length > 0 && <p className="notice warn small">{t("lightroom.unavailable", { count: report.unavailable.length })}</p>}
          {report.failed.length > 0 && (
            <div className="notice warn small">
              <p>{t("lightroom.failed", { count: report.failed.length })}</p>
              <ul>
                {report.failed.slice(0, 8).map(([n, m]) => (
                  <li key={n}>
                    {n}: {m}
                  </li>
                ))}
              </ul>
            </div>
          )}
          <p className="small">{t("lightroom.next")}</p>
        </div>
      )}
    </Modal>
  );
}
