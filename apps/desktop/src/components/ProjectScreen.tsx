import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { api, events, isMac } from "../api";
import { applyFilter, collectionCounts, labelCounts } from "../logic/filters";
import { clickSelect, moveFocus, restrict, selectAll, targets } from "../logic/selection";
import { keyToAction, type Action } from "../logic/shortcuts";
import { cloneSettings, DEFAULT_COPY_BLOCKS } from "../logic/develop";
import { errorText, useStore } from "../store";
import type {
  ImportSafety, Color, GroupRow, ImportMode, Label } from "../types";
import { AutoEditDialog, AutoEditQueue } from "./AutoEdit";
import { ExportDialog, ExportQueue } from "./Export";
import { GalleryDialog } from "./Gallery";
import { AlbumView } from "./AlbumView";
import { DeleteDialog, SafetyDialog } from "./SafetyDialogs";
import { WorkflowBar } from "./WorkflowBar";
import { ConsistencyView } from "./ConsistencyView";
import { ApplyPresetDialog } from "./Presets";
import { ClockDialog } from "./ClockDialog";
import { CompareView, type CompareHandle } from "./CompareView";
import { PeopleView } from "./PeopleView";
import { DevelopView } from "./develop/DevelopView";
import { SequencesView } from "./SequencesView";
import { HelpOverlay } from "./HelpOverlay";
import { ImportDialog, ImportReport } from "./ImportDialogs";
import { InfoPanel } from "./InfoPanel";
import { PhotoGrid, type GridLayout } from "./PhotoGrid";
import { Sidebar } from "./Sidebar";
import { StatusBar } from "./StatusBar";
import { Toolbar } from "./Toolbar";
import { Filmstrip } from "./Filmstrip";
import { Menu, type MenuEntry } from "./Menu";
import { shortcutLabel } from "../logic/keys";
import { Viewer, type ViewerHandle } from "./Viewer";
import { Icon } from "./Icon";

/** Extensões oferecidas no seletor de ficheiros (a deteção real é feita pelo conteúdo). */
const BASE_EXTENSIONS = [
  "jpg", "jpeg", "jpe", "png", "tif", "tiff", "webp", "avif", "heic", "heif", "hif", "jxl", "jp2", "j2k", "jpx",
  "psd", "psb", "bmp", "gif", "exr", "hdr", "ico", "ppm", "pgm", "pbm", "pnm", "pam", "tga",
  // RAW
  "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "rw2", "raw", "rwl", "orf", "pef", "dng",
  "3fr", "fff", "iiq", "x3f", "srw", "erf", "dcr", "kdc", "mos", "mef", "mrw",
];
const IMAGE_EXTENSIONS = [...BASE_EXTENSIONS, ...BASE_EXTENSIONS.map((e) => e.toUpperCase())];

function isTyping(target: EventTarget | null): boolean {
  const el = target as HTMLInputElement | null;
  if (!el) return false;
  if (el.tagName === "INPUT") return !["range", "checkbox", "radio", "button"].includes(el.type);
  return el.tagName === "SELECT" || el.tagName === "TEXTAREA" || el.isContentEditable;
}

