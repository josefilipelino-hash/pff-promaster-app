import { describe, expect, it } from "vitest";
import { imageUrl, isMac } from "../api";
import type { PhotoRow } from "../types";
import { applyFilter, collectionCounts, DEFAULT_FILTER } from "./filters";
import { cameraName, formatAperture, formatBytes, formatCaptureTime, formatEventDate, formatExposure, formatFocal } from "./format";
import { clickSelect, EMPTY_SELECTION, moveFocus, restrict, selectAll, targets } from "./selection";
import { keyToAction } from "./shortcuts";

const photo = (id: number, over: Partial<PhotoRow> = {}): PhotoRow => ({
  id,
  asset: id,
  variant: id,
  virtualCopy: null,
  edited: false,
  fileName: `IMG_${id}.jpg`,
  captureTime: null,
  width: 600,
  height: 400,
  stars: 0,
  flag: 0,
  color: null,
  thumb: "ready",
  partial: false,
  format: "jpeg",
  ...over,
});

describe("filtros", () => {
  const rows = [photo(1, { flag: 1, stars: 3 }), photo(2, { flag: -1 }), photo(3), photo(4, { stars: 5, color: "red" }), photo(5, { color: "red" })];
  it("coleções da barra lateral", () => {
    expect(applyFilter(rows, { ...DEFAULT_FILTER, collection: "picks" }).map((p) => p.id)).toEqual([1]);
    expect(applyFilter(rows, { ...DEFAULT_FILTER, collection: "rejects" }).map((p) => p.id)).toEqual([2]);
    expect(applyFilter(rows, { ...DEFAULT_FILTER, collection: "unrated" }).map((p) => p.id)).toEqual([3, 5]);
    expect(collectionCounts(rows)).toEqual({ all: 5, picks: 1, rejects: 1, unrated: 2, unavailable: 0 });
  });
  it("indisponíveis (offline/em falta) — Fase 13", () => {
    const r = [...rows.slice(0, 3), { ...rows[3]!, availability: "offline" as const }, { ...rows[4]!, availability: "missing" as const }];
    expect(collectionCounts(r).unavailable).toBe(2);
    expect(applyFilter(r, { collection: "unavailable", minStars: 0, color: null }).map((p) => p.id)).toEqual([rows[3]!.id, rows[4]!.id]);
  });
  it("estrelas mínimas e cor combinam-se", () => {
    expect(applyFilter(rows, { collection: "all", minStars: 3, color: null }).map((p) => p.id)).toEqual([1, 4]);
    expect(applyFilter(rows, { collection: "all", minStars: 0, color: "red" }).map((p) => p.id)).toEqual([4, 5]);
    expect(applyFilter(rows, { collection: "unrated", minStars: 0, color: "red" }).map((p) => p.id)).toEqual([5]);
  });
  it("sem filtro devolve a mesma lista (sem cópia)", () => {
    expect(applyFilter(rows, DEFAULT_FILTER)).toBe(rows);
  });
});

describe("seleção", () => {
  const order = [10, 20, 30, 40, 50];
  it("clique simples, Cmd/Ctrl e Shift", () => {
    let s = clickSelect(EMPTY_SELECTION, order, 20, { toggle: false, range: false });
    expect([...s.ids]).toEqual([20]);
    s = clickSelect(s, order, 40, { toggle: false, range: true });
    expect([...s.ids]).toEqual([20, 30, 40]);
    s = clickSelect(s, order, 30, { toggle: true, range: false });
    expect([...s.ids].sort()).toEqual([20, 40]);
    expect(s.focus).toBe(30);
    expect(targets(s)).toEqual([30]); // foco fora da seleção → só o foco
  });
  it("setas movem o foco e Shift estende", () => {
    let s = moveFocus(EMPTY_SELECTION, order, 1, false);
    expect(s.focus).toBe(10);
    s = moveFocus(s, order, 3, false);
    expect(s.focus).toBe(40);
    s = moveFocus(s, order, 10, false);
    expect(s.focus).toBe(50); // limitado ao fim
    s = moveFocus(s, order, -2, true);
    expect([...s.ids]).toEqual([30, 40, 50]);
    expect(targets(s).sort()).toEqual([30, 40, 50]);
  });
  it("selecionar tudo e restringir ao filtro", () => {
    const s = selectAll(order, 30);
    expect(s.ids.size).toBe(5);
    const r = restrict(s, new Set([10, 20]));
    expect([...r.ids]).toEqual([10, 20]);
    expect(r.focus).toBeNull();
  });
});

