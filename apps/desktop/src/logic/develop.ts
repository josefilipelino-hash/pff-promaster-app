import type { AutoOptions, DevelopSettings, LensProfile, LocalAdjust, LocalMask, MatchOptions, Preset } from "../types";

/** Ajustes neutros (iguais aos do núcleo: uma receita vazia reproduz o original). */
export function defaultSettings(): DevelopSettings {
  return {
    light: { exposure: 0, contrast: 0, highlights: 0, shadows: 0, whites: 0, blacks: 0 },
    wb: { temperature: 0, tint: 0 },
    color: { saturation: 0, vibrance: 0 },
    hsl: { hue: Array(8).fill(0), saturation: Array(8).fill(0), luminance: Array(8).fill(0) },
    curve: { highlights: 0, lights: 0, darks: 0, shadows: 0, splitShadows: 25, splitMidtones: 50, splitHighlights: 75, rgb: [], red: [], green: [], blue: [] },
    detail: { sharpenAmount: 0, sharpenRadius: 1, sharpenDetail: 25, sharpenMasking: 0, noiseLuminance: 0, noiseColor: 0, noiseDetail: 50, aiDenoise: 0 },
    effects: { texture: 0, clarity: 0, dehaze: 0, vignette: 0, vignetteMidpoint: 50, vignetteFeather: 50, vignetteRoundness: 0, grain: 0, grainSize: 25, grainRoughness: 50 },
    geometry: { crop: { x: 0, y: 0, w: 1, h: 1 }, quarterTurns: 0, flipHorizontal: false, straighten: 0, aspect: "original", vertical: 0, horizontal: 0, aspectAdjust: 0, scale: 100, offsetX: 0, offsetY: 0, focal35: 0, upright: "", guides: [] },
    lens: { distortion: 0, caRed: 0, caBlue: 0, vignetting: 0, vignettingMidpoint: 50, profile: defaultLensProfile() },
    retouch: { redEyes: [], spots: [], patches: [] },
    grading: { shadowsHue: 220, shadowsSat: 0, highlightsHue: 45, highlightsSat: 0, balance: 0 },
    bw: { enabled: false, mix: Array(8).fill(0) },
    lut: { id: "", name: "", amount: 100 },
    masks: [],
    portrait: { skin: 0, darkCircles: 0, shine: 0, teeth: 0, eyes: 0, faces: [] },
  };
}

export function cloneSettings(s: DevelopSettings): DevelopSettings {
  return JSON.parse(JSON.stringify(s)) as DevelopSettings;
}

export function sameSettings(a: DevelopSettings, b: DevelopSettings): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** Frame binário do núcleo: u32 largura, u32 altura, histograma 3×256 u32, RGBA. */
export interface Frame {
  width: number;
  height: number;
  histogram: [Uint32Array, Uint32Array, Uint32Array];
  rgba: Uint8ClampedArray;
}

export function parseFrame(buf: ArrayBuffer): Frame {
  const head = new DataView(buf);
  const width = head.getUint32(0, true);
  const height = head.getUint32(4, true);
  const hist = new Uint32Array(buf.slice(8, 8 + 3 * 256 * 4));
  const rgba = new Uint8ClampedArray(buf, 8 + 3 * 256 * 4, width * height * 4);
  return { width, height, histogram: [hist.slice(0, 256), hist.slice(256, 512), hist.slice(512, 768)], rgba };
}

/**
 * Formatos de recorte do mercado (largura:altura). A orientação troca-se com ⇄ ("4:5" ↔
 * "5:4"); "original" = proporção da fotografia, "free" = livre, qualquer "L:A" é aceite.
 */
export const ASPECTS: [string, number | null][] = [
  ["original", null],
  ["free", null],
  ["1:1", 1],
  ["4:5", 4 / 5],
  ["2:3", 2 / 3],
  ["3:4", 3 / 4],
  ["5:7", 5 / 7],
  ["1:1.414", 1 / 1.414],
  ["8.5:11", 8.5 / 11],
  ["9:16", 9 / 16],
  ["16:9", 16 / 9],
  ["16:10", 16 / 10],
  ["1.91:1", 1.91],
  ["21:9", 21 / 9],
  ["3:1", 3],
  ["65:24", 65 / 24],
];

/** Proporção (largura/altura) de um formato "L:A"; `null` para "free"/"original"/inválido. */
export function parseAspect(id: string): number | null {
  const m = /^\s*([\d.,]+)\s*:\s*([\d.,]+)\s*$/.exec(id);
  if (!m) return null;
  const a = Number(m[1]!.replace(",", "."));
  const b = Number(m[2]!.replace(",", "."));
  return a > 0 && b > 0 && Number.isFinite(a / b) ? a / b : null;
}

