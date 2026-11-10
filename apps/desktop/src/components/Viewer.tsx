import { perfViewerLoaded, perfViewerShown } from "../logic/perfProbe";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { imageUrl } from "../api";
import { useStore } from "../store";
import type { PhotoRow } from "../types";

/** Níveis de zoom em “píxeis da imagem por píxel físico do ecrã” (1 = 100 %). */
export const ZOOM_STEPS = [0.125, 0.25, 0.5, 1, 2, 4, 8];
const MAX_ZOOM = 8;

export type Zoom = "fit" | number;

export function fitScale(w: number, h: number, cw: number, ch: number, dpr: number): number {
  if (w <= 0 || h <= 0 || cw <= 0 || ch <= 0) return 1;
  return Math.min((cw * dpr) / w, (ch * dpr) / h, 1);
}

/** Próximo nível de zoom a partir da escala atual. */
export function stepZoom(current: number, dir: 1 | -1): number {
  if (dir > 0) return ZOOM_STEPS.find((z) => z > current + 1e-6) ?? MAX_ZOOM;
  return [...ZOOM_STEPS].reverse().find((z) => z < current - 1e-6) ?? ZOOM_STEPS[0]!;
}

export interface ViewerHandle {
  toggleZoom(): void;
  zoom(dir: 1 | -1): void;
}