describe("atalhos", () => {
  const k = (key: string, o: Partial<{ shiftKey: boolean; metaKey: boolean; ctrlKey: boolean; altKey: boolean }> = {}) => ({ key, shiftKey: false, metaKey: false, ctrlKey: false, altKey: false, ...o });
  it("classificação", () => {
    expect(keyToAction(k("3"), true)).toEqual({ type: "stars", value: 3 });
    expect(keyToAction(k("0"), true)).toEqual({ type: "stars", value: 0 });
    expect(keyToAction(k("p"), true)).toEqual({ type: "flag", value: 1 });
    expect(keyToAction(k("X"), true)).toEqual({ type: "flag", value: -1 });
    expect(keyToAction(k("u"), true)).toEqual({ type: "flag", value: 0 });
    expect(keyToAction(k("6"), true)).toEqual({ type: "color", value: "red" });
    expect(keyToAction(k("9"), true)).toEqual({ type: "color", value: "blue" });
  });
  it("Cmd no macOS, Ctrl no Windows", () => {
    expect(keyToAction(k("a", { metaKey: true }), true)).toEqual({ type: "selectAll" });
    expect(keyToAction(k("a", { ctrlKey: true }), false)).toEqual({ type: "selectAll" });
    expect(keyToAction(k("a", { ctrlKey: true }), true)).toBeNull();
    expect(keyToAction(k("I", { ctrlKey: true, shiftKey: true }), false)).toEqual({ type: "import" });
    expect(keyToAction(k("C", { ctrlKey: true, shiftKey: true }), false)).toEqual({ type: "copyEdit" });
    expect(keyToAction(k("v", { metaKey: true, shiftKey: true }), true)).toEqual({ type: "pasteEdit" });
    // Atalhos do sistema (ex.: Cmd+3) não classificam.
    expect(keyToAction(k("3", { metaKey: true }), true)).toBeNull();
  });
  it("navegação e visualizador", () => {
    expect(keyToAction(k("ArrowRight", { shiftKey: true }), true)).toEqual({ type: "move", dx: 1, dy: 0, extend: true });
    expect(keyToAction(k("Enter"), true)).toEqual({ type: "open" });
    expect(keyToAction(k("Escape"), true)).toEqual({ type: "close" });
    expect(keyToAction(k(" "), true)).toEqual({ type: "toggleZoom" });
    expect(keyToAction(k("f"), true)).toEqual({ type: "fullscreen" });
    expect(keyToAction(k("?", { shiftKey: true }), true)).toEqual({ type: "help" });
  });
});

describe("formatação pt-PT", () => {
  it("valores de exposição", () => {
    expect(formatExposure(1 / 250)).toBe("1/250 s");
    expect(formatExposure(0.5)).toBe("0,5 s");
    expect(formatExposure(2)).toBe("2 s");
    expect(formatAperture(2.8)).toBe("f/2,8");
    expect(formatAperture(8)).toBe("f/8");
    expect(formatFocal(50, 75)).toBe("50 mm (75 mm eq.)");
    expect(formatFocal(85, 85)).toBe("85 mm");
    expect(formatExposure(null)).toBeNull();
  });
  it("datas e tamanhos", () => {
    expect(formatCaptureTime("2026-06-15T14:03:22.12")).toBe("15/06/2026, 14:03:22");
    expect(formatEventDate("2026-06-15")).toBe("15 de junho de 2026");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5 MB");
    expect(formatBytes(1536)).toBe("1,5 KB");
  });
  it("nome da câmara sem repetir a marca", () => {
    expect(cameraName("Canon", "Canon EOS R5")).toBe("Canon EOS R5");
    expect(cameraName("Canon", "EOS R5")).toBe("Canon EOS R5");
    expect(cameraName("NIKON CORPORATION", "NIKON Z 6_2")).toBe("NIKON Z 6_2");
    expect(cameraName("SONY", "ILCE-7M4")).toBe("SONY ILCE-7M4");
    expect(cameraName(null, null)).toBeNull();
  });
});

