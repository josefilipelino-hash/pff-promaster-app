import { useTranslation } from "react-i18next";
import { THUMB_MAX, THUMB_MIN, useStore } from "../store";
import { COLORS } from "../types";

export function StatusBar({ total, visible, selected, onCancelImport, showSize }: { total: number; visible: number; selected: number; onCancelImport: () => void; showSize: boolean }) {
  const { t } = useTranslation();
  const progress = useStore((s) => s.importProgress);
  const running = useStore((s) => s.importRunning);
  const thumbs = useStore((s) => s.thumbProgress);
  const thumbSize = useStore((s) => s.thumbSize);
  const setThumbSize = useStore((s) => s.setThumbSize);
  const filter = useStore((s) => s.filter);
  const setFilter = useStore((s) => s.setFilter);

  return (
    <footer className="statusbar">
      <span>
        {t("status.photos", { count: total })}
        {visible !== total && ` · ${t("status.visible", { count: visible })}`}
        {selected > 0 && ` · ${t("status.selected", { count: selected })}`}
      </span>
      <span className="sep" />
      <label className="filter-select" title={t("filter.minStars")}>
        <span className="sr-only">{t("filter.minStars")}</span>
        <select value={filter.minStars} onChange={(e) => setFilter({ minStars: Number(e.target.value) })}>
          <option value={0}>{t("filter.anyStars")}</option>
          {[1, 2, 3, 4, 5].map((n) => (
            <option key={n} value={n}>
              {"≥ " + "★".repeat(n)}
            </option>
          ))}
        </select>
      </label>
      <span className="color-filter" role="group" aria-label={t("filter.color")}>
        {COLORS.map((c) => (
          <button
            key={c}
            type="button"
            className={`color-dot c-${c}${filter.color === c ? " on" : ""}`}
            title={t("filter.onlyColor", { color: t(`color.${c}`) })}
            aria-pressed={filter.color === c}
            onClick={() => setFilter({ color: filter.color === c ? null : c })}
          />
        ))}
      </span>
      <span className="spacer" />
      {running && progress && (
        <span className="progress-item">
          <span>{t("status.importing", { done: progress.processed, total: progress.total })}</span>
          <progress max={Math.max(1, progress.total)} value={progress.processed} />
          <button type="button" className="btn small ghost" onClick={onCancelImport}>
            {t("status.cancelImport")}
          </button>
        </span>
      )}
      {running && !progress && <span className="progress-item">{t("status.scanning")}</span>}
      {thumbs && (
        <span className="progress-item">
          <span>{t("status.thumbnails", { remaining: thumbs.remaining })}</span>
          <progress max={Math.max(1, thumbs.total)} value={thumbs.done} />
        </span>
      )}
      {showSize && (
        <label className="size-slider" title={t("status.thumbSize")}>
          <span aria-hidden>▫</span>
          <input type="range" min={THUMB_MIN} max={THUMB_MAX} step={10} value={thumbSize} aria-label={t("status.thumbSize")} onChange={(e) => setThumbSize(Number(e.target.value))} />
          <span aria-hidden>◻</span>
        </label>
      )}
    </footer>
  );
}
