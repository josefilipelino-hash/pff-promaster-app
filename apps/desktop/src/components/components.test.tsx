import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import "../i18n";
import type { PhotoRow, ProjectInfo } from "../types";

// --- Simulação do núcleo e das APIs Tauri ---------------------------------------------
const apiMock = {
  available: vi.fn(() => true),
  appInfo: vi.fn(async () => ({ appVersion: "0.1.0", coreVersion: "0.1.0", catalogSchemaVersion: 2, os: "macos", arch: "aarch64" })),
  eventTypes: vi.fn(async () => ["wedding", "baptism"]),
  recentProjects: vi.fn(async () => [] as unknown[]),
  forgetRecent: vi.fn(async () => {}),
  createProject: vi.fn(),
  openProject: vi.fn(),
  closeProject: vi.fn(async () => {}),
  currentProject: vi.fn(async () => null),
  listPhotos: vi.fn(async () => [] as PhotoRow[]),
  photoDetails: vi.fn(async () => null),
  setStars: vi.fn(async () => 1),
  setFlag: vi.fn(async () => 1),
  setColor: vi.fn(async () => 1),
  startImport: vi.fn(async () => 1),
  cancelImport: vi.fn(async () => {}),
  prioritizeThumbnails: vi.fn(async () => {}),
  saveUiState: vi.fn(async () => {}),
  startAnalysis: vi.fn(async () => 10),
  cancelAnalysis: vi.fn(async () => {}),
  analysisStatus: vi.fn(async () => analysisStatus(0, 10)),
  setSelectionSettings: vi.fn(async () => summary),
  recomputeSelection: vi.fn(async () => summary),
  listRecommendations: vi.fn(async () => [] as unknown[]),
  listGroups: vi.fn(async () => [] as unknown[]),
  analysisDetails: vi.fn(async () => ({ analysis: null, faces: [], recommendation: null, groupId: null, error: null }) as unknown),
  acceptRecommendations: vi.fn(async () => 1),
    acceptGroup: vi.fn(async () => 2),
  rebuildOrganization: vi.fn(async () => ({})),
  listPersons: vi.fn(async () => [] as unknown[]),
  personFaces: vi.fn(async () => [] as unknown[]),
  photoPersons: vi.fn(async () => [] as [number, number][]),
  updatePerson: vi.fn(async () => {}),
  mergePersons: vi.fn(async () => {}),
  excludeFaces: vi.fn(async () => 1),
  reclusterPeople: vi.fn(async () => 2),
  clearPeople: vi.fn(async () => {}),
  listMoments: vi.fn(async () => [] as unknown[]),
  assetMoments: vi.fn(async () => [] as unknown[]),
  createMoment: vi.fn(async () => 99),
  renameMoment: vi.fn(async () => {}),
  deleteMoment: vi.fn(async () => {}),
  assignMoment: vi.fn(async () => 2),
  resetMoments: vi.fn(async () => 1),
  reclassifyMoments: vi.fn(async () => [0, 0]),
  listCameras: vi.fn(async () => [] as unknown[]),
  suggestOffsets: vi.fn(async () => [] as unknown[]),
  setCameraOffsets: vi.fn(async () => ({})),
};
const summary = { best: 1, alternative: 1, review: 0, reject: 1, duplicate: 0, groups: 1, sequences: 1, durationMs: 1 };
function analysisStatus(analyzed: number, total: number) {
  return {
    running: false,
    counts: { total, analyzed, failed: 0, pending: total - analyzed },
    modelsAvailable: true,
    settings: { mode: "balanced", target: null },
    summary,
  };
}
vi.mock("../api", async (orig) => {
  const real = await orig<typeof import("../api")>();
  const noop = async () => () => {};
  return { ...real, api: apiMock, events: { importProgress: noop, importFinished: noop, thumbnails: noop, analysis: noop } };
});
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({ onDragDropEvent: async () => () => {} }) }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ isFullscreen: async () => false, setFullscreen: vi.fn(async () => {}) }) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => null) }));

beforeAll(() => {
  globalThis.ResizeObserver = class {
    observe() {}
    disconnect() {}
    unobserve() {}
  } as unknown as typeof ResizeObserver;
});

