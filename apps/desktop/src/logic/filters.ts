import type { Color, Label, PhotoRow, RecommendationBrief } from "../types";

export type Collection = "all" | "picks" | "rejects" | "unrated" | "unavailable";

export interface Filter {
  collection: Collection;
  /** Mostrar só fotografias com pelo menos N estrelas (0 = sem filtro). */
  minStars: number;
  /** Etiqueta de cor (null = qualquer). */
  color: Color | null;
    /** Etiqueta da seleção assistida (null = qualquer). */
  label?: Label | null;
  /** Pessoas que têm de aparecer **todas** na fotografia (ex.: noiva + mãe). */
  persons?: number[];
  /** Momento (id), “unclassified” = Por classificar, null = qualquer. */
  moment?: number | "unclassified" | null;
}

/** Dados de organização usados pelos filtros. */
export interface OrgMaps {
  photoPersons?: Map<number, number[]>;
  /** Momento efetivo de cada fotografia (null = por classificar). */
  photoMoment?: Map<number, number | null>;
}

export const DEFAULT_FILTER: Filter = { collection: "all", minStars: 0, color: null, label: null };

/** “Sem classificação” = sem estrelas e sem marca de favorita/rejeitada. */
export function inCollection(p: PhotoRow, c: Collection): boolean {
  switch (c) {
    case "all":
      return true;
    case "picks":
      return p.flag === 1;
    case "rejects":
      return p.flag === -1;
    case "unrated":
      return p.stars === 0 && p.flag === 0;
    case "unavailable":
      return (p.availability ?? "ok") !== "ok";
  }
}

export type RecMap = Map<number, RecommendationBrief>;

export function matches(p: PhotoRow, f: Filter, recs?: RecMap, org?: OrgMaps): boolean {
  if (!inCollection(p, f.collection) || p.stars < f.minStars || (f.color !== null && p.color !== f.color)) return false;
  if (f.label && recs?.get(p.asset ?? p.id)?.label !== f.label) return false;
  if (f.persons && f.persons.length > 0) {
    const here = org?.photoPersons?.get(p.asset ?? p.id) ?? [];
    if (!f.persons.every((x) => here.includes(x))) return false;
  }
  if (f.moment != null) {
    const m = org?.photoMoment?.get(p.asset ?? p.id) ?? null;
    if (f.moment === "unclassified" ? m !== null : m !== f.moment) return false;
  }
  return true;
}

export function applyFilter(photos: PhotoRow[], f: Filter, recs?: RecMap, org?: OrgMaps): PhotoRow[] {
  if (f.collection === "all" && f.minStars === 0 && f.color === null && !f.label && !(f.persons && f.persons.length) && f.moment == null) return photos;
  return photos.filter((p) => matches(p, f, recs, org));
}

/** Contagens por momento (inclui “unclassified”). */
export function momentCounts(photos: PhotoRow[], photoMoment: Map<number, number | null>): Map<number | "unclassified", number> {
  const c = new Map<number | "unclassified", number>();
  for (const p of photos) {
    const m = photoMoment.get(p.asset ?? p.id) ?? null;
    const k = m === null ? "unclassified" : m;
    c.set(k, (c.get(k) ?? 0) + 1);
  }
  return c;
}

export function labelCounts(photos: PhotoRow[], recs: RecMap): Record<Label, number> {
  const c: Record<Label, number> = { best: 0, alternative: 0, review: 0, reject: 0, duplicate: 0 };
  for (const p of photos) {
    const r = recs.get(p.asset ?? p.id);
    if (r) c[r.label]++;
  }
  return c;
}

export function collectionCounts(photos: PhotoRow[]): Record<Collection, number> {
  const c = { all: photos.length, picks: 0, rejects: 0, unrated: 0, unavailable: 0 };
  for (const p of photos) {
    if ((p.availability ?? "ok") !== "ok") c.unavailable++;
    if (p.flag === 1) c.picks++;
    else if (p.flag === -1) c.rejects++;
    if (p.stars === 0 && p.flag === 0) c.unrated++;
  }
  return c;
}
