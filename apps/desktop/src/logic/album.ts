import type { AlbumCrop, AlbumSlot, AlbumSpec, AlbumSpread } from "../types";

/** Janela do recorte (u0, v0, u1, v1), igual a `crop_window` no núcleo. */
export function cropWindow(pw: number, ph: number, sw: number, sh: number, c: AlbumCrop): [number, number, number, number] {
  const ap = pw / Math.max(ph, 1e-6);
  const as = sw / Math.max(sh, 1e-6);
  let [ww, wh] = as > ap ? [1, ap / as] : [as / ap, 1];
  const z = Math.min(Math.max(c.zoom, 1), 10);
  ww /= z;
  wh /= z;
  const cx = Math.min(Math.max(c.cx, ww / 2), 1 - ww / 2);
  const cy = Math.min(Math.max(c.cy, wh / 2), 1 - wh / 2);
  return [cx - ww / 2, cy - wh / 2, cx + ww / 2, cy + wh / 2];
}

/** DPI efetivo (igual ao núcleo). */
export function effectiveDpi(pw: number, ph: number, s: AlbumSlot): number {
  const [u0, v0, u1, v1] = cropWindow(pw, ph, s.w, s.h, s.crop);
  return Math.min(((u1 - u0) * pw) / (s.w / 25.4), ((v1 - v0) * ph) / (s.h / 25.4));
}

/** Página (0 esquerda, 1 direita) de um slot, pelo centro. */
export function pageOf(spec: AlbumSpec, s: AlbumSlot): 0 | 1 {
  return s.x + s.w / 2 < spec.widthMm ? 0 : 1;
}

/** A sangrar na página do slot (três lados exteriores; o lado da dobra fica no meio). */
export function bleedSlot(spec: AlbumSpec, s: AlbumSlot): AlbumSlot {
  const b = spec.bleedMm;
  const left = pageOf(spec, s) === 0;
  return { ...s, x: left ? -b : spec.widthMm, y: -b, w: spec.widthMm + b, h: spec.heightMm + 2 * b };
}

/** Panorâmica: o spread inteiro a sangrar. */
export function panoramaSlot(spec: AlbumSpec, s: AlbumSlot): AlbumSlot {
  const b = spec.bleedMm;
  return { ...s, x: -b, y: -b, w: 2 * spec.widthMm + 2 * b, h: spec.heightMm + 2 * b };
}

export type Align = "left" | "hcenter" | "right" | "top" | "vcenter" | "bottom";

/** Alinha os slots escolhidos pela caixa que os contém. */
export function align(slots: AlbumSlot[], ids: Set<string>, how: Align): AlbumSlot[] {
  const sel = slots.filter((s) => ids.has(s.id));
  if (sel.length < 2) return slots;
  const x0 = Math.min(...sel.map((s) => s.x));
  const x1 = Math.max(...sel.map((s) => s.x + s.w));
  const y0 = Math.min(...sel.map((s) => s.y));
  const y1 = Math.max(...sel.map((s) => s.y + s.h));
  return slots.map((s) => {
    if (!ids.has(s.id)) return s;
    switch (how) {
      case "left":
        return { ...s, x: x0 };
      case "right":
        return { ...s, x: x1 - s.w };
      case "hcenter":
        return { ...s, x: (x0 + x1) / 2 - s.w / 2 };
      case "top":
        return { ...s, y: y0 };
      case "bottom":
        return { ...s, y: y1 - s.h };
      case "vcenter":
        return { ...s, y: (y0 + y1) / 2 - s.h / 2 };
    }
  });
}

/** Distribui (espaços iguais entre slots) na horizontal ou na vertical. */
export function distribute(slots: AlbumSlot[], ids: Set<string>, axis: "h" | "v"): AlbumSlot[] {
  const sel = slots.filter((s) => ids.has(s.id));
  if (sel.length < 3) return slots;
  const pos = (s: AlbumSlot) => (axis === "h" ? s.x : s.y);
  const size = (s: AlbumSlot) => (axis === "h" ? s.w : s.h);
  const sorted = [...sel].sort((a, b) => pos(a) - pos(b));
  const start = pos(sorted[0]!);
  const end = Math.max(...sorted.map((s) => pos(s) + size(s)));
  const gap = (end - start - sorted.reduce((t, s) => t + size(s), 0)) / (sorted.length - 1);
  const at = new Map<string, number>();
  let p = start;
  for (const s of sorted) {
    at.set(s.id, p);
    p += size(s) + gap;
  }
  return slots.map((s) => (at.has(s.id) ? (axis === "h" ? { ...s, x: at.get(s.id)! } : { ...s, y: at.get(s.id)! }) : s));
}

/** Mantém o slot dentro da sangria do spread. */
export function clampSlot(spec: AlbumSpec, s: AlbumSlot): AlbumSlot {
  const b = spec.bleedMm;
  const W = 2 * spec.widthMm + 2 * b;
  const H = spec.heightMm + 2 * b;
  const w = Math.min(Math.max(s.w, 10), W);
  const h = Math.min(Math.max(s.h, 10), H);
  return { ...s, w, h, x: Math.min(Math.max(s.x, -b), W - b - w), y: Math.min(Math.max(s.y, -b), H - b - h) };
}

/** Número de fotografias num spread. */
export function photoCount(sp: AlbumSpread): number {
  return sp.slots.filter((s) => s.photo != null).length;
}

/** Desenho esquemático de um layout ("B|G3", "P"…): retângulos em [0,1]² do spread. */
export function templateBoxes(id: string): [number, number, number, number][] {
  if (id === "P") return [[0, 0, 1, 1]];
  const [l, r] = id.split("|");
  const out: [number, number, number, number][] = [];
  [l, r].forEach((f, page) => {
    const x0 = page * 0.5;
    if (!f || f === "-") return;
    if (f === "B") return void out.push([x0, 0, 0.5, 1]);
    const n = f === "F" ? 1 : Number(f.slice(1));
    const cols = n <= 2 ? 1 : n <= 4 ? 2 : 3;
    const rows = Math.ceil(n / cols);
    for (let i = 0; i < n; i++) {
      const c = i % cols;
      const rr = Math.floor(i / cols);
      out.push([x0 + 0.06 + (c * 0.38) / cols, 0.1 + (rr * 0.8) / rows, 0.38 / cols - 0.02, 0.8 / rows - 0.03]);
    }
  });
  return out;
}

/** Centímetros a partir de mm, para mostrar. */
export function cm(mm: number): string {
  return (mm / 10).toLocaleString("pt-PT", { maximumFractionDigits: 1 });
}
