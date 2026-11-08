import { describe, expect, it } from "vitest";
import { imageUrl, isMac } from "../api";
import type { PhotoRow } from "../types";
import { applyFilter, collectionCounts, DEFAULT_FILTER } from "./filters";
import { cameraName, formatAperture, formatBytes, formatCaptureTime, formatEventDate, formatExposure, formatFocal } from "./format";
import { clickSelect, EMPTY_SELECTION, moveFocus, restrict, selectAll, targets } from "./selection";
import { keyToAction } from "./shortcuts";

const photo = (id: number, over: Partial<PhotoRow> = {}): PhotoRow => ({
  id,
  fileName: `IMG_${id}.jpg`,
  captureTime: null,
  width: 600,
  height: 400,
  stars: 0,
  flag: 0,
  color: null,
  thumb: "ready",
  ...over,
});

describe("filtros", () => {
  const rows = [photo(1, { flag: 1, stars: 3 }), photo(2, { flag: -1 }), photo(3), photo(4, { stars: 5, color: "red" }), photo(5, { color: "red" })];
  it("coleções da barra lateral", () => {
    expect(applyFilter(rows, { ...DEFAULT_FILTER, collection: "picks" }).map((p) => p.id)).toEqual([1]);
    expect(applyFilter(rows, { ...DEFAULT_FILTER, collection: "rejects" }).map((p) => p.id)).toEqual([2]);
    expect(applyFilter(rows, { ...DEFAULT_FILTER, collection: "unrated" }).map((p) => p.id)).toEqual([3, 5]);
    expect(collectionCounts(rows)).toEqual({ all: 5, picks: 1, rejects: 1, unrated: 2 });
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