const { App } = await import("../App");
const { useStore } = await import("../store");
const { folderPreview } = await import("./NewProjectDialog");
const { computeLayout, visibleRows } = await import("./PhotoGrid");
const { fitScale, stepZoom } = await import("./Viewer");
const { clampCenter, fitCss, zoomAt, FIT_VIEW } = await import("./CompareView");
const { explain, joinList } = await import("../logic/explain");
const { parseOffset, formatOffset } = await import("../logic/clock");
const { faceCrop } = await import("./FaceThumb");
const i18n = (await import("../i18n")).default;

const project: ProjectInfo = {
  path: "/trabalho/2026-06-15_Ana_e_Pedro.momento",
  name: "Casamento Ana & Pedro",
  clientName: "Ana & Pedro",
  eventType: "wedding",
  eventDate: "2026-06-15",
  location: "Sintra",
  originalsDir: null,
  status: "imported",
  notes: "",
  createdAt: 0,
  updatedAt: 0,
  photoCount: 3000,
  uiState: null,
  importRunning: false,
  imageToken: 1,
};

const rows = (n: number): PhotoRow[] =>
  Array.from({ length: n }, (_, i) => ({
    id: i + 1,
    fileName: `IMG_${String(i + 1).padStart(5, "0")}.jpg`,
    captureTime: null,
    width: 6000,
    height: 4000,
    stars: 0,
    flag: i === 1 ? 1 : 0,
    color: null,
    thumb: "ready" as const,
    partial: i === 2,
    format: i === 2 ? "cr3" : "jpeg",
  }));

beforeEach(() => {
  vi.clearAllMocks();
  useStore.getState().setProject(null);
});
afterEach(cleanup);

describe("ecrã inicial", () => {
  it("mostra ações e projetos recentes em pt-PT", async () => {
    apiMock.recentProjects.mockResolvedValueOnce([
      { path: "/p.momento", name: "Batizado Maria", clientName: "Família Silva", eventType: "baptism", eventDate: "2026-03-01", location: "Porto", photoCount: 812, lastOpened: Date.now(), available: true },
      { path: "/q.momento", name: "Concerto", clientName: "", eventType: "concert", eventDate: null, location: "", photoCount: 0, lastOpened: Date.now(), available: false },
    ]);
    render(<App />);
    expect(await screen.findByText("Batizado Maria")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Novo projeto" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Abrir projeto…" })).toBeTruthy();
    expect(screen.getByText(/812 fotografias/)).toBeTruthy();
    expect(screen.getByText(/Pasta não encontrada/)).toBeTruthy();
  });

  it("formulário de novo projeto valida campos obrigatórios", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Novo projeto" }));
    const dialog = screen.getByRole("dialog", { name: "Novo projeto" });
    const create = within(dialog).getByRole("button", { name: "Criar projeto" }) as HTMLButtonElement;
    expect(create.disabled).toBe(true);
    fireEvent.change(within(dialog).getByPlaceholderText("Ex.: Casamento Ana & Pedro"), { target: { value: "Casamento" } });
    fireEvent.change(within(dialog).getByPlaceholderText("Pasta onde o projeto será criado"), { target: { value: "/trabalho" } });
    expect(create.disabled).toBe(false);
    expect(within(dialog).getByText(/\/trabalho\/Casamento\.momento/)).toBeTruthy();
  });

  it("fora do Tauri mostra aviso", () => {
    apiMock.available.mockReturnValueOnce(false);
    render(<App />);
    expect(screen.getByText(/fora da aplicação desktop/)).toBeTruthy();
  });
});

