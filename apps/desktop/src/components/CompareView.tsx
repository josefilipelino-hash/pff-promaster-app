import { Icon } from "./Icon";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { imageUrl } from "../api";
import { explain, parseReasons } from "../logic/explain";
import { useStore } from "../store";
import type { PhotoRow } from "../types";
import { LabelChip } from "./LabelChip";
import { Stars } from "./Rating";

const MAX_Z = 32;

/**
 * Vista sincronizada partilhada pelos painéis: `z` é o zoom relativo ao “ajustar” de cada
 * painel (1 = ajustar) e (`cx`, `cy`) é o ponto da imagem, em coordenadas normalizadas
 * 0–1, que fica no centro de cada painel. Assim, fotografias com dimensões diferentes
 * mostram sempre a mesma zona relativa (ex.: os rostos de uma rajada).
 */
export interface SyncView {
  z: number;
  cx: number;
  cy: number;
}
export const FIT_VIEW: SyncView = { z: 1, cx: 0.5, cy: 0.5 };

/** Escala css (px de ecrã por px de imagem) para “ajustar”. */
export function fitCss(w: number, h: number, bw: number, bh: number): number {
  if (w <= 0 || h <= 0 || bw <= 0 || bh <= 0) return 1;
  return Math.min(bw / w, bh / h);
}

/** Centro efetivo num painel: se a imagem não enche o painel nesse eixo, fica centrada. */
export function clampCenter(c: number, disp: number, viewport: number): number {
  if (disp <= viewport) return 0.5;
  const half = viewport / (2 * disp);
  return Math.min(1 - half, Math.max(half, c));
}

/** Zoom à volta de um ponto (`mx`, `my` relativos ao centro do painel, em px css). */
export function zoomAt(v: SyncView, nz: number, mx: number, my: number, dispW: number, dispH: number): SyncView {
  const z = Math.min(MAX_Z, Math.max(1, nz));
  if (z === 1) return FIT_VIEW;
  const px = v.cx + mx / dispW;
  const py = v.cy + my / dispH;
  const k = v.z / z;
  return { z, cx: px - (mx / dispW) * k, cy: py - (my / dispH) * k };
}

export interface CompareHandle {
  toggleZoom(): void;
  zoom(dir: 1 | -1): void;
}