/** "4:5" → "5:4" (os outros ficam iguais). */
export function swapAspect(id: string): string {
  const m = /^\s*([\d.,]+)\s*:\s*([\d.,]+)\s*$/.exec(id);
  return m ? `${m[2]}:${m[1]}` : id;
}

/** Entrada da lista correspondente a um formato (na orientação da lista ou trocada), ou "custom". */
export function aspectEntry(id: string): string {
  if (ASPECTS.some(([a]) => a === id)) return id;
  const sw = swapAspect(id);
  return ASPECTS.some(([a]) => a === sw) ? sw : "custom";
}

/** Formato na orientação da fotografia (horizontal numa fotografia horizontal e vice-versa). */
export function orientAspect(id: string, frameW: number, frameH: number): string {
  const r = parseAspect(id);
  if (r === null || r === 1) return id;
  return r > 1 !== frameW >= frameH ? swapAspect(id) : id;
}

/**
 * Ajusta um recorte (normalizado) à proporção `ratio` (largura/altura, em píxeis) dentro de
 * uma imagem `imgW`×`imgH`, mantendo o centro e o maior tamanho possível.
 */
export function fitCropToAspect(crop: { x: number; y: number; w: number; h: number }, ratio: number, imgW: number, imgH: number) {
  const cx = crop.x + crop.w / 2;
  const cy = crop.y + crop.h / 2;
  // Em píxeis: w·imgW / (h·imgH) = ratio.
  let w = crop.w;
  let h = (w * imgW) / (ratio * imgH);
  if (h > crop.h) {
    h = crop.h;
    w = (h * ratio * imgH) / imgW;
  }
  w = Math.min(w, 1);
  h = Math.min(h, 1);
  const x = Math.min(Math.max(cx - w / 2, 0), 1 - w);
  const y = Math.min(Math.max(cy - h / 2, 0), 1 - h);
  return { x, y, w, h };
}

/** Blocos copiados por omissão (a geometria/recorte raramente se quer em todas as fotografias). */
export const DEFAULT_COPY_BLOCKS = ["light", "wb", "color", "hsl", "curve", "detail", "effects", "lens", "grading", "bw", "lut"];

/** Opções por omissão da edição automática (iguais às do núcleo). */
export function defaultAutoOptions(): AutoOptions {
  return {
    intensity: "natural",
    strength: 100,
    tone: true,
    whiteBalance: true,
    color: true,
    sharpen: true,
    denoise: true,
    aiDenoise: "off",
    straighten: true,
    crop: false,
    lens: true,
    redEye: true,
    dust: false,
  };
}

export function defaultMatchOptions(): MatchOptions {
  return { exposure: true, whiteBalance: true, skin: true, contrast: true, color: true, look: true };
}

/** Presets agrupados (internos primeiro, pela ordem do núcleo). */
export function groupPresets(list: Preset[], noGroup: string): [string, Preset[]][] {
  const m = new Map<string, Preset[]>();
  for (const p of list) {
    const g = p.group || noGroup;
    if (!m.has(g)) m.set(g, []);
    m.get(g)!.push(p);
  }
  return [...m.entries()];
}

export function emptyAdjust(): LocalAdjust {
  return { exposure: 0, contrast: 0, highlights: 0, shadows: 0, whites: 0, blacks: 0, temperature: 0, tint: 0, saturation: 0, texture: 0, clarity: 0, sharpness: 0, noise: 0 };
}

let maskSeq = 0;
export function newMask(name: string): LocalMask {
  maskSeq += 1;
  return { id: `m${Date.now().toString(36)}${maskSeq}`, name, enabled: true, invert: false, density: 100, components: [], adjust: emptyAdjust() };
}

/** Distância entre dois pontos normalizados, em unidades do lado maior (ru, rv = largura/altura ÷ lado maior). */
export function longDist(a: [number, number], b: [number, number], ru: number, rv: number): number {
  return Math.hypot((a[0] - b[0]) * ru, (a[1] - b[1]) * rv);
}

/** Escalas de u e v para unidades do lado maior. */
export function unitScale(width: number, height: number): [number, number] {
  const l = Math.max(width, height, 1);
  return [width / l, height / l];
}

export function defaultLensProfile(): LensProfile {
  return { enabled: false, name: "", distortion: 100, vignetting: 100, chromatic: true, distModel: 0, dist: [0, 0, 0], distScale: 1, tcaModel: 0, tca: [1, 1, 0, 0, 0, 0], vig: [0, 0, 0], vigScale: 1 };
}
