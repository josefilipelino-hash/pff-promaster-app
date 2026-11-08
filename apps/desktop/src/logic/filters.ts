import type { Color, PhotoRow } from "../types";

export type Collection = "all" | "picks" | "rejects" | "unrated";

export interface Filter {
  collection: Collection;
  /** Mostrar só fotografias com pelo menos N estrelas (0 = sem filtro). */
  minStars: number;
  /** Etiqueta de cor (null = qualquer). */
  color: Color | null;
}

export const DEFAULT_FILTER: Filter = { collection: "all", minStars: 0, color: null };

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

export function matches(p: PhotoRow, f: Filter): boolean {
  return inCollection(p, f.collection) && p.stars >= f.minStars && (f.color === null || p.color === f.color);
}

export function applyFilter(photos: PhotoRow[], f: Filter): PhotoRow[] {
  if (f.collection === "all" && f.minStars === 0 && f.color === null) return photos;
  return photos.filter((p) => matches(p, f));
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
