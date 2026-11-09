import type { AutoOptions, DevelopSettings } from "../types";

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
    geometry: { crop: { x: 0, y: 0, w: 1, h: 1 }, quarterTurns: 0, flipHorizontal: false, straighten: 0, aspect: "original", vertical: 0, horizontal: 0 },
    lens: { distortion: 0, caRed: 0, caBlue: 0, vignetting: 0, vignettingMidpoint: 50 },
    retouch: { redEyes: [], spots: [] },
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

/** Proporções do recorte (largura/altura); "original" = proporção da fotografia, "free" = livre. */
export const ASPECTS: [string, number | null][] = [
  ["original", null],
  ["free", null],
  ["1:1", 1],
  ["4:5", 4 / 5],
  ["5:4", 5 / 4],
  ["2:3", 2 / 3],
  ["3:2", 3 / 2],
  ["16:9", 16 / 9],
  ["9:16", 9 / 16],
];

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
export const DEFAULT_COPY_BLOCKS = ["light", "wb", "color", "hsl", "curve", "detail", "effects", "lens"];

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
