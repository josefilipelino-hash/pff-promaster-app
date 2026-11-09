import type { Color, Label, PhotoRow, RecommendationBrief } from "../types";

export type Collection = "all" | "picks" | "rejects" | "unrated";

export interface Filter {
  collection: Collection;
  /** Mostrar só fotografias com pelo menos N estrelas (0 = sem filtro). */
  minStars: number;
  /** Etiqueta de cor (null = qualquer). */
  color: Color | null;
  /** Etiqueta da seleção assistida (null = qualquer). */
  label?: Label | null;
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
  }
}

export type RecMap = Map<number, RecommendationBrief>;

export function matches(p: PhotoRow, f: Filter, recs?: RecMap): boolean {
  return (
    inCollection(p, f.collection) &&
    p.stars >= f.minStars &&
    (f.color === null || p.color === f.color) &&
    (!f.label || recs?.get(p.id)?.label === f.label)
  );
}

export function applyFilter(photos: PhotoRow[], f: Filter, recs?: RecMap): PhotoRow[] {
  if (f.collection === "all" && f.minStars === 0 && f.color === null && !f.label) return photos;
  return photos.filter((p) => matches(p, f, recs));
}

export function labelCounts(photos: PhotoRow[], recs: RecMap): Record<Label, number> {
  const c: Record<Label, number> = { best: 0, alternative: 0, review: 0, reject: 0, duplicate: 0 };
  for (const p of photos) {
    const r = recs.get(p.id);
    if (r) c[r.label]++;
  }
  return c;
}

export function collectionCounts(photos: PhotoRow[]): Record<Collection, number> {
  const c = { all: photos.length, picks: 0, rejects: 0, unrated: 0 };
  for (const p of photos) {
    if (p.flag === 1) c.picks++;
    else if (p.flag === -1) c.rejects++;
    if (p.stars === 0 && p.flag === 0) c.unrated++;
  }
  return c;
}