export function ProjectScreen({ onClosed }: { onClosed: () => void }) {
  const { t } = useTranslation();
  const s = useStore();
  const project = s.project!;
  const layoutRef = useRef<GridLayout>({ columns: 1, cell: 180, rowHeight: 212 });
  const viewerRef = useRef<ViewerHandle | null>(null);
  const compareRef = useRef<CompareHandle | null>(null);
  const [compareActive, setCompareActive] = useState(0);
  const [dragging, setDragging] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  /** Menu contextual de fotografias (Fase 14). */
  const [photoMenu, setPhotoMenu] = useState<{ x: number; y: number } | null>(null);
  const mac = isMac();

    const org = useMemo(() => ({ photoPersons: s.photoPersons, photoMoment: s.photoMoment }), [s.photoPersons, s.photoMoment]);
  const visible = useMemo(() => applyFilter(s.photos, s.filter, s.recs, org), [s.photos, s.filter, s.recs, org]);
  const lCounts = useMemo(() => labelCounts(s.photos, s.recs), [s.photos, s.recs]);
  const order = useMemo(() => visible.map((p) => p.id), [visible]);
  const counts = useMemo(() => collectionCounts(s.photos), [s.photos]);
  const focusPhoto = s.selection.focus !== null ? (s.photos[s.index.get(s.selection.focus) ?? -1] ?? null) : null;
  const viewerIndex = s.viewerId !== null ? order.indexOf(s.viewerId) : -1;
  const viewerPhoto = viewerIndex >= 0 ? visible[viewerIndex]! : null;

  const fail = useCallback((e: unknown) => s.toast("error", errorText(e, t)), [s, t]);

  // Carregar fotografias e seleção assistida ao abrir.
  useEffect(() => {
    s.reloadPhotos().catch(fail);
        s.reloadSelection().catch(fail);
    s.reloadOrganization().catch(fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [project.path]);

  // Ao mudar o filtro, a seleção mantém apenas fotografias visíveis.
  useEffect(() => {
    const vis = new Set(order);
    const sel = useStore.getState().selection;
    if ([...sel.ids].some((i) => !vis.has(i)) || (sel.focus !== null && !vis.has(sel.focus))) s.setSelection(restrict(sel, vis));
    if (s.viewerId !== null && !vis.has(s.viewerId)) s.openViewer(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [order]);

  // Eventos do núcleo.
  useEffect(() => {
    let lastReload = 0;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const scheduleReload = () => {
      if (timer) return;
      const wait = Math.max(0, 1500 - (Date.now() - lastReload));
      timer = setTimeout(() => {
        timer = null;
        lastReload = Date.now();
        useStore.getState().reloadPhotos().catch(() => {});
      }, wait);
    };
    api.exportJobs().then((j) => useStore.getState().setExportJobs(j)).catch(() => {});
    // Fase 13: sessão anterior interrompida, catálogo restaurado, exportações a meio.
    api
      .safetyStatus()
      .then((st) => {
        const s0 = useStore.getState();
        if (st.report.restoredFrom != null) s0.toast("error", t("safety.toastRestored"));
        else if (st.report.previousUnclean) s0.toast("info", t("safety.toastUnclean"));
        if (st.pendingExports.length > 0 || st.report.restoredFrom != null) s0.setSafetyDialog(true);
      })
      .catch(() => {});
    const un = [
      events.importProgress((p) => {
        const st = useStore.getState();
        st.setImportRunning(true);
        st.setImportProgress(p);
        if (p.imported > 0) scheduleReload();
      }),
      events.importFinished((sum) => {
        const st = useStore.getState();
        st.setImportRunning(false);
        st.setImportProgress(null);
        st.reloadPhotos().catch(() => {});
        // Contagens da seleção automática ("Analisar N fotografias") com as novas.
        st.reloadSelection().catch(() => {});
        if (sum.issues.length > 0 || sum.cancelled || sum.error || sum.counts.pending > 0) st.setImportSummary(sum);
        else st.toast("info", t("report.toast", { count: sum.counts.imported }));
      }),
            events.thumbnails((b) => useStore.getState().applyThumbBatch(b)),
      events.editsChanged((keys) => {
        useStore.getState().bumpEdited(keys);
        // Numa fila de centenas de fotografias chegam muitos avisos: recarregar com limite.
        scheduleReload();
      }),
      events.availability((a) => {
        const st = useStore.getState();
        st.reloadPhotos().catch(() => {});
        if (a.reconnected > 0) st.toast("info", t("safety.toastReconnected", { count: a.reconnected }));
        if (a.offline + a.missing > 0 && a.changed > a.reconnected) st.toast("error", t("safety.toastUnavailable", { offline: a.offline, missing: a.missing }));
      }),
      events.exportProgress((p) => {
        const st = useStore.getState();
        st.upsertExportJob(p);
        if (p.state === "done") st.toast("info", t("export.toastDone", { count: p.filesDone, failed: p.filesFailed }));
        else if (p.state === "failed") st.toast("error", t("export.toastFailed", { name: p.name }));
      }),
      events.autoEdit((p) => {
        const st = useStore.getState();
        st.setAutoProgress(p);
        if (p.finished && p.total > 0) {
          scheduleReload();
          if (p.kind === "sync") st.toast("info", t("sync.toastFinished", { count: p.done }));
          else st.toast("info", p.cancelled ? t("auto.toastCancelled", { count: p.done }) : t("auto.toastFinished", { count: p.done }));
        }
      }),
      events.analysis((p) => {
        const st = useStore.getState();
        st.setAnalysisProgress(p.finished ? null : p);
                if (p.finished) {
          st.reloadSelection().catch(() => {});
          st.reloadOrganization().catch(() => {});
          st.toast("info", t("selection.finished", { count: p.done }));
        } else if (st.analysis && !st.analysis.running) {
          st.reloadSelection().catch(() => {});
        }
      }),
      getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "enter" || p.type === "over") setDragging(true);
        else if (p.type === "leave") setDragging(false);
        else if (p.type === "drop") {
          setDragging(false);
          if (p.paths.length > 0 && !useStore.getState().importRunning) useStore.getState().setPendingImport({ paths: p.paths, hasFolders: true });
          else if (useStore.getState().importRunning) useStore.getState().toast("info", t("import.busy"));
        }
      }),
    ];
    return () => {
      if (timer) clearTimeout(timer);
      un.forEach((p) => p.then((f) => f()).catch(() => {}));
    };
  }, [t]);

  // Guardar o estado da interface no catálogo (com atraso para agrupar alterações).
  const uiJson = JSON.stringify({ filter: s.filter, thumbSize: s.thumbSize, focusId: s.selection.focus, infoOpen: s.infoOpen, view: s.view });
  useEffect(() => {
    const h = setTimeout(() => api.saveUiState(uiJson).catch(() => {}), 500);
    return () => clearTimeout(h);
  }, [uiJson]);

  // Com objetivo de entrega, as favoritas do utilizador contam: recalcula (com atraso).
  const recomputeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const scheduleRecompute = useCallback(() => {
    if (useStore.getState().analysis?.settings.target == null) return;
    if (recomputeTimer.current) clearTimeout(recomputeTimer.current);
    recomputeTimer.current = setTimeout(() => {
      api
        .recomputeSelection()
        .then(() => useStore.getState().reloadSelection())
        .catch(() => {});
    }, 900);
  }, []);

  const rate = useCallback(
    (kind: "stars" | "flag" | "color", value: number | Color | null, ids?: number[]) => {
      const st = useStore.getState();
      const cmp = st.compare;
      const list = ids ?? (cmp ? [cmp[compareActive] ?? cmp[0]!] : st.viewerId !== null ? [st.viewerId] : targets(st.selection));
      if (list.length === 0) return;
      st.rate(kind, value, list)
        .then(() => kind === "flag" && scheduleRecompute())
        .catch(fail);
    },
    [fail, compareActive, scheduleRecompute],
  );

  const openCompare = useCallback((ids: number[]) => {
    const st = useStore.getState();
    const uniq = [...new Set(ids)].slice(0, 4);
    if (uniq.length < 2) return void st.toast("info", t("compare.needTwo"));
    setCompareActive(0);
    st.openViewer(null);
    st.openCompare(uniq);
  }, [t]);

  /** Comparar: 2–4 selecionadas; ou a sequência da fotografia com foco. */
  const compareFromSelection = useCallback(() => {
    const st = useStore.getState();
    const sel = order.filter((id) => st.selection.ids.has(id));
    if (sel.length >= 2) return openCompare(sel);
    const focus = st.selection.focus;
    const g = focus !== null ? st.recs.get(focus)?.groupId : null;
    const group = g != null ? st.groupById.get(g) : undefined;
    if (group && focus !== null) return openCompare([focus, ...group.members.filter((m) => m !== focus)]);
    st.toast("info", t("compare.needTwo"));
  }, [order, openCompare, t]);

  const acceptIds = useCallback(
    async (ids: number[]) => {
      try {
        const n = await api.acceptRecommendations(ids);
        await useStore.getState().reloadPhotos();
        scheduleRecompute();
        useStore.getState().toast("info", t("selection.accepted", { count: n }));
      } catch (e) {
        fail(e);
      }
    },
    [fail, scheduleRecompute, t],
  );

    // Organização: momentos (arrastar/escolher) e pessoas.
    // Momentos e pessoas são do ficheiro: converte chaves (inclui cópias virtuais) em ficheiros.
  const assetsOf = useCallback((keys: number[]) => {
    const st = useStore.getState();
    return [...new Set(keys.map((k) => st.photos[st.index.get(k) ?? -1]?.asset ?? k))];
  }, []);
  const assignMoment = useCallback(
    async (keys: number[], moment: number | null) => {
      try {
        const n = await api.assignMoment(assetsOf(keys), moment);
        await useStore.getState().reloadOrganization();
        useStore.getState().toast("info", t("moments.moved", { count: n }));
      } catch (e) {
        fail(e);
      }
    },
        [fail, t, assetsOf],
  );
  const resetMoments = useCallback(
    async (keys: number[]) => {
      try {
        await api.resetMoments(assetsOf(keys));
        await useStore.getState().reloadOrganization();
      } catch (e) {
        fail(e);
      }
    },
        [fail, assetsOf],
  );
  const reloadAfterClock = useCallback(async () => {
    const st = useStore.getState();
    await Promise.all([st.reloadPhotos(), st.reloadSelection(), st.reloadOrganization()]).catch(fail);
  }, [fail]);

  const acceptGroup = useCallback(
    async (g: GroupRow) => {
      try {
        const n = await api.acceptGroup(g.id);
        await useStore.getState().reloadPhotos();
        scheduleRecompute();
        useStore.getState().toast("info", t("selection.accepted", { count: n }));
      } catch (e) {
        fail(e);
      }
    },
    [fail, scheduleRecompute, t],
  );

  const toggleFullscreen = useCallback(async () => {
    try {
      const w = getCurrentWindow();
      const next = !(await w.isFullscreen());
      await w.setFullscreen(next);
      setFullscreen(next);
    } catch (e) {
      fail(e);
    }
  }, [fail]);

  const chooseImport = async (folder: boolean) => {
    const sel = await open(
      folder
        ? { directory: true, multiple: true, title: t("import.folder") }
        : { directory: false, multiple: true, title: t("import.files"), filters: [{ name: t("import.imagesFilter"), extensions: IMAGE_EXTENSIONS }] },
    );
    const paths = Array.isArray(sel) ? sel : typeof sel === "string" ? [sel] : [];
    if (paths.length > 0) s.setPendingImport({ paths, hasFolders: folder });
  };

  const startImport = async (mode: ImportMode, recursive: boolean, safety?: ImportSafety) => {
    const pending = s.pendingImport;
    s.setPendingImport(null);
    if (!pending) return;
    try {
      await api.startImport(pending.paths, mode, recursive, safety);
      s.setImportRunning(true);
    } catch (e) {
      fail(e);
    }
  };

  const closeProject = async () => {
    try {
      await api.closeProject();
    } catch (e) {
      fail(e);
    }
    if (fullscreen) await getCurrentWindow().setFullscreen(false).catch(() => {});
    s.setProject(null);
    onClosed();
  };

  const onClickPhoto = useCallback(
    (id: number, e: React.MouseEvent) => {
      const st = useStore.getState();
      const toggle = mac ? e.metaKey : e.ctrlKey;
      const inSelection = st.selection.ids.has(id);
      // Botão direito sobre a seleção: o menu age sobre TODAS as selecionadas (antes a
      // seleção encolhia para uma e "Rejeitar" só rejeitava essa — Fase 15).
      if (e.button === 2 && inSelection) return;
      // Clique simples numa fotografia de uma seleção múltipla: pode ser o início de um
      // arrastar (ex.: para um momento) — a seleção só encolhe ao largar sem arrastar.
      if (e.button === 0 && inSelection && st.selection.ids.size > 1 && !toggle && !e.shiftKey) {
        st.setSelection({ ...st.selection, focus: id });
        const x0 = e.clientX;
        const y0 = e.clientY;
        const up = (u: MouseEvent) => {
          window.removeEventListener("mouseup", up, true);
          if (Math.abs(u.clientX - x0) < 4 && Math.abs(u.clientY - y0) < 4) {
            const s2 = useStore.getState();
            s2.setSelection(clickSelect(s2.selection, order, id, { toggle: false, range: false }));
          }
        };
        window.addEventListener("mouseup", up, true);
        return;
      }
      st.setSelection(clickSelect(st.selection, order, id, { toggle, range: e.shiftKey }));
    },
    [order, mac],
  );

  // Teclado.
  const handleActionRef = useRef<((a: Action) => void) | null>(null);
  const openPhotoMenu = useCallback((id: number, x: number, y: number) => {
    const st = useStore.getState();
    if (!st.selection.ids.has(id)) st.setSelection({ ids: new Set([id]), focus: id, anchor: id });
    setPhotoMenu({ x, y });
  }, []);
  // Ações reais (as mesmas dos atalhos), com o atalho visível à direita.
  const photoMenuEntries = (): MenuEntry[] => {
    const st = useStore.getState();
    const n = targets(st.selection).length;
    const focus = st.selection.focus;
    const ph = focus !== null ? st.photos[st.index.get(focus) ?? -1] : undefined;
    const act = (a: Action) => () => handleActionRef.current?.(a);
    return [
      { label: t("menu.open"), icon: "eye", onSelect: act({ type: "open" }), shortcut: "↵" },
      { label: t("view.develop"), icon: "sliders", onSelect: act({ type: "develop" }), shortcut: "D" },
      { label: t("compare.open"), icon: "compare", onSelect: act({ type: "compare" }), shortcut: "C" },
      { kind: "separator" },
      { label: t("menu.pick"), icon: "heart", onSelect: act({ type: "flag", value: 1 }), shortcut: "P" },
      { label: t("menu.reject"), icon: "reject", onSelect: act({ type: "flag", value: -1 }), shortcut: "X" },
      { label: t("menu.unflag"), onSelect: act({ type: "flag", value: 0 }), shortcut: "U" },
      { kind: "separator" },
      ...[5, 4, 3, 2, 1].map((v) => ({ label: "★".repeat(v), onSelect: act({ type: "stars", value: v }), shortcut: String(v) })),
      { label: t("menu.noStars"), onSelect: act({ type: "stars", value: 0 }), shortcut: "0" },
      { kind: "separator" },
      { label: t("menu.copyEdit"), icon: "copy", disabled: focus === null, onSelect: act({ type: "copyEdit" }), shortcut: shortcutLabel("Mod+Shift+C") },
      { label: t("menu.pasteEdit", { count: n }), icon: "paste", disabled: !st.clipboard || n === 0, onSelect: act({ type: "pasteEdit" }), shortcut: shortcutLabel("Mod+Shift+V") },
      { kind: "separator" },
      { label: t("safety.reveal"), icon: "folder", disabled: !ph || (ph.availability ?? "ok") !== "ok" || n !== 1, onSelect: () => focus !== null && api.revealOriginal(focus).catch(fail) },
      { label: t("menu.remove", { count: n }), icon: "trash", danger: true, onSelect: act({ type: "delete" }), shortcut: shortcutLabel("Delete") },
    ];
  };

  const handleAction = useCallback(
    (a: Action) => {
      if (a.type === "menu") {
        const st0 = useStore.getState();
        const f = st0.selection.focus;
        if (f === null) return;
        const r = document.querySelector(`[data-id="${f}"]`)?.getBoundingClientRect();
        setPhotoMenu(r ? { x: r.left + r.width / 2, y: r.top + r.height / 2 } : { x: window.innerWidth / 2, y: window.innerHeight / 2 });
        return;
      }
            const st = useStore.getState();
      // No editor: classificar, navegar e sair; o resto dos atalhos é do próprio editor.
      if (st.view === "develop") {
        // A fotografia MOSTRADA no editor (a mesma regra de `developKey`): se a seleção
        // ficou fora do filtro, as teclas classificavam uma fotografia escondida (Fase 15).
        const f = st.selection.focus;
        const cur = f !== null && order.includes(f) ? f : order[0];
        switch (a.type) {
          case "stars":
          case "flag":
            return cur !== undefined ? rate(a.type, a.value, [cur]) : undefined;
          case "move": {
            if (a.dx === 0 || cur === undefined) return;
            const i = order.indexOf(cur) + a.dx;
            if (i >= 0 && i < order.length) st.setSelection({ ids: new Set([order[i]!]), focus: order[i]!, anchor: order[i]! });
            return;
          }
          case "close":
          case "develop":
            return st.setView("grid");
          case "help":
            return st.setHelpOpen(true);
          default:
            return;
        }
      }
      // Na comparação: teclas aplicam-se ao painel ativo; ← → mudam de painel.
      if (st.compare) {
        switch (a.type) {
          case "stars":
          case "flag":
            return rate(a.type, a.value);
          case "move":
            if (a.dx !== 0) setCompareActive((i) => Math.max(0, Math.min(st.compare!.length - 1, i + a.dx)));
            return;
          case "close":
            return st.openCompare(null);
          case "toggleZoom":
            return compareRef.current?.toggleZoom();
          case "zoom":
            return compareRef.current?.zoom(a.dir);
          case "compare":
            return st.openCompare(null);
          case "help":
            return st.setHelpOpen(true);
          default:
            return;
        }
      }
      // Pessoas, sequências e consistência mostram outras coisas que não a grelha: teclas
      // que agem sobre a seleção da grelha (invisível aqui) não fazem nada (Fase 15).
      if ((st.view === "people" || st.view === "sequences" || st.view === "consistency") && st.viewerId === null) {
        if (["stars", "flag", "color", "selectAll", "delete", "move", "open", "toggleZoom", "menu"].includes(a.type)) return;
      }
      switch (a.type) {
        case "develop":
          if (st.selection.focus === null && order.length > 0) st.setSelection({ ids: new Set([order[0]!]), focus: order[0]!, anchor: order[0]! });
          st.openViewer(null);
          return st.setView("develop");
        case "compare":
          return compareFromSelection();
        case "stars":
          return rate("stars", a.value);
        case "flag":
          return rate("flag", a.value);
        case "color": {
          const cur = st.viewerId !== null ? st.photos[st.index.get(st.viewerId)!] : focusPhoto;
          return rate("color", cur?.color === a.value ? null : a.value);
        }
        case "move":
          if (st.viewerId !== null) {
            if (a.dx !== 0) {
              const i = order.indexOf(st.viewerId) + a.dx;
              if (i >= 0 && i < order.length) st.openViewer(order[i]!);
            }
            return;
          }
          return st.setSelection(moveFocus(st.selection, order, a.dx + a.dy * layoutRef.current.columns, a.extend));
        case "open":
          if (st.viewerId === null && st.selection.focus !== null) st.openViewer(st.selection.focus);
          else if (st.viewerId === null && order.length > 0) st.openViewer(order[0]!);
          return;
        case "close":
          if (fullscreen) return void toggleFullscreen();
          if (st.viewerId !== null) st.openViewer(null);
          return;
        case "toggleZoom":
          if (st.viewerId !== null) viewerRef.current?.toggleZoom();
          else if (st.selection.focus !== null) st.openViewer(st.selection.focus);
          return;
        case "zoom":
          if (st.viewerId !== null) viewerRef.current?.zoom(a.dir);
          else st.setThumbSize(st.thumbSize + a.dir * 20);
          return;
        case "fullscreen":
          return void toggleFullscreen();
        case "info":
          return st.toggleInfo();
        case "selectAll":
          return st.setSelection(selectAll(order, st.selection.focus));
        case "import":
          return void chooseImport(false);
        case "help":
          return st.setHelpOpen(true);
        case "export":
          return st.setExportDialog(true);
        case "delete": {
          const keys = targets(st.selection);
          if (keys.length > 0 && (st.view === "grid" || st.view === "sequences")) st.setDeleteRequest(keys);
          return;
        }
        case "copyEdit": {
          // Copia a edição da fotografia em foco (sem recorte/rotação, máscaras nem retoque,
          // como no editor: são próprios de cada fotografia).
          const f = st.viewerId ?? st.selection.focus;
          if (f === null) return;
          api
            .editView(f)
            .then((v) => {
              st.setClipboard({ settings: cloneSettings(v.settings), blocks: DEFAULT_COPY_BLOCKS });
              st.toast("info", t("develop.copied"));
            })
            .catch(fail);
          return;
        }
        case "pasteEdit": {
          const clip = st.clipboard;
          if (!clip) return st.toast("info", t("develop.clipboardEmpty"));
          const keys = st.viewerId !== null ? [st.viewerId] : targets(st.selection);
          if (keys.length === 0) return;
          api
            .pasteSettings(keys, clip.settings, clip.blocks)
            .then((n) => {
              st.toast("info", t("develop.pasted", { count: n }));
              return st.reloadPhotos();
            })
            .catch(fail);
          return;
        }
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [order, rate, focusPhoto, fullscreen, toggleFullscreen, compareFromSelection],
  );
  handleActionRef.current = handleAction;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const st = useStore.getState();
      if (isTyping(e.target) || st.pendingImport || st.importSummary || st.helpOpen || st.clockOpen || st.autoDialog || st.presetDialog || st.exportDialog || st.galleryDialog || st.safetyDialog || st.deleteRequest) return;
      const a = keyToAction(e, mac);
      if (!a) return;
      // No álbum, as teclas são do editor: nada de classificar ou mover a seleção da
      // grelha (invisível) por engano.
      if (st.view === "album" && a.type !== "help" && a.type !== "develop") return;
      e.preventDefault();
      handleAction(a);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [handleAction, mac]);

    const immersive = (fullscreen && viewerPhoto !== null) || s.view === "develop" || s.view === "album";
  const developKey = s.view === "develop" ? (s.selection.focus !== null && order.includes(s.selection.focus) ? s.selection.focus : order[0]) : undefined;

  return (
    <div className={`project${immersive ? " immersive" : ""}${s.infoOpen ? "" : " no-info"}`}>
      {!immersive && (
        <Sidebar
          active={s.filter.collection}
          counts={counts}
          onSelect={(c) => {
            s.setFilter({ collection: c, label: null });
            s.setView("grid");
          }}
          labelCounts={lCounts}
          activeLabel={s.filter.label ?? null}
          onLabel={(l: Label | null) => {
            s.setFilter({ label: l });
            s.setView("grid");
          }}
                    view={s.view}
          onView={(v) => s.setView(v)}
          activeMoment={s.filter.moment ?? null}
          onMoment={(m) => {
            s.setFilter({ moment: m });
            s.setView("grid");
          }}
          onDropMoment={(ids, m) => void assignMoment(ids, m)}
          onGallery={() => s.setGalleryDialog(true)}
          onSafety={() => s.setSafetyDialog(true)}
          activePersons={s.filter.persons ?? []}
          onPersons={(ids) => {
            s.setFilter({ persons: ids });
            s.setView("grid");
          }}
        />
      )}
      <section className="main">
                {(!immersive || s.view === "develop" || s.view === "album") && (
          <Toolbar
            project={project}
            importBusy={s.importRunning}
            onImportFiles={() => void chooseImport(false)}
            onImportFolder={() => void chooseImport(true)}
            onClose={() => void closeProject()}
            onHelp={() => s.setHelpOpen(true)}
            onCompare={compareFromSelection}
            onAutoEdit={() => s.setAutoDialog(true)}
            onPresets={() => s.setPresetDialog(true)}
            onExport={() => s.setExportDialog(true)}
            selectedCount={s.selection.ids.size}
            view={s.view}
            onView={(v) => s.setView(v)}
          />
        )}
        {!fullscreen && (
          <WorkflowBar view={s.view} onView={(v) => s.setView(v)} onImport={() => void chooseImport(true)} onExport={() => s.setExportDialog(true)} />
        )}
        <div className="content">
                    {s.view === "develop" && developKey !== undefined ? (
            <DevelopView
              photoKey={developKey}
              order={order}
              onNavigate={(k) => s.setSelection({ ids: new Set([k]), focus: k, anchor: k })}
              filmstrip={
                <Filmstrip
                  photos={visible}
                  current={developKey}
                  selected={s.selection.ids}
                  onSelect={(k, how) => {
                    const sel = s.selection;
                    // Cmd/Ctrl e Shift juntam à seleção sem mudar a fotografia mostrada (para
                    // editar várias ao mesmo tempo com a sincronização automática).
                    if (how.toggle || how.range) {
                      const ids = new Set(sel.ids);
                      if (developKey !== undefined) ids.add(developKey);
                      if (how.range) {
                        const a = order.indexOf(sel.anchor ?? developKey ?? k);
                        const b = order.indexOf(k);
                        for (const x of order.slice(Math.min(a, b), Math.max(a, b) + 1)) ids.add(x);
                      } else if (ids.has(k) && k !== developKey) ids.delete(k);
                      else ids.add(k);
                      s.setSelection({ ids, focus: developKey ?? k, anchor: how.range ? sel.anchor : k });
                    } else {
                      s.setSelection({ ids: new Set([k]), focus: k, anchor: k });
                    }
                  }}
                  onMenu={openPhotoMenu}
                />
              }
            />
          ) : s.photos.length === 0 ? (
            <div className="empty-state">
              <span className="empty-icon" aria-hidden="true"><Icon name="import" size={32} /></span>
              <h2>{t("project.emptyTitle")}</h2>
              <p className="muted">{t("project.emptyHint")}</p>
              <div className="home-actions">
                <button type="button" className="btn primary" disabled={s.importRunning} onClick={() => void chooseImport(true)}>
                  {t("import.folder")}
                </button>
                <button type="button" className="btn" disabled={s.importRunning} onClick={() => void chooseImport(false)}>
                  {t("import.files")}
                </button>
              </div>
            </div>
                    ) : s.view === "album" ? (
            <AlbumView />
          ) : s.view === "consistency" ? (
            <ConsistencyView
              onEdit={(k) => {
                s.setSelection({ ids: new Set([k]), focus: k, anchor: k });
                s.setView("develop");
              }}
            />
          ) : s.view === "people" ? (
            <PeopleView
              onShowPerson={(id) => {
                s.setFilter({ persons: [id] });
                s.setView("grid");
              }}
            />
          ) : s.view === "sequences" ? (
            <SequencesView onSelect={onClickPhoto} onOpen={(id) => s.openViewer(id)} onCompare={openCompare} onAcceptGroup={(g) => void acceptGroup(g)} />
          ) : visible.length === 0 ? (
            <div className="empty-state">
              <span className="empty-icon" aria-hidden="true"><Icon name="search" size={32} /></span>
              <p className="muted">{t("project.noMatches")}</p>
            </div>
          ) : (
            <PhotoGrid photos={visible} onClickPhoto={onClickPhoto} onOpen={(id) => s.openViewer(id)} onMenu={openPhotoMenu} layoutRef={layoutRef} />
          )}
          {viewerPhoto && (
            <Viewer
              photo={viewerPhoto}
              token={project.imageToken}
              index={viewerIndex}
              total={order.length}
              onPrev={() => viewerIndex > 0 && s.openViewer(order[viewerIndex - 1]!)}
              onNext={() => viewerIndex < order.length - 1 && s.openViewer(order[viewerIndex + 1]!)}
              onClose={() => s.openViewer(null)}
              onFullscreen={() => void toggleFullscreen()}
              handleRef={viewerRef}
              filmstrip={fullscreen ? null : <Filmstrip photos={visible} current={viewerPhoto.id} onSelect={(id) => s.openViewer(id)} onMenu={openPhotoMenu} />}
            />
          )}
          {s.compare && (
            <CompareView
              ids={s.compare}
              active={compareActive}
              setActive={setCompareActive}
              onClose={() => s.openCompare(null)}
              onRate={(k, v, ids) => rate(k, v, ids)}
              onReplace={(slot, id) => {
                const next = s.compare!.slice();
                const existing = next.indexOf(id);
                if (existing >= 0) next[existing] = next[slot]!;
                next[slot] = id;
                s.openCompare(next);
              }}
              handleRef={compareRef}
            />
          )}
          {dragging && <div className="drop-overlay">{t("import.dropHere")}</div>}
        </div>
        {!immersive && <StatusBar total={s.photos.length} visible={visible.length} selected={s.selection.ids.size} onCancelImport={() => api.cancelImport().catch(fail)} showSize={viewerPhoto === null} />}
      </section>
      {s.infoOpen && !immersive && (
        <InfoPanel
          photo={s.compare ? (s.photos[s.index.get(s.compare[compareActive] ?? -1) ?? -1] ?? null) : (viewerPhoto ?? focusPhoto)}
          selectedCount={viewerPhoto || s.compare ? 1 : s.selection.ids.size}
          onRate={(k, v) => rate(k, v)}
                    onCompareGroup={openCompare}
          onAccept={(id) => void acceptIds([id])}
          onMoment={(ids, m) => void assignMoment(viewerPhoto || s.compare ? ids : targets(s.selection), m)}
          onResetMoment={(ids) => void resetMoments(viewerPhoto || s.compare ? ids : targets(s.selection))}
          onPerson={(id) => {
            s.setFilter({ persons: [id] });
            s.setView("grid");
          }}
        />
      )}

      {s.pendingImport && <ImportDialog paths={s.pendingImport.paths} originalsDir={project.originalsDir} onCancel={() => s.setPendingImport(null)} onStart={(m, r, sf) => void startImport(m, r, sf)} />}
      {s.importSummary && <ImportReport summary={s.importSummary} onClose={() => s.setImportSummary(null)} />}
            {s.helpOpen && <HelpOverlay onClose={() => s.setHelpOpen(false)} />}
      {s.clockOpen && <ClockDialog onClose={() => s.setClockOpen(false)} onApplied={() => void reloadAfterClock()} />}
      {s.autoDialog && <AutoEditDialog keys={order.filter((k) => s.selection.ids.has(k))} onClose={() => s.setAutoDialog(false)} />}
      {s.presetDialog && <ApplyPresetDialog onClose={() => s.setPresetDialog(false)} />}
      <AutoEditQueue />
      {s.exportDialog && <ExportDialog selected={order.filter((k) => s.selection.ids.has(k))} visible={order} onClose={() => s.setExportDialog(false)} />}
      {s.safetyDialog && (
        <SafetyDialog
          onClose={() => s.setSafetyDialog(false)}
          onReopened={(p) => {
            s.setProject(p);
            void s.reloadPhotos();
          }}
          onDeleted={() => {
            s.setSafetyDialog(false);
            s.setProject(null);
            onClosed();
          }}
        />
      )}
      {photoMenu && <Menu x={photoMenu.x} y={photoMenu.y} label={t("menu.photo")} entries={photoMenuEntries()} onClose={() => setPhotoMenu(null)} />}
      {s.deleteRequest && <DeleteDialog keys={s.deleteRequest} onClose={() => s.setDeleteRequest(null)} />}
      {s.galleryDialog && <GalleryDialog selected={order.filter((k) => s.selection.ids.has(k))} focus={s.selection.focus} onClose={() => s.setGalleryDialog(false)} />}
      <ExportQueue />
      {s.dragIds && <div className="drag-badge">{t("moments.dropHint", { count: s.dragIds.length })}</div>}
    </div>
  );
}
