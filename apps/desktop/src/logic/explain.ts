import type { Reason } from "../types";

type T = (key: string, opts?: Record<string, unknown>) => string;

/** Junta itens numa lista natural: “a, b e c” / “a, b and c”. */
export function joinList(items: string[], and: string): string {
  if (items.length <= 1) return items[0] ?? "";
  return `${items.slice(0, -1).join(", ")} ${and} ${items[items.length - 1]}`;
}

export function parseReasons(json: string | null | undefined): Reason[] {
  if (!json) return [];
  try {
    const v = JSON.parse(json) as unknown;
    return Array.isArray(v) ? v.filter((r): r is Reason => !!r && typeof (r as Reason).code === "string") : [];
  } catch {
    return [];
  }
}

/** Valor formatado para as frases (percentagens e graus com 1 casa decimal no máximo). */
function fmt(v: number | undefined, lang: string): string {
  if (v === undefined) return "";
  const r = Math.abs(v) >= 10 ? Math.round(v) : Math.round(v * 10) / 10;
  return r.toLocaleString(lang);
}

export function reasonText(r: Reason, t: T, lang: string): string {
  // “ambos os rostos visíveis” quando são 2.
  if (r.code === "allFacesVisible" && r.value === 2) return t("reason.bothFacesVisible");
  return t(`reason.${r.code}`, { value: fmt(r.value, lang), count: r.value ?? 0 });
}

/**
 * Frase de explicação, ex.: “Melhor fotografia desta sequência: maior nitidez facial,
 * ambos os rostos visíveis e melhor exposição.”
 */
export function explain(summary: string, reasons: Reason[], t: T, lang: string): string {
  const head = t(`summary.${summary}`);
  // O resumo já diz o essencial; a razão com o mesmo código não se repete.
  const parts = reasons.filter((r) => r.code !== summary).map((r) => reasonText(r, t, lang));
  return parts.length === 0 ? `${head}.` : `${head}: ${joinList(parts, t("reason.and"))}.`;
}