describe("plataformas", () => {
  it("URL de imagens no Windows e no macOS", () => {
    expect(imageUrl(3, "thumb", 42, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Edg/130")).toBe("http://momento.localhost/3/thumb/42");
    expect(imageUrl(3, "full", 7, "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit")).toBe("momento://localhost/3/full/7");
  });
  it("deteção do macOS", () => {
    expect(isMac("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)")).toBe(true);
    expect(isMac("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")).toBe(false);
  });
});

import { isoToPt, maskPtDate, parsePtDate } from "./date";

describe("datas DD/MM/AAAA", () => {
  it("converte e valida", () => {
    expect(parsePtDate("15/06/2026")).toBe("2026-06-15");
    expect(parsePtDate("5/6/2026")).toBe("2026-06-05");
    expect(parsePtDate("29/02/2024")).toBe("2024-02-29");
    expect(parsePtDate("29/02/2025")).toBeNull();
    expect(parsePtDate("31/04/2026")).toBeNull();
    expect(parsePtDate("15/06/0002")).toBeNull();
    expect(parsePtDate("")).toBe("");
    expect(isoToPt("2026-06-15")).toBe("15/06/2026");
  });
  it("insere barras ao escrever", () => {
    expect(maskPtDate("15")).toBe("15");
    expect(maskPtDate("1506")).toBe("15/06");
    expect(maskPtDate("15062026")).toBe("15/06/2026");
    expect(maskPtDate("150620269")).toBe("15/06/2026");
  });
});

describe("álbum: geometria do editor (Fase 11)", async () => {
  const { align, bleedSlot, clampSlot, cropWindow, distribute, effectiveDpi, panoramaSlot, templateBoxes } = await import("./album");
  const spec = { format: "30x30", widthMm: 300, heightMm: 300, pages: 40, dpi: 300, bleedMm: 3, safeMm: 10, gutterMm: 16, color: "srgb", iccPath: "", intent: "perceptual", lab: "", confirmed: true, exportUnit: "spreads", minPages: 0, maxPages: 0, background: "#fff" } as const;
  const crop = { zoom: 1, cx: 0.5, cy: 0.5 };
  const slot = (id: string, x: number, y: number, w = 50, h = 40) => ({ id, x, y, w, h, photo: 1, crop });

  it("recorte e DPI efetivo iguais ao núcleo", () => {
    // 3:2 numa moldura quadrada: janela 2/3 da largura, centrada.
    const [u0, v0, u1, v1] = cropWindow(6000, 4000, 100, 100, crop);
    expect(u1 - u0).toBeCloseTo(2 / 3);
    expect([v0, v1]).toEqual([0, 1]);
    // O centro não deixa a janela sair da imagem; o zoom reduz a janela.
    expect(cropWindow(6000, 4000, 100, 100, { zoom: 1, cx: 0, cy: 0.5 })[0]).toBe(0);
    expect(cropWindow(6000, 4000, 100, 100, { zoom: 2, cx: 0.5, cy: 0.5 })[3]).toBeCloseTo(0.75);
    // 4000 px em 254 mm (10") = 400 DPI; com zoom 2 → 200.
    expect(effectiveDpi(6000, 4000, { ...slot("a", 0, 0, 254, 254) })).toBeCloseTo(400);
    expect(effectiveDpi(6000, 4000, { ...slot("a", 0, 0, 254, 254), crop: { zoom: 2, cx: 0.5, cy: 0.5 } })).toBeCloseTo(200);
  });

  it("full bleed na página certa, panorâmica e limites", () => {
    expect(bleedSlot(spec, slot("a", 20, 20))).toMatchObject({ x: -3, y: -3, w: 303, h: 306 });
    expect(bleedSlot(spec, slot("a", 400, 20))).toMatchObject({ x: 300, y: -3, w: 303, h: 306 });
    expect(panoramaSlot(spec, slot("a", 400, 20))).toMatchObject({ x: -3, y: -3, w: 606, h: 306 });
    expect(clampSlot(spec, slot("a", 700, -50, 5, 5))).toMatchObject({ x: 603 - 10, y: -3, w: 10, h: 10 });
  });

  it("alinhar e distribuir", () => {
    const ss = [slot("a", 10, 10), slot("b", 100, 30, 50, 80), slot("c", 400, 20, 20, 20), slot("z", 0, 0)];
    const ids = new Set(["a", "b", "c"]);
    expect(align(ss, ids, "top").filter((s) => ids.has(s.id)).map((s) => s.y)).toEqual([10, 10, 10]);
    expect(align(ss, ids, "right").map((s) => s.x + s.w)).toEqual([420, 420, 420, 50]);
    const d = distribute(ss, ids, "h");
    // Espaços iguais entre a, b e c (o primeiro e o último ficam).
    const gap1 = d[1]!.x - (d[0]!.x + d[0]!.w);
    const gap2 = d[2]!.x - (d[1]!.x + d[1]!.w);
    expect(gap1).toBeCloseTo(gap2);
    expect(d[0]!.x).toBe(10);
    expect(d[2]!.x).toBe(400);
    expect(d[3]).toBe(ss[3]);
  });

  it("miniaturas dos layouts", () => {
    expect(templateBoxes("P")).toEqual([[0, 0, 1, 1]]);
    expect(templateBoxes("B|-")).toEqual([[0, 0, 0.5, 1]]);
    expect(templateBoxes("G4|G6")).toHaveLength(10);
    expect(templateBoxes("-|F")[0]![0]).toBeGreaterThan(0.5);
  });
});

describe("diagnóstico de desempenho (Fase 12)", async () => {
  const { frameStats } = await import("./perfProbe");
  it("FPS, p95 e frames longos de uma janela de frames", () => {
    expect(frameStats([])).toEqual({ fps: 0, p95: 0, long: 0, frames: 0 });
    const s = frameStats([...Array(19).fill(16), 80]);
    expect(s.frames).toBe(20);
    expect(s.long).toBe(1);
    expect(s.fps).toBe(Math.round((20 * 1000) / (19 * 16 + 80)));
    expect(s.p95).toBe(80);
  });
});

describe("teclado e atalhos (Fase 14)", async () => {
  const { focusRegion, installKeyboardNav } = await import("./keyboardNav");
  const { shortcutLabel } = await import("./keys");
  it("F6 percorre as regiões e as setas mudam de separador", () => {
    document.body.innerHTML = `
      <div class="toolbar"><button id="t1">a</button></div>
      <nav class="sidebar"><button id="s1">b</button><button id="s2" class="active">c</button></nav>
      <div class="main"><div class="grid-scroller" tabindex="-1"><div class="cell" tabindex="0" id="c1"></div></div></div>
      <aside class="info-panel"><div role="tablist"><button role="tab" id="a" aria-selected="true">A</button><button role="tab" id="b">B</button></div></aside>`;
    (document.getElementById("t1") as HTMLElement).focus();
    focusRegion(1);
    expect(document.activeElement?.id).toBe("s2");
    focusRegion(1);
    expect(document.activeElement?.closest(".grid-scroller")).toBeTruthy();
    focusRegion(1);
    expect(document.activeElement?.id).toBe("a");
    focusRegion(1);
    expect(document.activeElement?.id).toBe("t1");
    focusRegion(-1);
    expect(document.activeElement?.id).toBe("a");
    installKeyboardNav();
    let clicked = "";
    document.getElementById("b")!.addEventListener("click", () => (clicked = "b"));
    document.getElementById("a")!.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    expect(document.activeElement?.id).toBe("b");
    expect(clicked).toBe("b");
    document.body.innerHTML = "";
  });
  it("atalhos mostrados conforme o sistema", () => {
    expect(shortcutLabel("Mod+Shift+E", true)).toBe("⇧⌘E");
    expect(shortcutLabel("Mod+Shift+E", false)).toBe("Ctrl+Shift+E");
    expect(shortcutLabel("D", true)).toBe("D");
  });
});

describe("contraste dos temas (Fase 14)", async () => {
  const { contrast, themeTokens } = await import("@momento/ui");
  it("texto cumpre WCAG AA nos dois temas", () => {
    for (const name of ["dark", "light"] as const) {
      const t = themeTokens[name];
      for (const bg of ["--bg", "--surface", "--surface-2"]) {
        expect(contrast(t["--text"]!, t[bg]!), `${name} texto/${bg}`).toBeGreaterThanOrEqual(7);
        expect(contrast(t["--text-muted"]!, t[bg]!), `${name} secundário/${bg}`).toBeGreaterThanOrEqual(4.5);
        expect(contrast(t["--text-faint"]!, t[bg]!), `${name} terciário/${bg}`).toBeGreaterThanOrEqual(4.5);
      }
      expect(contrast(t["--on-accent"]!, t["--accent"]!), `${name} botão principal`).toBeGreaterThanOrEqual(4.5);
      expect(contrast(t["--accent"]!, t["--bg"]!), `${name} destaque/fundo (foco, ≥ 3:1)`).toBeGreaterThanOrEqual(3);
    }
  });
});

describe("zoom do editor", async () => {
  const { zoomRegion, stepZoom, fitZoom, anchorCenter } = await import("./zoom");
  const full = { x: 0, y: 0, w: 1, h: 1 };
  it("100 % numa fotografia de 6000×4000 mostra só a zona do ecrã, em resolução total", () => {
    const r = zoomRegion(full, { w: 6000, h: 4000 }, { w: 1500, h: 1000 }, 1, { x: 0.5, y: 0.5 });
    expect(r.outW).toBe(1500);
    expect(r.outH).toBe(1000);
    expect(r.crop.w).toBeCloseTo(0.25);
    expect(r.crop.x).toBeCloseTo(0.375);
    expect(r.hiRes).toBe(true);
    // A vista nunca sai da fotografia (centro corrigido junto à borda).
    const edge = zoomRegion(full, { w: 6000, h: 4000 }, { w: 1500, h: 1000 }, 1, { x: 0, y: 1 });
    expect(edge.crop.x).toBeCloseTo(0);
    expect(edge.crop.y + edge.crop.h).toBeCloseTo(1);
  });
  it("dentro de um recorte e com zoom baixo usa a fonte de edição", () => {
    const crop = { x: 0.2, y: 0.1, w: 0.5, h: 0.5 };
    const r = zoomRegion(crop, { w: 3000, h: 2000 }, { w: 1500, h: 1000 }, 0.5, { x: 0.5, y: 0.5 });
    expect(r.crop).toMatchObject({ x: 0.2, y: 0.1 });
    expect(r.crop.w).toBeCloseTo(0.5);
    expect(r.hiRes).toBe(false);
  });
  it("passos e âncora no cursor", () => {
    const fit = fitZoom({ w: 6000, h: 4000 }, { w: 1500, h: 1000 });
    expect(fit).toBeCloseTo(0.25);
    expect(stepZoom(null, fit, 1)).toBe(0.5);
    expect(stepZoom(1, fit, 1)).toBe(2);
    expect(stepZoom(0.5, fit, -1)).toBeNull();
    expect(stepZoom(4, fit, 1)).toBe(4);
    // O ponto sob o cursor fica no mesmo sítio.
    const c = anchorCenter({ cx: 0.5, cy: 0.5, rw: 1, rh: 1 }, 0.8, 0.2, { rw: 0.25, rh: 0.25 });
    expect(c.x - 0.125 + 0.8 * 0.25).toBeCloseTo(0.8);
    expect(c.y - 0.125 + 0.2 * 0.25).toBeCloseTo(0.2);
  });
});
