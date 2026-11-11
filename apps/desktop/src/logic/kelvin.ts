/**
 * Temperatura em Kelvin para RAW. A receita guarda a temperatura relativa ao balanço da
 * câmara (t, em centésimos de oitava: K = Kdisparo · 2^(t/100)), para que um preset leve a
 * mesma correção a fotografias com balanços diferentes; a interface mostra e lê Kelvin.
 */
export const KELVIN_MIN = 2000;
export const KELVIN_MAX = 50000;

export function kelvinFromRelative(t: number, asShot: number): number {
  return asShot * Math.pow(2, t / 100);
}

export function relativeFromKelvin(k: number, asShot: number): number {
  return 100 * Math.log2(k / asShot);
}

/** Valor mostrado: arredondado a 50 K (como no Lightroom). */
export function formatKelvin(t: number, asShot: number): string {
  return String(Math.round(kelvinFromRelative(t, asShot) / 50) * 50);
}

/** Lê "5600", "5 600", "5600K" ou "5600 k". */
export function parseKelvin(text: string, asShot: number): number | null {
  const k = Number(text.replace(/\s|k/gi, "").replace(",", "."));
  if (!Number.isFinite(k) || k <= 0) return null;
  return relativeFromKelvin(Math.min(KELVIN_MAX, Math.max(KELVIN_MIN, k)), asShot);
}

/** Limites do controlo (em t) para 2000–50000 K. */
export function relativeRange(asShot: number): [number, number] {
  return [relativeFromKelvin(KELVIN_MIN, asShot), relativeFromKelvin(KELVIN_MAX, asShot)];
}
