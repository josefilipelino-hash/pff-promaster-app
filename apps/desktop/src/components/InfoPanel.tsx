import { open } from "@tauri-apps/plugin-dialog";
import { useEffect } from "react";
import { api } from "../api";
import { useTranslation } from "react-i18next";
import { cameraName, formatAperture, formatBytes, formatCaptureTime, formatExposure, formatFocal } from "../logic/format";
import { errorText, useStore } from "../store";
import type { Color, PhotoRow } from "../types";
import { AnalysisSection } from "./AnalysisSection";
import { OrganizationSection } from "./OrganizationSection";
import { ColorButtons, FlagButtons, Stars } from "./Rating";

/** Nome do formato: para RAW sobre TIFF usa-se a extensão (NEF, ARW, DNG…). */
export function formatLabel(format: string, fileName: string): string {
  if (format === "raw-tiff") {
    const ext = fileName.slice(fileName.lastIndexOf(".") + 1).toUpperCase();
    return `${ext} (RAW)`;
  }
  const raw = ["cr2", "cr3", "crw", "raf", "orf", "rw2", "mrw", "x3f"];
  return raw.includes(format) ? `${format.toUpperCase()} (RAW)` : format.toUpperCase();
}

export function InfoPanel({
  photo,
  selectedCount,
  onRate,
    onCompareGroup,
  onAccept,
  onMoment,
  onResetMoment,
  onPerson,
}: {
  photo: PhotoRow | null;
  selectedCount: number;
  onRate: (kind: "stars" | "flag" | "color", value: number | Color | null) => void;
    onCompareGroup?: (ids: number[]) => void;
  onAccept?: (id: number) => void;
  onMoment?: (ids: number[], moment: number | null) => void;
  onResetMoment?: (ids: number[]) => void;
  onPerson?: (id: number) => void;
}) {
  const { t, i18n } = useTranslation();
  const details = useStore((s) => (photo ? s.details.get(photo.asset ?? photo.id) : undefined));
  const loadDetails = useStore((s) => s.loadDetails);

  useEffect(() => {
    if (photo) void loadDetails(photo.asset ?? photo.id);
  }, [photo, loadDetails]);

  if (!photo) {
    return (
      <aside className="info-panel">
        <p className="muted">{t("info.none")}</p>
      </aside>
    );
  }
  const d = details;
  const rows: [string, string | null | undefined][] = d
    ? [
        [t("info.captured"), formatCaptureTime(d.captureTime, i18n.language) ?? t("info.unknown")],
        [t("info.camera"), cameraName(d.cameraMake, d.cameraModel)],
        [t("info.lens"), d.lens],
        [t("info.iso"), d.iso ? `ISO ${d.iso}` : null],
        [t("info.shutter"), formatExposure(d.exposureTime)],
        [t("info.aperture"), formatAperture(d.fNumber)],
        [t("info.focal"), formatFocal(d.focalLength, d.focalLength35mm)],
        [t("info.dimensions"), `${photo.width} × ${photo.height} px (${((photo.width * photo.height) / 1e6).toFixed(1).replace(".", ",")} MP)`],
        [t("info.size"), formatBytes(d.fileSize, i18n.language)],
        [t("info.format"), formatLabel(d.format, d.fileName)],
        [t("info.bitDepth"), d.bitDepth ? t("info.bits", { count: d.bitDepth }) : null],
        [t("info.colorSpace"), d.colorSpace],
        [t("info.embeddedPreview"), d.embeddedPreview],
        [t("info.codec"), d.codec ? [d.codec, d.codecDetail].filter(Boolean).join(" · ") : null],
        [t("info.orientation"), d.orientation === 1 ? t("info.orientationNormal") : t("info.orientationValue", { value: d.orientation })],
      ]
    : [];

  return (
    <aside className="info-panel" aria-label={t("info.title")}>
      {selectedCount > 1 && <p className="badge">{t("info.applyToSelection", { count: selectedCount })}</p>}
      <h3 className="info-name" title={photo.fileName}>
        {photo.fileName}
      </h3>
      <div className="info-rating">
        <Stars value={photo.stars} onChange={(n) => onRate("stars", n)} />
        <FlagButtons value={photo.flag} onChange={(v) => onRate("flag", v)} />
        <ColorButtons value={photo.color} onChange={(c) => onRate("color", c)} />
      </div>
      {photo.thumb === "failed" && (
        <div className="notice error" role="alert">
          <strong>{t("info.unreadableTitle")}</strong>
          <span>{t("info.unreadableText")}</span>
          {d?.thumbError && <small>{d.thumbError}</small>}
        </div>
      )}
      {d?.partialNote && (
        <div className="notice warn">
          <strong>{t("info.partialTitle")}</strong>
          <span>{d.partialNote}</span>
        </div>
      )}
            <OrganizationSection photo={photo} onMoment={(m) => onMoment?.([photo.id], m)} onReset={() => onResetMoment?.([photo.id])} onPerson={(id) => onPerson?.(id)} />
      <AnalysisSection photo={photo} onCompareGroup={onCompareGroup ?? (() => {})} onAccept={onAccept ?? (() => {})} />
      <dl className="info-list">
        {rows
          .filter(([, v]) => v)
          .map(([k, v]) => (
            <div key={k}>
              <dt>{k}</dt>
              <dd>{v}</dd>
            </div>
          ))}
      </dl>
      {d && (
        <>
          <dl className="info-path">
            <dt>{t("info.path")}</dt>
          <dd title={d.path}>{d.path}</dd>
          {d.sourcePath && (
            <>
              <dt>{t("info.copiedFrom")}</dt>
              <dd title={d.sourcePath}>{d.sourcePath}</dd>
            </>
          )}
          </dl>
          <OriginalActions photo={photo} />
        </>
      )}
    </aside>
  );
}

/** Original: mostrar no gestor de ficheiros; indisponível → aviso e localizar (relink). */
function OriginalActions({ photo }: { photo: PhotoRow }) {
  const { t } = useTranslation();
  const toast = useStore((s) => s.toast);
  const reloadPhotos = useStore((s) => s.reloadPhotos);
  const av = photo.availability ?? "ok";
  const fail = (e: unknown) => toast("error", errorText(e, t));
  const relink = async () => {
    const f = await open({ multiple: false, title: t("safety.locate") });
    if (typeof f !== "string") return;
    try {
      const r = await api.relinkFile(photo.id, f);
      toast("info", t("safety.relinked", { count: r.relinked, left: r.stillUnavailable }));
      await reloadPhotos();
    } catch (e) {
      fail(e);
    }
  };
  return (
    <div className="original-actions">
      {av !== "ok" && (
        <p className="notice warn small" role="status">
          {t(av === "offline" ? "safety.photoOffline" : "safety.photoMissing")}
        </p>
      )}
      <div className="dev-row">
        {av === "ok" ? (
          <button type="button" className="btn small" onClick={() => api.revealOriginal(photo.id).catch(fail)}>
            {t("safety.reveal")}
          </button>
        ) : (
          <button type="button" className="btn small" onClick={() => void relink()}>
            {t("safety.locate")}
          </button>
        )}
      </div>
    </div>
  );
}
