import { api } from "../api";

/**
 * Modo de diagnóstico de desempenho (só com `MOMENTO_PERF=1` no ambiente da aplicação).
 * Mede no WebView real: FPS e frames longos durante o scroll, tempo até a interface e a
 * grelha estarem prontas. Os valores vão para o log da aplicação (sem dados pessoais).
 */
let enabled = false;

export function perfMark(event: string, data: Record<string, unknown> = {}): void {
  if (enabled) api.perfLog(event, data).catch(() => {});
}

/** Estatísticas de uma janela de frames (exportada para os testes). */
export function frameStats(intervals: number[]): { fps: number; p95: number; long: number; frames: number } {
  if (intervals.length === 0) return { fps: 0, p95: 0, long: 0, frames: 0 };
  const total = intervals.reduce((a, b) => a + b, 0);
  const sorted = [...intervals].sort((a, b) => a - b);
  return {
    fps: Math.round((intervals.length * 1000) / total),
    p95: Math.round(sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))]! * 10) / 10,
    long: intervals.filter((d) => d > 50).length,
    frames: intervals.length,
  };
}

export async function startPerfProbe(): Promise<void> {
  try {
    enabled = await api.perfMode();
  } catch {
    enabled = false;
  }
  if (!enabled) return;
  requestAnimationFrame(() => perfMark("interface_pronta"));

  // FPS só enquanto há scroll (os frames ociosos não contam).
  let lastScroll = 0;
  document.addEventListener("scroll", () => (lastScroll = performance.now()), { capture: true, passive: true });
  let prev = performance.now();
  let window: number[] = [];
  let windowStart = prev;
  const loop = (now: number) => {
    if (now - lastScroll < 150) window.push(now - prev);
    prev = now;
    if (now - windowStart > 3000) {
      if (window.length > 10) perfMark("scroll", { ...frameStats(window), celulas: document.querySelectorAll(".cell").length });
      window = [];
      windowStart = now;
    }
    requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
}

/** Depois de abrir um projeto: espera até a grelha ter as miniaturas visíveis carregadas. */
export function perfWatchGrid(t0: number, photos: number): void {
  if (!enabled) return;
  const check = () => {
    const imgs = Array.from(document.querySelectorAll<HTMLImageElement>(".cell img"));
    if (imgs.length > 0 && imgs.every((i) => i.complete)) {
      perfMark("grelha_pronta", { ms: Math.round(performance.now() - t0), fotografias: photos, miniaturas: imgs.length });
    } else if (performance.now() - t0 < 60000) setTimeout(check, 50);
  };
  check();
}

// Visualizador: tempo entre mostrar uma fotografia e a preview estar no ecrã.
const shownAt = new Map<number, number>();
export function perfViewerShown(id: number): void {
  if (enabled) shownAt.set(id, performance.now());
}
export function perfViewerLoaded(id: number): void {
  const t = shownAt.get(id);
  if (!enabled || t === undefined) return;
  shownAt.delete(id);
  perfMark("visualizador_preview", { ms: Math.round(performance.now() - t) });
}