export function Viewer({
  photo,
  token,
  index,
  total,
  onPrev,
  onNext,
  onClose,
  onFullscreen,
  handleRef,
}: {
  photo: PhotoRow;
  token: number;
  index: number;
  total: number;
  onPrev: () => void;
  onNext: () => void;
  onClose: () => void;
  onFullscreen: () => void;
  handleRef: React.MutableRefObject<ViewerHandle | null>;
}) {
    const { t } = useTranslation();
  // Versões editadas mudam de URL a cada gravação (o protocolo serve ficheiros imutáveis).
  const version = useStore((s) => s.thumbVersion.get(photo.id) ?? 0);
  const stage = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ w: 800, h: 600 });
  const [zoom, setZoom] = useState<Zoom>("fit");
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [fullLoaded, setFullLoaded] = useState(false);
  const [previewFailed, setPreviewFailed] = useState(false);
  const drag = useRef<{ x: number; y: number; ox: number; oy: number } | null>(null);
  const dpr = window.devicePixelRatio || 1;

  useLayoutEffect(() => {
    const el = stage.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setBox({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    setBox({ w: el.clientWidth, h: el.clientHeight });
    return () => ro.disconnect();
  }, []);

  // Nova fotografia: volta a “ajustar”.
  useEffect(() => {
    perfViewerShown(photo.id);
    setZoom("fit");
    setOffset({ x: 0, y: 0 });
    setFullLoaded(false);
    setPreviewFailed(false);
  }, [photo.id]);

  const fit = fitScale(photo.width, photo.height, box.w, box.h, dpr);
  const scale = zoom === "fit" ? fit : zoom;
  const cssW = (photo.width * scale) / dpr;
  const cssH = (photo.height * scale) / dpr;
  const clamp = useCallback(
    (o: { x: number; y: number }, s: number) => {
      const w = (photo.width * s) / dpr;
      const h = (photo.height * s) / dpr;
      const mx = Math.max(0, (w - box.w) / 2);
      const my = Math.max(0, (h - box.h) / 2);
      return { x: Math.max(-mx, Math.min(mx, o.x)), y: Math.max(-my, Math.min(my, o.y)) };
    },
    [photo.width, photo.height, dpr, box.w, box.h],
  );

  // Quando o palco muda de tamanho (ex.: ecrã completo), o deslocamento volta a ser limitado.
  useEffect(() => {
    setOffset((o) => {
      const c = clamp(o, scale);
      return c.x === o.x && c.y === o.y ? o : c;
    });
  }, [clamp, scale]);

  /** Altera a escala mantendo fixo o ponto `p` (relativo ao centro do palco). */
  const zoomAround = useCallback(
    (next: Zoom, p = { x: 0, y: 0 }) => {
      const ns = next === "fit" ? fit : Math.min(MAX_ZOOM, next);
      const ratio = ns / scale;
      setOffset((o) => (next === "fit" ? { x: 0, y: 0 } : clamp({ x: (o.x - p.x) * ratio + p.x, y: (o.y - p.y) * ratio + p.y }, ns)));
      setZoom(next === "fit" || Math.abs(ns - fit) < 1e-6 ? "fit" : ns);
    },
    [fit, scale, clamp],
  );

  const toggleZoom = useCallback((p?: { x: number; y: number }) => zoomAround(zoom === "fit" ? 1 : "fit", p), [zoom, zoomAround]);

  handleRef.current = {
    toggleZoom: () => toggleZoom(),
    zoom: (dir) => zoomAround(stepZoom(scale, dir)),
  };

  const rel = (e: { clientX: number; clientY: number }) => {
    const r = stage.current!.getBoundingClientRect();
    return { x: e.clientX - r.left - r.width / 2, y: e.clientY - r.top - r.height / 2 };
  };

  const onWheel = (e: React.WheelEvent) => {
    const factor = Math.exp(-e.deltaY * 0.0015);
    const next = Math.max(Math.min(fit, ZOOM_STEPS[0]!), Math.min(MAX_ZOOM, scale * factor));
    zoomAround(next <= fit + 1e-6 ? "fit" : next, rel(e));
  };

  const onPointerDown = (e: React.PointerEvent) => {
    if (zoom === "fit" || e.button !== 0) return;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, y: e.clientY, ox: offset.x, oy: offset.y };
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current;
    if (!d) return;
    setOffset(clamp({ x: d.ox + e.clientX - d.x, y: d.oy + e.clientY - d.y }, scale));
  };
  const onPointerUp = () => {
    drag.current = null;
  };

  // A resolução total só é pedida quando o zoom ultrapassa o que a preview (2560 px) cobre.
  const previewEdge = Math.min(2560, Math.max(photo.width, photo.height));
  const needFull = Math.max(cssW, cssH) * dpr > previewEdge * 1.05;
  const zoomLabel = `${Math.round(scale * 100)} %`;

  return (
    <div className="viewer">
      <div className="viewer-toolbar">
        <button type="button" className="btn small" onClick={onClose} title={`${t("viewer.back")} (Esc)`}>
          ← {t("viewer.back")}
        </button>
        <span className="viewer-title" title={photo.fileName}>
          {photo.fileName} <span className="muted">· {t("viewer.position", { index: index + 1, total })}</span>
        </span>
        <span className="spacer" />
        <button type="button" className="btn small" onClick={onPrev} disabled={index <= 0} title={`${t("viewer.prev")} (←)`}>
          ‹ {t("viewer.prev")}
        </button>
        <button type="button" className="btn small" onClick={onNext} disabled={index >= total - 1} title={`${t("viewer.next")} (→)`}>
          {t("viewer.next")} ›
        </button>
        <span className="sep" />
        <button type="button" className={`btn small${zoom === "fit" ? " active" : ""}`} onClick={() => zoomAround("fit")} title={`${t("viewer.fit")} (Z)`}>
          {t("viewer.fit")}
        </button>
        <button type="button" className={`btn small${zoom === 1 ? " active" : ""}`} onClick={() => zoomAround(1)} title={`100 % (Z)`}>
          100 %
        </button>
        <button type="button" className="btn small" onClick={() => zoomAround(stepZoom(scale, -1))} title={`${t("viewer.zoomOut")} (−)`}>
          −
        </button>
        <span className="zoom-label">{zoomLabel}</span>
        <button type="button" className="btn small" onClick={() => zoomAround(stepZoom(scale, 1))} title={`${t("viewer.zoomIn")} (+)`}>
          +
        </button>
        <span className="sep" />
        <button type="button" className="btn small" onClick={onFullscreen} title={`${t("viewer.fullscreen")} (F)`}>
          ⛶ {t("viewer.fullscreen")}
        </button>
      </div>
      <div
        className={`viewer-stage${zoom === "fit" ? "" : " zoomed"}`}
        ref={stage}
        onWheel={onWheel}
        onDoubleClick={(e) => toggleZoom(rel(e))}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
      >
        <div className="viewer-image" style={{ width: cssW, height: cssH, transform: `translate(${offset.x}px, ${offset.y}px)` }}>
          {!previewFailed ? (
            <img src={`${imageUrl(token, "preview", photo.id)}?v=${version}`} alt={photo.fileName} draggable={false} onError={() => setPreviewFailed(true)} onLoad={() => perfViewerLoaded(photo.id)} />
          ) : (
            <div className="viewer-error">{t("viewer.loadFailed")}</div>
          )}
          {needFull && (
            <img
              key={`full-${photo.id}`}
              className={`full${fullLoaded ? " loaded" : ""}`}
              src={`${imageUrl(token, "full", photo.id)}?v=${version}`}
              alt=""
              draggable={false}
              onLoad={() => setFullLoaded(true)}
            />
          )}
        </div>
        {needFull && !fullLoaded && <div className="viewer-loading">{t("viewer.loadingFull")}</div>}
      </div>
    </div>
  );
}
