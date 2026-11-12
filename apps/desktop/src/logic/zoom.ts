/**
 * Zoom do editor: em vez de revelar a fotografia inteira em grande, revela-se só a zona
 * visível (um sub-recorte) ao tamanho do ecrã — rápido a qualquer ampliação.
 *
 * `zoom` = píxeis do ecrã (físicos) por píxel do original (1 = 100 %); `center` = centro da
 * vista no enquadramento recortado (0–1).
 */

export const ZOOM_STEPS = [0.25, 0.5, 1, 2, 4] as const;
/** Lado maior da fonte de edição (acima disto o render usa a fonte em resolução total). */
export const EDIT_SOURCE_EDGE = 3072;

export interface Crop {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface ZoomRegion {
  /** Sub-recorte a revelar (coordenadas do enquadramento inteiro, como `geometry.crop`). */
  crop: Crop;
  /** Tamanho do render (píxeis físicos). */
  outW: number;
  outH: number;
  /** Centro corrigido (a vista nunca sai da fotografia). */
  center: { x: number; y: number };
  /** Precisa da fonte em resolução total. */
  hiRes: boolean;
}

/**
 * Zona visível para um zoom: `native` = tamanho nativo do recorte (px do original);
 * `viewport` = área do ecrã (px físicos).
 */
export function zoomRegion(crop: Crop, native: { w: number; h: number }, viewport: { w: number; h: number }, zoom: number, center: { x: number; y: number }): ZoomRegion {
  const rw = Math.min(1, viewport.w / (zoom * native.w));
  const rh = Math.min(1, viewport.h / (zoom * native.h));
  const cx = Math.min(1 - rw / 2, Math.max(rw / 2, center.x));
  const cy = Math.min(1 - rh / 2, Math.max(rh / 2, center.y));
  const fullLong = Math.max(native.w / crop.w, native.h / crop.h);
  return {
    crop: { x: crop.x + (cx - rw / 2) * crop.w, y: crop.y + (cy - rh / 2) * crop.h, w: rw * crop.w, h: rh * crop.h },
    outW: Math.max(1, Math.round(rw * zoom * native.w)),
    outH: Math.max(1, Math.round(rh * zoom * native.h)),
    center: { x: cx, y: cy },
    hiRes: zoom * fullLong > EDIT_SOURCE_EDGE * 1.02,
  };
}

/** Zoom que mostra a fotografia inteira na vista. */
export function fitZoom(native: { w: number; h: number }, viewport: { w: number; h: number }): number {
  return Math.min(viewport.w / native.w, viewport.h / native.h);
}

/** Passo seguinte (dir = +1 aproxima, −1 afasta); `null` = ajustar. */
export function stepZoom(current: number | null, fit: number, dir: 1 | -1): number | null {
  const z = current ?? fit;
  if (dir > 0) return ZOOM_STEPS.find((s) => s > z * 1.01) ?? ZOOM_STEPS[ZOOM_STEPS.length - 1]!;
  const lower = [...ZOOM_STEPS].reverse().find((s) => s < z * 0.99);
  return lower === undefined || lower <= fit ? null : lower;
}

/**
 * Centro para manter o ponto (u, v) da vista (0–1) debaixo do cursor ao mudar de zoom.
 * `region` = zona visível antes; `next` = largura/altura (0–1) da zona depois.
 */
export function anchorCenter(before: { cx: number; cy: number; rw: number; rh: number }, u: number, v: number, next: { rw: number; rh: number }): { x: number; y: number } {
  const px = before.cx - before.rw / 2 + u * before.rw;
  const py = before.cy - before.rh / 2 + v * before.rh;
  return { x: px - (u - 0.5) * next.rw, y: py - (v - 0.5) * next.rh };
}
