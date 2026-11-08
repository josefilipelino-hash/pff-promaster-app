import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { cameraName, formatAperture, formatBytes, formatCaptureTime, formatExposure, formatFocal } from "../logic/format";
import { useStore } from "../store";
import type { Color, PhotoRow } from "../types";
import { ColorButtons, FlagButtons, Stars } from "./Rating";

export function InfoPanel({
  photo,
  selectedCount,
  onRate,
}: {
  photo: PhotoRow | null;
  selectedCount: number;
  onRate: (kind: "stars" | "flag" | "color", value: number | Color | null) => void;
}) {
  const { t, i18n } = useTranslation();
  const details = useStore((s) => (photo ? s.details.get(photo.id) : undefined));
  const loadDetails = useStore((s) => s.loadDetails);

  useEffect(() => {
    if (photo) void loadDetails(photo.id);
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
        [t("info.format"), d.format.toUpperCase()],
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
      {photo.thumb === "failed" && d?.thumbError && (
        <p className="form-error">
          {t("info.thumbError")}: {d.thumbError}
        </p>
      )}
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
        <div className="info-path">
          <dt>{t("info.path")}</dt>
          <dd title={d.path}>{d.path}</dd>
          {d.sourcePath && (
            <>
              <dt>{t("info.copiedFrom")}</dt>
              <dd title={d.sourcePath}>{d.sourcePath}</dd>
            </>
          )}
        </div>
      )}
    </aside>
  );
}
