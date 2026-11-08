import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { ImportMode, ImportSummary } from "../types";
import { Modal } from "./Modal";

export function ImportDialog({
  paths,
  originalsDir,
  onCancel,
  onStart,
}: {
  paths: string[];
  originalsDir: string | null;
  onCancel: () => void;
  onStart: (mode: ImportMode, recursive: boolean) => void;
}) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<ImportMode>("reference");
  const [recursive, setRecursive] = useState(true);
  return (
    <Modal
      title={t("import.title")}
      onClose={onCancel}
      footer={
        <>
          <button type="button" className="btn" data-close onClick={onCancel}>
            {t("common.cancel")}
          </button>
          <button type="button" className="btn primary" onClick={() => onStart(mode, recursive)}>
            {t("import.start")}
          </button>
        </>
      }
    >
      <p>{t("import.selected", { count: paths.length })}</p>
      <ul className="path-list">
        {paths.slice(0, 6).map((p) => (
          <li key={p} title={p}>
            {p}
          </li>
        ))}
        {paths.length > 6 && <li className="muted">{t("import.andMore", { count: paths.length - 6 })}</li>}
      </ul>
      <fieldset className="options">
        <legend>{t("import.modeLabel")}</legend>
        <label className="radio">
          <input type="radio" name="mode" checked={mode === "reference"} onChange={() => setMode("reference")} />
          <span>
            <strong>{t("import.reference")}</strong>
            <small className="muted">{t("import.referenceHint")}</small>
          </span>
        </label>
        <label className={`radio${originalsDir ? "" : " disabled"}`}>
          <input type="radio" name="mode" checked={mode === "copy"} disabled={!originalsDir} onChange={() => setMode("copy")} />
          <span>
            <strong>{t("import.copy")}</strong>
            <small className="muted">{originalsDir ? t("import.copyHint", { path: originalsDir }) : t("import.copyUnavailable")}</small>
          </span>
        </label>
      </fieldset>
      <label className="checkbox">
        <input type="checkbox" checked={recursive} onChange={(e) => setRecursive(e.target.checked)} />
        <span>{t("import.recursive")}</span>
      </label>
      <p className="muted small">{t("import.formatsNote")}</p>
    </Modal>
  );
}

export function ImportReport({ summary, onClose }: { summary: ImportSummary; onClose: () => void }) {
  const { t } = useTranslation();
  const c = summary.counts;
  const rows: [string, number][] = [
    [t("report.imported"), c.imported],
    [t("report.duplicate"), c.duplicate],
    [t("report.alreadyImported"), c.alreadyImported],
    [t("report.unsupported"), c.unsupported],
    [t("report.missingCodec"), c.missingCodec],
    [t("report.corrupt"), c.corrupt],
    [t("report.error"), c.error],
    [t("report.pending"), c.pending],
  ];
  return (
    <Modal
      title={summary.cancelled ? t("report.titleCancelled") : t("report.title")}
      onClose={onClose}
      wide
      footer={
        <button type="button" className="btn primary" onClick={onClose}>
          {t("common.close")}
        </button>
      }
    >
      {summary.error && <p className="form-error">{summary.error}</p>}
      <dl className="report-counts">
        {rows
          .filter(([, n], i) => n > 0 || i === 0)
          .map(([k, n]) => (
            <div key={k}>
              <dt>{k}</dt>
              <dd>{n.toLocaleString()}</dd>
            </div>
          ))}
      </dl>
      {(c.missingCodec > 0 || c.corrupt > 0 || c.unsupported > 0) && (
        <ul className="legend">
          {c.corrupt > 0 && <li><span className="issue-state s-corrupt">{t("itemState.corrupt")}</span> {t("report.corruptHint")}</li>}
          {c.missingCodec > 0 && <li><span className="issue-state s-missingCodec">{t("itemState.missingCodec")}</span> {t("report.missingCodecHint")}</li>}
          {c.unsupported > 0 && <li><span className="issue-state s-unsupported">{t("itemState.unsupported")}</span> {t("report.unsupportedHint")}</li>}
        </ul>
      )}
      {summary.issues.length > 0 && (
        <>
          <h3 className="section-title">{t("report.details")}</h3>
          <table className="issues">
            <tbody>
              {summary.issues.map((i) => (
                <tr key={i.sourcePath}>
                  <td className={`issue-state s-${i.state}`}>{t(`itemState.${i.state}`)}</td>
                  <td className="issue-path" title={i.sourcePath}>
                    {i.sourcePath}
                    {i.message && <small className="muted"> — {i.message}</small>}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </Modal>
  );
}