describe("ecrã de projeto", () => {
  async function openProject(n: number) {
    apiMock.listPhotos.mockResolvedValue(rows(n));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
  }

  it("grelha virtualizada: 3000 fotografias não criam 3000 elementos", async () => {
    await openProject(3000);
    expect(screen.getByRole("button", { name: /Todas\s*3/ })).toBeTruthy();
    const cells = document.querySelectorAll(".cell");
    expect(cells.length).toBeGreaterThan(0);
    expect(cells.length).toBeLessThan(200);
  });

  it("secções futuras estão desativadas e identificadas", async () => {
    await openProject(5);
        for (const name of [/Exportação/]) {
      const b = screen.getByRole("button", { name }) as HTMLButtonElement;
      expect(b.disabled).toBe(true);
    }
  });

  it("atalhos classificam a fotografia com foco e gravam no núcleo", async () => {
    await openProject(10);
    fireEvent.mouseDown(document.querySelector('[data-id="3"]')!);
    fireEvent.keyDown(window, { key: "4" });
    fireEvent.keyDown(window, { key: "p" });
    fireEvent.keyDown(window, { key: "7" });
    await act(async () => {});
    expect(apiMock.setStars).toHaveBeenCalledWith([3], 4);
    expect(apiMock.setFlag).toHaveBeenCalledWith([3], 1);
    expect(apiMock.setColor).toHaveBeenCalledWith([3], "yellow");
    const p = useStore.getState().photos.find((r) => r.id === 3)!;
    expect([p.stars, p.flag, p.color]).toEqual([4, 1, "yellow"]);
  });

  it("seleção múltipla aplica a classificação a todas", async () => {
    await openProject(10);
    fireEvent.mouseDown(document.querySelector('[data-id="2"]')!);
    fireEvent.mouseDown(document.querySelector('[data-id="4"]')!, { shiftKey: true });
    fireEvent.keyDown(window, { key: "x" });
    await act(async () => {});
    expect(apiMock.setFlag).toHaveBeenCalledWith(expect.arrayContaining([2, 3, 4]), -1);
    expect((apiMock.setFlag.mock.calls[0] as unknown[])[0]).toHaveLength(3);
  });

  it("filtro Favoritas e abertura do visualizador", async () => {
    await openProject(10);
    fireEvent.click(screen.getByRole("button", { name: /Favoritas\s*1/ }));
    expect(document.querySelectorAll(".cell").length).toBe(1);
    fireEvent.doubleClick(document.querySelector('[data-id="2"]')!);
    expect(await screen.findByText(/1 de 1/)).toBeTruthy();
    expect(screen.getByRole("button", { name: /Ajustar/ })).toBeTruthy();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByText(/1 de 1/)).toBeNull();
  });

  it("revertida se o núcleo recusar a classificação", async () => {
    await openProject(3);
    apiMock.setStars.mockRejectedValueOnce({ code: "error.database", detail: "x" });
    fireEvent.mouseDown(document.querySelector('[data-id="1"]')!);
    fireEvent.keyDown(window, { key: "5" });
    await act(async () => {});
    expect(useStore.getState().photos[0]!.stars).toBe(0);
    expect(await screen.findByRole("alert")).toBeTruthy();
  });
});

describe("formatos e estados (Fase 2)", () => {
  it("mostra o tipo de ficheiro e o selo de suporte parcial na grelha", async () => {
    apiMock.listPhotos.mockResolvedValue(rows(5).map((r, i) => (i === 2 ? { ...r, fileName: "IMG_0003.CR3" } : r)));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
    const cell = document.querySelector('[data-id="3"]')!;
    expect(cell.textContent).toContain("CR3");
    expect(cell.textContent).toContain("Parcial");
    expect(document.querySelectorAll(".cell-partial").length).toBe(1);
  });

  it("painel de informação explica suporte parcial e ficheiros ilegíveis", async () => {
    apiMock.listPhotos.mockResolvedValue(rows(3).map((r, i) => (i === 0 ? { ...r, thumb: "failed" as const } : r)));
    apiMock.photoDetails.mockResolvedValue({
      id: 1, path: "/f/IMG.NEF", sourcePath: null, fileName: "IMG.NEF", fileSize: 1000, format: "raw-tiff", width: 6000, height: 4000,
      orientation: 1, captureTime: null, captureOffset: null, cameraMake: "Nikon", cameraModel: "Z 9", cameraSerial: null, lens: null,
      iso: null, exposureTime: null, fNumber: null, focalLength: null, focalLength35mm: null, importedAt: 0, thumbError: "dados truncados",
      bitDepth: 14, colorSpace: "Câmara (RGBG, 3 cores)", codec: "libraw", codecDetail: "LibRaw 0.22.3", embeddedPreview: "8256×5504",
      partialNote: "Só a preview embutida pode ser mostrada.",
    } as never);
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
    fireEvent.mouseDown(document.querySelector('[data-id="1"]')!);
    expect(await screen.findByText("Não foi possível ler esta fotografia")).toBeTruthy();
    expect(screen.getByText("Suporte parcial")).toBeTruthy();
    expect(screen.getByText("NEF (RAW)")).toBeTruthy();
    expect(screen.getByText("14 bits")).toBeTruthy();
    expect(screen.getByText("8256×5504")).toBeTruthy();
  });
});

