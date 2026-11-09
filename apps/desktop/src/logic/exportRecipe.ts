import type { ExportCaps, ExportColorSpace, ExportProgress, ExportRecipe } from "../types";

/** Receita nova (JPEG sRGB, tamanho original). */
export function newRecipe(name: string): ExportRecipe {
  return {
    id: "",
    name,
    format: "jpeg",
    depth: "8",
    quality: 90,
    compression: "lossy",
    color: "srgb",
    iccPath: "",
    intent: "perceptual",
    size: { mode: "original", unit: "px", a: 2048, b: 2048, noEnlarge: true, ppi: 300, matchOrientation: false },
    sharpen: { target: "none", amount: "standard" },
    metadata: { mode: "all", removeGps: true, copyright: "", creator: "", contact: "" },
    watermark: { enabled: false, kind: "text", text: "© ", imagePath: "", fontPath: "", color: "#ffffff", size: 4, opacity: 60, anchor: "bottomRight", margin: 3 },
    naming: { template: "{nome}", prefix: "", suffix: "", seqStart: 1, seqDigits: 4, subfolder: "", conflict: "rename" },
  };
}

/** Espaços de cor permitidos por um formato. */
export function allowedSpaces(caps: ExportCaps | undefined, all: ExportColorSpace[]): ExportColorSpace[] {
  if (!caps) return all;
  if (caps.spaces === "builtin") return all.filter((s) => s !== "icc");
  if (caps.spaces === "cicp") return all.filter((s) => s === "srgb" || s === "displayP3" || s === "rec709" || s === "rec2020");
  return all;
}

/**
 * Ao mudar de formato, mantém o que for possível e corrige o resto (profundidade,
 * compressão e espaço de cor válidos para o novo formato).
 */
export function fitToFormat(r: ExportRecipe, caps: ExportCaps, spaces: ExportColorSpace[]): ExportRecipe {
  const depth = caps.depths.includes(r.depth) ? r.depth : (caps.depths[0] ?? r.depth);
  let compression = r.compression;
  if (caps.compressions.length === 0) compression = caps.quality ? "lossy" : "none";
  else if (!caps.compressions.includes(compression)) compression = caps.compressions[caps.compressions.length > 1 ? 1 : 0] ?? compression;
  const allowed = allowedSpaces(caps, spaces);
  const color = allowed.includes(r.color) ? r.color : "srgb";
  const jp2Lossless = caps.format === "jpeg2000" && compression === "lossless";
  return { ...r, format: caps.format, depth: jp2Lossless ? "8" : depth, compression, color };
}

/** Valores de exemplo para pré-visualizar o nome dos ficheiros. */
export interface NameVars {
  stem: string;
  seq: number;
  date: string;
  time: string;
  project: string;
  client: string;
  recipe: string;
  moment: string;
  width: number;
  height: number;
}

/** Igual a `naming::sanitize` no motor (carateres proibidos em Windows/macOS). */
export function sanitize(s: string): string {
  // eslint-disable-next-line no-control-regex
  let out = s.replace(/[/\\:*?"<>|\u0000-\u001f]/g, "_").trim().replace(/\.+$/, "").trim();
  const stem = out.toUpperCase().split(".")[0] ?? "";
  if (["CON", "PRN", "AUX", "NUL"].includes(stem) || /^(COM|LPT)\d$/.test(stem)) out = `_${out}`;
  return out;
}

function expand(t: string, n: ExportRecipe["naming"], v: NameVars): string {
  const seq = String(v.seq).padStart(Math.min(Math.max(n.seqDigits, 1), 9), "0");
  return t
    .replaceAll("{nome}", v.stem)
    .replaceAll("{seq}", seq)
    .replaceAll("{data}", v.date)
    .replaceAll("{hora}", v.time)
    .replaceAll("{projeto}", v.project)
    .replaceAll("{cliente}", v.client)
    .replaceAll("{receita}", v.recipe)
    .replaceAll("{momento}", v.moment)
    .replaceAll("{largura}", String(v.width))
    .replaceAll("{altura}", String(v.height));
}

/** Caminho de exemplo (subpasta + nome), como o motor o calcula. */
export function examplePath(r: ExportRecipe, ext: string, v: NameVars): string {
  const n = r.naming;
  const body = sanitize(`${n.prefix}${expand(n.template, n, v)}${n.suffix}`) || "foto";
  const folder = expand(n.subfolder, n, v)
    .split(/[/\\]/)
    .map(sanitize)
    .filter((p) => p && p !== "." && p !== "..");
  return [...folder, `${body}.${ext}`].join("/");
}

/** Duração legível (ex.: "1 h 05 min", "3 min 20 s", "12 s"). */
export function formatDuration(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  if (s >= 3600) return `${Math.floor(s / 3600)} h ${String(Math.floor((s % 3600) / 60)).padStart(2, "0")} min`;
  if (s >= 60) return `${Math.floor(s / 60)} min ${String(s % 60).padStart(2, "0")} s`;
  return `${s} s`;
}

/** Fração concluída de um trabalho (0–1). */
export function jobFraction(p: ExportProgress): number {
  if (p.filesTotal <= 0) return 0;
  return Math.min(1, (p.filesDone + p.filesFailed + p.filesSkipped) / p.filesTotal);
}

export function isActive(p: ExportProgress): boolean {
  return p.state === "queued" || p.state === "running" || p.state === "paused";
}
