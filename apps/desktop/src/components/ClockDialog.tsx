import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import { formatOffset, parseOffset } from "../logic/clock";
import { formatCaptureTime } from "../logic/format";
import { errorText, useStore } from "../store";
import type { CameraRow, OffsetSuggestion } from "../types";
import { Modal } from "./Modal";

function cameraLabel(c: CameraRow, unknown: string): string {
  return [c.make, c.model, c.serial ? `nº ${c.serial}` : null].filter(Boolean).join(" ") || unknown;
}

/**
 * Sincronização de hora entre câmaras: referência, desvios e sugestão automática.
 * O utilizador escreve o **desvio do relógio** (“Sony = +3min42s” = adiantada); a correção
 * guardada é o inverso (−3min42s), somada à hora EXIF original, que nunca é alterada.
 */
export function ClockDialog({ onClose, onApplied }: { onClose: () => void; onApplied: () => void }) {
  const { t, i18n } = useTranslation();
  const toast = useStore((s) => s.toast);
  const [cams, setCams] = useState<CameraRow[] | null>(null);
  const [reference, setReference] = useState<string>("");
  const [text, setText] = useState<Record<string, string>>({});
  const [sugg, setSugg] = useState<Record<string, OffsetSuggestion>>({});
  const [busy, setBusy] = useState(false);
  const fail = (e: unknown) => toast("error", errorText(e, t));

  useEffect(() => {
    api
      .listCameras()
      .then((c) => {
        setCams(c);
        setReference((c.find((x) => x.reference) ?? c[0])?.key ?? "");
        setText(Object.fromEntries(c.map((x) => [x.key, formatOffset(-x.offsetMs)])));
      })
      .catch(fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const suggest = async () => {
    setBusy(true);
    try {
      const s = await api.suggestOffsets(reference);
      setSugg(Object.fromEntries(s.map((x) => [x.camera, x])));
      setText((old) => ({ ...old, ...Object.fromEntries(s.map((x) => [x.camera, formatOffset(-x.offsetMs)])) }));
      if (s.length === 0) toast("info", t("clock.noSuggestion"));
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const invalid = cams?.filter((c) => c.key !== reference && parseOffset(text[c.key] ?? "") === null) ?? [];
  const apply = async () => {
    if (!cams) return;
    setBusy(true);
    try {
      await api.setCameraOffsets(cams.map((c) => ({ key: c.key, offsetMs: c.key === reference ? 0 : -(parseOffset(text[c.key] ?? "0") ?? 0), reference: c.key === reference })));
      toast("info", t("clock.applied"));
      onApplied();
      onClose();
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title={t("clock.title")}
      onClose={onClose}
      wide
      footer={
        <>
          <button type="button" className="btn" onClick={onClose} data-close>
            {t("common.cancel")}
          </button>
          <button type="button" className="btn" disabled={busy || !cams || cams.length < 2} onClick={() => void suggest()}>
            {t("clock.suggest")}
          </button>
          <button type="button" className="btn primary" disabled={busy || !cams || invalid.length > 0} onClick={() => void apply()}>
            {t("clock.apply")}
          </button>
        </>
      }
    >
      <p className="small muted">{t("clock.intro")}</p>
      {!cams ? (
        <p className="muted">{t("common.loading")}</p>
      ) : (
        <table className="clock-table">
          <thead>
            <tr>
              <th>{t("clock.reference")}</th>
              <th>{t("clock.camera")}</th>
              <th>{t("clock.photos")}</th>
              <th>{t("clock.period")}</th>
              <th>{t("clock.offset")}</th>
            </tr>
          </thead>
          <tbody>
            {cams.map((c) => {
              const s = sugg[c.key];
              const bad = invalid.some((x) => x.key === c.key);
              return (
                <tr key={c.key}>
                  <td>
                    <input type="radio" name="clock-ref" checked={reference === c.key} onChange={() => setReference(c.key)} aria-label={t("clock.referenceFor", { camera: cameraLabel(c, t("clock.unknownCamera")) })} />
                  </td>
                  <td>{cameraLabel(c, t("clock.unknownCamera"))}</td>
                  <td>{c.photos.toLocaleString()}</td>
                  <td className="small muted">
                    {formatCaptureTime(c.firstTime, i18n.language) ?? "—"} → {formatCaptureTime(c.lastTime, i18n.language) ?? "—"}
                  </td>
                  <td>
                    {reference === c.key ? (
                      <span className="muted small">{t("clock.isReference")}</span>
                    ) : (
                      <>
                        <input
                          className={`offset-input${bad ? " invalid" : ""}`}
                          value={text[c.key] ?? ""}
                          onChange={(e) => setText({ ...text, [c.key]: e.target.value })}
                          aria-label={t("clock.offsetFor", { camera: cameraLabel(c, t("clock.unknownCamera")) })}
                          placeholder="+3min42s"
                        />
                        {s && <span className="small muted suggestion">{t("clock.suggestion", { votes: s.votes, agreement: Math.round(s.agreement * 100) })}</span>}
                      </>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
      <p className="small muted">{t("clock.formatHint")}</p>
    </Modal>
  );
}