describe("seleção assistida (Fase 3)", () => {
  const recs = [
    { asset: 1, label: "best", groupId: 7, rank: 0, groupSize: 3 },
    { asset: 2, label: "alternative", groupId: 7, rank: 1, groupSize: 3 },
    { asset: 3, label: "reject", groupId: 7, rank: 2, groupSize: 3 },
    { asset: 4, label: "review", groupId: null, rank: 0, groupSize: 1 },
  ];
  async function openAnalyzed() {
    apiMock.listPhotos.mockResolvedValue(rows(6).map((r) => ({ ...r, flag: 0 })));
    apiMock.analysisStatus.mockResolvedValue(analysisStatus(6, 6));
    apiMock.listRecommendations.mockResolvedValue(recs);
    apiMock.listGroups.mockResolvedValue([{ id: 7, kind: "sequence", size: 3, members: [1, 2, 3] }]);
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
  }

  it("antes da análise: explica e oferece analisar (nada corre sozinho)", async () => {
    apiMock.listPhotos.mockResolvedValue(rows(10));
    apiMock.analysisStatus.mockResolvedValue(analysisStatus(0, 10));
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
    expect(screen.getByText(/Nada é enviado para a internet/)).toBeTruthy();
    expect(apiMock.startAnalysis).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Analisar 10 fotografias" }));
    await act(async () => {});
    expect(apiMock.startAnalysis).toHaveBeenCalledWith(false);
  });

  it("etiquetas na grelha, contagens e filtro por etiqueta", async () => {
    await openAnalyzed();
    expect(document.querySelector('[data-id="1"]')!.textContent).toContain("Melhor");
    expect(document.querySelector('[data-id="1"]')!.textContent).toContain("1/3");
    expect(document.querySelector('[data-id="3"]')!.textContent).toContain("Rejeitar?");
    fireEvent.click(screen.getByRole("button", { name: /Rejeitar sugerido\s*1/ }));
    expect(document.querySelectorAll(".cell").length).toBe(1);
    expect(document.querySelector('[data-id="3"]')).toBeTruthy();
  });

  it("modo e objetivo de entrega são enviados ao núcleo", async () => {
    await openAnalyzed();
    fireEvent.change(screen.getByRole("combobox", { name: /Modo/ }), { target: { value: "strict" } });
    await act(async () => {});
    expect(apiMock.setSelectionSettings).toHaveBeenCalledWith({ mode: "strict", target: null });
    const input = screen.getByPlaceholderText("Sem objetivo");
    fireEvent.change(input, { target: { value: "350" } });
    fireEvent.blur(input);
    await act(async () => {});
    expect(apiMock.setSelectionSettings).toHaveBeenCalledWith({ mode: "balanced", target: 350 });
  });

  it("vista de sequências: aceitar sugestões do grupo e comparar", async () => {
    await openAnalyzed();
    fireEvent.click(screen.getByRole("button", { name: /^Sequências\s*1$/ }));
    const row = screen.getByRole("region", { name: "Sequência" });
    expect(within(row).getByText(/3 fotografias/)).toBeTruthy();
    fireEvent.click(within(row).getByRole("button", { name: "Aceitar sugestões" }));
    await act(async () => {});
    expect(apiMock.acceptGroup).toHaveBeenCalledWith(7);
    fireEvent.click(within(row).getByRole("button", { name: "Comparar" }));
    const cmp = screen.getByRole("region", { name: "Comparação" });
    // Melhor + alternativa (a rejeitada fica de fora da comparação inicial).
    expect(cmp.querySelectorAll(".compare-pane").length).toBe(2);
  });

  it("comparação: teclas no painel ativo, trocar por outra e fechar", async () => {
    await openAnalyzed();
    fireEvent.mouseDown(document.querySelector('[data-id="1"]')!);
    fireEvent.keyDown(window, { key: "c" });
    const cmp = screen.getByRole("region", { name: "Comparação" });
    expect(cmp.querySelectorAll(".compare-pane").length).toBe(3);
    fireEvent.keyDown(window, { key: "ArrowRight" });
    fireEvent.keyDown(window, { key: "x" });
    fireEvent.keyDown(window, { key: "3" });
    await act(async () => {});
    expect(apiMock.setFlag).toHaveBeenCalledWith([2], -1);
    expect(apiMock.setStars).toHaveBeenCalledWith([2], 3);
    const swap = within(cmp).getAllByRole("combobox", { name: "Selecionar outra…" })[0]!;
    fireEvent.change(swap, { target: { value: "3" } });
    expect(useStore.getState().compare).toEqual([3, 2, 1]);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("region", { name: "Comparação" })).toBeNull();
  });

  it("painel de informação: explicação e pontuações individuais", async () => {
    apiMock.analysisDetails.mockResolvedValue({
      analysis: { models: "x", width: 2048, height: 1365, sharpness: 2, focusPeak: 2.5, focusCenter: 2, motionBlur: 0.1, brightness: 0.5, highlightsClipped: 0.01, shadowsCrushed: 0, contrast: 0.6, noise: 2, tiltDegrees: 0.4, phash: 1, dhash: 2, durationMs: 100 },
      faces: [{ x: 0.1, y: 0.1, w: 0.1, h: 0.1, px: 160, detScore: 0.9, presence: 0.99, blinkLeft: 0.1, blinkRight: 0.12, smile: 0.8, jawOpen: 0.1, sharpness: 2.4, eyeSharpness: 2.5, cut: false }],
      recommendation: { asset: 1, label: "best", rank: 0, groupSize: 3, summary: "bestInSequence", reasons: '[{"code":"sharpestFace"},{"code":"allFacesVisible","value":2},{"code":"bestExposure"}]', focus: 0.95, motion: 0.9, exposure: 0.88, noise: 0.8, eyes: 1, expression: 0.8, composition: 1, technical: 0.9, people: 0.93, overall: 0.92 },
      groupId: 7,
      error: null,
    });
    await openAnalyzed();
    fireEvent.mouseDown(document.querySelector('[data-id="1"]')!);
    expect(await screen.findByText("Melhor fotografia desta sequência: maior nitidez facial, ambos os rostos visíveis e melhor exposição.")).toBeTruthy();
    expect(screen.getByText("Olhos abertos")).toBeTruthy();
    expect(screen.getByText("Sequência de 3 · 1.ª")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Aceitar: marcar favorita" }));
    await act(async () => {});
    expect(apiMock.acceptRecommendations).toHaveBeenCalledWith([1]);
  });
});