function Pane({
  photo,
  view,
  setView,
  active,
  onActivate,
  onRate,
  onSwap,
  token,
}: {
  photo: PhotoRow;
  view: SyncView;
  setView: (v: SyncView) => void;
  active: boolean;
  onActivate: () => void;
  onRate: (kind: "stars" | "flag", value: number) => void;
  onSwap: (id: number) => void;
  token: number;
}) {
  const { t, i18n } = useTranslation();
  const stage = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ w: 400, h: 300 });
  const [fullLoaded, setFullLoaded] = useState(false);
  const drag = useRef<{ x: number; y: number; v: SyncView } | null>(null);
  const rec = useStore((s) => s.recs.get(photo.id));
  const details = useStore((s) => s.analysisDetails.get(photo.id));
  const group = useStore((s) => (rec?.groupId != null ? s.groupById.get(rec.groupId) : undefined));
  const photos = useStore((s) => s.photos);
  const index = useStore((s) => s.index);
  const recs = useStore((s) => s.recs);
    const loadAnalysisDetails = useStore((s) => s.loadAnalysisDetails);
  const version = useStore((s) => s.thumbVersion.get(photo.id) ?? 0);
  const dpr = window.devicePixelRatio || 1;

  useLayoutEffect(() => {
    const el = stage.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setBox({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    setBox({ w: el.clientWidth, h: el.clientHeight });
    return () => ro.disconnect();
  }, []);
  useEffect(() => {
    setFullLoaded(false);
    void loadAnalysisDetails(photo.id).catch(() => {});
  }, [photo.id, loadAnalysisDetails]);

  const fit = fitCss(photo.width, photo.height, box.w, box.h);
  const dispW = photo.width * fit * view.z;
  const dispH = photo.height * fit * view.z;
  const cx = clampCenter(view.cx, dispW, box.w);
  const cy = clampCenter(view.cy, dispH, box.h);
  const tx = (0.5 - cx) * dispW;
  const ty = (0.5 - cy) * dispH;
  const needFull = Math.max(dispW, dispH) * dpr > Math.min(2560, Math.max(photo.width, photo.height)) * 1.05;
  const rel = (e: { clientX: number; clientY: number }) => {
    const r = stage.current!.getBoundingClientRect();
    return { x: e.clientX - r.left - r.width / 2, y: e.clientY - r.top - r.height / 2 };
  };
  const r = details?.recommendation;
  const sentence = r ? explain(r.summary, parseReasons(r.reasons), t, i18n.language) : null;

  return (
    <div className={`compare-pane${active ? " active" : ""}${photo.flag === -1 ? " rejected" : ""}`} onMouseDown={onActivate}>
      <header className="compare-pane-header">
        <span className="compare-name" title={photo.fileName}>
          {photo.fileName}
        </span>
        {rec && <LabelChip label={rec.label} />}
        {rec && rec.groupSize > 1 && <span className="muted small">{t("compare.rank", { rank: rec.rank + 1, total: rec.groupSize })}</span>}
      </header>
      <div
        ref={stage}
        className={`compare-stage${view.z > 1 ? " zoomed" : ""}`}
        onWheel={(e) => {
          const m = rel(e);
          setView(zoomAt({ ...view, cx, cy }, view.z * Math.exp(-e.deltaY * 0.0015), m.x, m.y, dispW, dispH));
        }}
        onDoubleClick={(e) => {
          const m = rel(e);
          setView(view.z > 1 ? FIT_VIEW : zoomAt(view, 1 / (fit * dpr), m.x, m.y, dispW, dispH));
        }}
        onPointerDown={(e) => {
          if (view.z <= 1 || e.button !== 0) return;
          (e.target as HTMLElement).setPointerCapture(e.pointerId);
          drag.current = { x: e.clientX, y: e.clientY, v: { ...view, cx, cy } };
        }}
        onPointerMove={(e) => {
          const d = drag.current;
          if (!d) return;
          setView({ z: d.v.z, cx: d.v.cx - (e.clientX - d.x) / dispW, cy: d.v.cy - (e.clientY - d.y) / dispH });
        }}
        onPointerUp={() => (drag.current = null)}
        onPointerCancel={() => (drag.current = null)}
      >
        <div className="viewer-image" style={{ width: dispW, height: dispH, transform: `translate(${tx}px, ${ty}px)` }}>
          <img src={`${imageUrl(token, "preview", photo.id)}?v=${version}`} alt={photo.fileName} draggable={false} />
          {needFull && <img key={`full-${photo.id}`} className={`full${fullLoaded ? " loaded" : ""}`} src={`${imageUrl(token, "full", photo.id)}?v=${version}`} alt="" draggable={false} onLoad={() => setFullLoaded(true)} />}
        </div>
        {needFull && !fullLoaded && <div className="viewer-loading state loading" role="status">{t("viewer.loadingFull")}</div>}
      </div>
      <footer className="compare-pane-footer">
        <Stars value={photo.stars} onChange={(n) => onRate("stars", n)} />
        <button type="button" className={`btn small${photo.flag === 1 ? " active" : ""}`} aria-pressed={photo.flag === 1} onClick={() => onRate("flag", photo.flag === 1 ? 0 : 1)} title={`${t("compare.accept")} (P)`}>
          ♥ {t("compare.accept")}
        </button>
        <button type="button" className={`btn small${photo.flag === -1 ? " active" : ""}`} aria-pressed={photo.flag === -1} onClick={() => onRate("flag", photo.flag === -1 ? 0 : -1)} title={`${t("compare.reject")} (X)`}>
          ✕ {t("compare.reject")}
        </button>
        {group && group.members.length > 1 && (
          <select className="compare-swap" value="" onChange={(e) => e.target.value && onSwap(Number(e.target.value))} aria-label={t("compare.swap")} title={t("compare.swapHint")}>
            <option value="">{t("compare.swap")}</option>
            {group.members
              .filter((m) => m !== photo.id)
              .map((m) => {
                const p = photos[index.get(m) ?? -1];
                const l = recs.get(m)?.label;
                return (
                  <option key={m} value={m}>
                    {(p?.fileName ?? m) + (l ? ` — ${t(`label.${l}`)}` : "")}
                  </option>
                );
              })}
          </select>
        )}
      </footer>
      {sentence && <p className="compare-why small">{sentence}</p>}
    </div>
  );
}

/** Comparação lado a lado (2–4) com zoom e deslocamento sincronizados. */
export function CompareView({
  ids,
  active,
  setActive,
  onClose,
  onRate,
  onReplace,
  handleRef,
}: {
  ids: number[];
  active: number;
  setActive: (i: number) => void;
  onClose: () => void;
  onRate: (kind: "stars" | "flag", value: number, ids: number[]) => void;
  onReplace: (slot: number, id: number) => void;
  handleRef: React.MutableRefObject<CompareHandle | null>;
}) {
  const { t } = useTranslation();
  const photos = useStore((s) => s.photos);
  const index = useStore((s) => s.index);
  const token = useStore((s) => s.project?.imageToken ?? 0);
  const [view, setView] = useState<SyncView>(FIT_VIEW);
  const [synced, setSynced] = useState(true);
  const [own, setOwn] = useState<SyncView[]>(() => ids.map(() => FIT_VIEW));
  const list = ids.map((id) => photos[index.get(id) ?? -1]).filter((p): p is PhotoRow => !!p);

  useEffect(() => setOwn((o) => ids.map((_, i) => o[i] ?? FIT_VIEW)), [ids]);
  const viewOf = (i: number) => (synced ? view : (own[i] ?? FIT_VIEW));
  const setViewOf = useCallback(
    (i: number, v: SyncView) => {
      if (synced) setView(v);
      else setOwn((o) => o.map((x, k) => (k === i ? v : x)));
    },
    [synced],
  );

  handleRef.current = {
    toggleZoom: () => setViewOf(active, viewOf(active).z > 1 ? FIT_VIEW : { ...viewOf(active), z: 3 }),
    zoom: (dir) => {
      const v = viewOf(active);
      setViewOf(active, dir > 0 ? { ...v, z: Math.min(MAX_Z, v.z * 1.5) } : v.z / 1.5 <= 1.01 ? FIT_VIEW : { ...v, z: v.z / 1.5 });
    },
  };

  const cols = list.length === 4 ? 2 : list.length;
  return (
    <div className="compare" role="region" aria-label={t("compare.title")}>
      <div className="viewer-toolbar">
        <button type="button" className="btn small ghost" onClick={onClose} title={`${t("viewer.back")} (Esc)`}>
          <Icon name="chevronLeft" />
          {t("viewer.back")}
        </button>
        <strong>{t("compare.title")}</strong>
        <span className="kbd-hints" title={t("compare.hint")}>
          <kbd>←</kbd>
          <kbd>→</kbd> {t("compare.keyPane")} <kbd>P</kbd>
          <kbd>X</kbd>
          <kbd>0–5</kbd> {t("compare.keyRate")}
        </span>
        <span className="spacer" />
        <label className="small">
          <input type="checkbox" checked={synced} onChange={(e) => setSynced(e.target.checked)} /> {t("compare.sync")}
        </label>
        <span className="sep" />
        <button type="button" className="btn small" onClick={() => (synced ? setView(FIT_VIEW) : setOwn(ids.map(() => FIT_VIEW)))} title={`${t("viewer.fit")} (Z)`}>
          {t("viewer.fit")}
        </button>
        <button type="button" className="btn small" onClick={() => handleRef.current?.zoom(-1)} title={`${t("viewer.zoomOut")} (−)`}>
          −
        </button>
        <button type="button" className="btn small" onClick={() => handleRef.current?.zoom(1)} title={`${t("viewer.zoomIn")} (+)`}>
          +
        </button>
      </div>
      <div className="compare-grid" style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}>
        {list.map((p, i) => (
          <Pane
            key={`${i}-${p.id}`}
            photo={p}
            view={viewOf(i)}
            setView={(v) => setViewOf(i, v)}
            active={i === active}
            onActivate={() => setActive(i)}
            onRate={(k, v) => onRate(k, v, [p.id])}
            onSwap={(id) => onReplace(i, id)}
            token={token}
          />
        ))}
      </div>
    </div>
  );
}