describe("organização do casamento (Fase 4)", () => {
  const moments = [
    { id: 1, kind: "bride_prep", name: null, sort: 10, custom: false, count: 1 },
    { id: 13, kind: "kiss", name: null, sort: 130, custom: false, count: 1 },
    { id: 21, kind: "cake", name: null, sort: 210, custom: false, count: 0 },
    { id: 50, kind: null, name: "Fogo de artifício", sort: 999, custom: true, count: 0 },
  ];
  const persons = [
    { id: 7, name: "Ana", role: "bride", hidden: false, faceCount: 3, photoCount: 3, cover: { faceId: 1, asset: 1, x: 0.4, y: 0.2, w: 0.1, h: 0.1, source: 1 } },
    { id: 8, name: "Pedro", role: "groom", hidden: false, faceCount: 2, photoCount: 2, cover: null },
    { id: 9, name: null, role: "bride_mother", hidden: false, faceCount: 1, photoCount: 1, cover: null },
    { id: 10, name: null, role: null, hidden: false, faceCount: 4, photoCount: 4, cover: null },
  ];
  async function openOrganized() {
    apiMock.listPhotos.mockResolvedValue(rows(6).map((r) => ({ ...r, flag: 0 })));
    apiMock.analysisStatus.mockResolvedValue(analysisStatus(6, 6));
    apiMock.listMoments.mockResolvedValue(moments);
    apiMock.assetMoments.mockResolvedValue([
      { asset: 1, momentId: 1, confidence: 0.72, source: "auto", reasons: "[]" },
      { asset: 2, momentId: 13, confidence: null, source: "user", reasons: null },
    ]);
    apiMock.listPersons.mockResolvedValue(persons);
    // Fotografia 1: noiva; 2: noiva + noivo; 3: noiva + mãe da noiva + noivo; 4: noivo.
    apiMock.photoPersons.mockResolvedValue([[1, 7], [2, 7], [2, 8], [3, 7], [3, 9], [3, 8], [4, 8]]);
    act(() => useStore.getState().setProject(project));
    render(<App />);
    await screen.findByText("Casamento Ana & Pedro");
    await act(async () => {});
  }
  const cells = () => [...document.querySelectorAll(".cell")].map((c) => Number(c.getAttribute("data-id")));

  it("pesquisas: noiva, noivos juntos, noiva + mãe, uma pessoa", async () => {
    await openOrganized();
    fireEvent.click(screen.getByRole("button", { name: /Todas as fotografias da noiva/ }));
    expect(cells()).toEqual([1, 2, 3]);
    fireEvent.click(screen.getByRole("button", { name: /Noivos juntos/ }));
    expect(cells()).toEqual([2, 3]);
    fireEvent.click(screen.getByRole("button", { name: /Noiva \+ mãe/ }));
    expect(cells()).toEqual([3]);
    expect((screen.getByRole("button", { name: /Noivo \+ pai/ }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: /^Pedro\s*2$/ }));
    expect(cells()).toEqual([2, 3, 4]);
  });

  it("momentos: filtro, por classificar e arrastar fotografias para um momento", async () => {
    await openOrganized();
    const panel = screen.getByRole("region", { name: "Momentos" });
    expect(within(panel).getByText("Preparação da noiva")).toBeTruthy();
    expect(within(panel).queryByText("Bolo")).toBeNull(); // vazio: escondido
    expect(within(panel).getByText("Fogo de artifício")).toBeTruthy(); // personalizado: sempre
    fireEvent.click(within(panel).getByRole("button", { name: /Por classificar\s*4/ }));
    expect(cells()).toEqual([3, 4, 5, 6]);
        fireEvent.click(within(panel).getByRole("button", { name: /Por classificar/ }));
    
        // Arrastar a fotografia 3 (com a 4 selecionada também) para “Fogo de artifício”.
    fireEvent.mouseDown(document.querySelector('[data-id="3"]')!);
    fireEvent.mouseDown(document.querySelector('[data-id="4"]')!, { shiftKey: true });
    fireEvent.mouseMove(window, { clientX: 300, clientY: 300, buttons: 1 });
    expect(await screen.findByText(/Largue 2 fotografias num momento/)).toBeTruthy();
        expect(within(panel).queryByText("Bolo")).toBeNull(); // a lista não muda durante o arrasto
    fireEvent.mouseUp(within(panel).getByRole("button", { name: /Fogo de artifício/ }));
    await act(async () => {});
        expect(apiMock.assignMoment).toHaveBeenCalledWith([3, 4], 50);
    expect(screen.queryByText(/Largue 2 fotografias/)).toBeNull(); // regressão: o aviso desaparece ao largar
  });

  it("criar momento personalizado e escolher momento no painel", async () => {
    await openOrganized();
    fireEvent.click(screen.getByRole("button", { name: /\+ Novo momento/ }));
    const input = screen.getByPlaceholderText("Nome do momento");
    fireEvent.change(input, { target: { value: "Fogo de artifício 2" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.blur(input);
    await act(async () => {});
    expect(apiMock.createMoment).toHaveBeenCalledWith("Fogo de artifício 2", null);
    fireEvent.mouseDown(document.querySelector('[data-id="1"]')!);
    expect(await screen.findByText("Automático · confiança 72 %")).toBeTruthy();
    fireEvent.change(screen.getByRole("combobox", { name: /Momento/ }), { target: { value: "21" } });
    await act(async () => {});
    expect(apiMock.assignMoment).toHaveBeenCalledWith([1], 21);
  });

  it("gerir pessoas: nome, papel, não é esta pessoa, juntar", async () => {
    apiMock.personFaces.mockResolvedValue([{ faceId: 31, asset: 1, x: 0.4, y: 0.2, w: 0.1, h: 0.1, source: 0 }]);
    await openOrganized();
    fireEvent.click(screen.getByRole("button", { name: /^Pessoas$/ }));
    const card = await screen.findByRole("article", { name: "Pessoa 10" });
    fireEvent.change(within(card).getByRole("textbox", { name: "Nome" }), { target: { value: "Rui" } });
    fireEvent.blur(within(card).getByRole("textbox", { name: "Nome" }));
    await act(async () => {});
    expect(apiMock.updatePerson).toHaveBeenCalledWith(10, "Rui", null, false);
    fireEvent.change(within(card).getByRole("combobox", { name: "Papel" }), { target: { value: "best_man" } });
    await act(async () => {});
    // Regressão: mudar o papel logo a seguir mantém o nome acabado de escrever.
    expect(apiMock.updatePerson).toHaveBeenLastCalledWith(10, "Rui", "best_man", false);
    fireEvent.click(await within(card).findByRole("button", { name: "Não é esta pessoa" }));
    await act(async () => {});
    expect(apiMock.excludeFaces).toHaveBeenCalledWith([31]);
    fireEvent.click(within(card).getByRole("checkbox", { name: "Selecionar" }));
    fireEvent.click(within(screen.getByRole("article", { name: "Ana" })).getByRole("checkbox", { name: "Selecionar" }));
    fireEvent.click(screen.getByRole("button", { name: "Juntar 2 selecionadas" }));
    await act(async () => {});
    expect(apiMock.mergePersons).toHaveBeenCalledWith(10, 7);
  });

  it("sincronizar câmaras: sugestão (Sony +3min42s, DJI −1min15s) e reconstruir", async () => {
    apiMock.listCameras.mockResolvedValue([
      { key: "Canon EOS R5", make: "Canon", model: "EOS R5", serial: null, photos: 900, firstTime: "2026-06-15T10:00:00", lastTime: "2026-06-15T23:50:00", offsetMs: 0, reference: false },
      { key: "SONY ILCE-7M4", make: "SONY", model: "ILCE-7M4", serial: null, photos: 400, firstTime: null, lastTime: null, offsetMs: 0, reference: false },
      { key: "DJI FC3582", make: "DJI", model: "FC3582", serial: null, photos: 40, firstTime: null, lastTime: null, offsetMs: 0, reference: false },
    ]);
    apiMock.suggestOffsets.mockResolvedValue([
      { camera: "SONY ILCE-7M4", offsetMs: -222000, votes: 31, agreement: 0.94 },
      { camera: "DJI FC3582", offsetMs: 75000, votes: 6, agreement: 0.8 },
    ]);
    await openOrganized();
    fireEvent.click(screen.getByRole("button", { name: /Sincronizar câmaras/ }));
    const dialog = await screen.findByRole("dialog", { name: "Sincronizar câmaras" });
    await act(async () => {});
    fireEvent.click(within(dialog).getByRole("button", { name: "Sugerir automaticamente" }));
    await act(async () => {});
    expect(apiMock.suggestOffsets).toHaveBeenCalledWith("Canon EOS R5");
    expect((within(dialog).getByRole("textbox", { name: /Desvio de SONY/ }) as HTMLInputElement).value).toBe("+3min42s");
    expect((within(dialog).getByRole("textbox", { name: /Desvio de DJI/ }) as HTMLInputElement).value).toBe("-1min15s");
    fireEvent.change(within(dialog).getByRole("textbox", { name: /Desvio de DJI/ }), { target: { value: "abc" } });
    expect((within(dialog).getByRole("button", { name: /Aplicar/ }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(within(dialog).getByRole("textbox", { name: /Desvio de DJI/ }), { target: { value: "-1:15" } });
    fireEvent.click(within(dialog).getByRole("button", { name: /Aplicar/ }));
    await act(async () => {});
    expect(apiMock.setCameraOffsets).toHaveBeenCalledWith([
      { key: "Canon EOS R5", offsetMs: 0, reference: true },
      { key: "SONY ILCE-7M4", offsetMs: -222000, reference: false },
      { key: "DJI FC3582", offsetMs: 75000, reference: false },
    ]);
  });
});

describe("cálculos de layout", () => {
  it("desvios de relógio", () => {
    expect(parseOffset("+3min42s")).toBe(222000);
    expect(parseOffset("+3m42s")).toBe(222000);
    expect(parseOffset("-1min15s")).toBe(-75000);
    expect(parseOffset("+00:03:42")).toBe(222000);
    expect(parseOffset("-1:15")).toBe(-75000);
    expect(parseOffset("+1h2m")).toBe(3720000);
    expect(parseOffset("0")).toBe(0);
    expect(parseOffset("abc")).toBeNull();
    expect(formatOffset(222000)).toBe("+3min42s");
    expect(formatOffset(-75000)).toBe("-1min15s");
    expect(formatOffset(3600000)).toBe("+1h");
    expect(formatOffset(0)).toBe("0");
  });
  it("recorte do rosto a partir da miniatura", () => {
    // Rosto de 10 % no centro de uma fotografia 6000×4000, avatar de 60 px.
    const c = faceCrop({ faceId: 1, asset: 1, x: 0.45, y: 0.45, w: 0.1, h: 0.1, source: 0 }, 6000, 4000, 60);
        const nums = (v: string) => v.split(" ").map((x) => parseFloat(x));
    const [sw, sh] = nums(c.backgroundSize); // lado = 600 × 1,5 = 900 px → escala 60/900
    expect(sw).toBeCloseTo(400);
    expect(sh).toBeCloseTo(266.667, 2);
    const [px, py] = nums(c.backgroundPosition);
    expect(px).toBeCloseTo(-170);
    expect(py).toBeCloseTo(-103.333, 2);
  });
  it("explicações em linguagem natural", () => {
    const t = i18n.t.bind(i18n) as (k: string, o?: Record<string, unknown>) => string;
    expect(joinList(["a", "b", "c"], "e")).toBe("a, b e c");
    expect(explain("rejectSuggested", [{ code: "eyesClosed", value: 2 }, { code: "tilted", value: -3.44 }], t, "pt-PT")).toBe("Rejeição sugerida: olhos fechados em 2 pessoas e inclinada -3,4°.");
    expect(explain("goodSingle", [], t, "pt-PT")).toBe("Boa fotografia.");
  });
  it("comparação sincronizada", () => {
    expect(fitCss(6000, 4000, 600, 600)).toBeCloseTo(0.1);
    expect(clampCenter(0.9, 300, 600)).toBe(0.5); // imagem menor que o painel: centrada
    expect(clampCenter(0.99, 1200, 600)).toBeCloseTo(0.75);
    // O ponto sob o cursor mantém-se: zoom 1→2 a 100 px à direita do centro (disp 600).
    const v = zoomAt(FIT_VIEW, 2, 100, 0, 600, 400);
    const before = 0.5 + 100 / 600;
    expect(v.cx + 100 / 1200).toBeCloseTo(before);
    expect(zoomAt(v, 0.5, 0, 0, 1200, 800)).toEqual(FIT_VIEW);
  });
  it("pré-visualização do nome da pasta igual ao núcleo Rust", () => {
    // Mesmos casos que packages/project-management/src/paths.rs
    expect(folderPreview("2026-06-15", "Ana & Pedro", "x")).toBe("2026-06-15_Ana_e_Pedro.momento");
    expect(folderPreview("", "", "Batizado / Maria")).toBe("Batizado_Maria.momento");
    expect(folderPreview("", "  ", "  ")).toBe("Projeto.momento");
    expect(folderPreview("2026-01-01", "João Ção", "")).toBe("2026-01-01_João_Ção.momento");
  });
  it("grelha preenche a largura", () => {
    const l = computeLayout(1000, 180);
    expect(l.columns).toBe(5);
    expect(l.cell * l.columns + 8 * (l.columns - 1)).toBeLessThanOrEqual(1000 - 24);
    expect(visibleRows(0, 600, l.rowHeight, 1000)[0]).toBe(0);
    expect(visibleRows(100_000, 600, l.rowHeight, 1000)[1]).toBeLessThan(1000);
  });
  it("zoom do visualizador", () => {
    // 6000×4000 num palco de 1200×800 em ecrã Retina (dpr 2): ajustar = 40 %.
    expect(fitScale(6000, 4000, 1200, 800, 2)).toBeCloseTo(0.4);
    expect(fitScale(800, 600, 1200, 800, 1)).toBe(1); // nunca amplia além de 100 %
    expect(stepZoom(0.4, 1)).toBe(0.5);
    expect(stepZoom(1, 1)).toBe(2);
    expect(stepZoom(1, -1)).toBe(0.5);
  });
});
