import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import { allowedSpaces, examplePath, fitToFormat, formatDuration, isActive, jobFraction, newRecipe } from "../logic/exportRecipe";
import { formatBytes } from "../logic/format";
import { errorText, useStore } from "../store";
import { ANCHORS, EXPORT_SPACES, SIZE_MODES, type ExportCaps, type ExportEstimate, type ExportPlan, type ExportProgress, type ExportRecipe } from "../types";
import { Modal } from "./Modal";
import { momentName } from "./MomentsPanel";

const DEST_KEY = "momento.exportDestination";

function loadDest(): string {
  try {
    return localStorage.getItem(DEST_KEY) ?? "";
  } catch {
    return "";
  }
}

/** Resumo curto de uma receita: "JPEG · 2560 px · sRGB". */
function summary(r: ExportRecipe, t: (k: string, o?: Record<string, unknown>) => string): string {
  const size =
    r.size.mode === "original"
      ? t("export.size.original")
      : r.size.mode === "fit" || r.size.mode === "fill"
        ? `${r.size.a}×${r.size.b} ${r.size.unit}`
        : r.size.mode === "megapixels"
          ? `${r.size.a} MP`
          : `${r.size.a} ${r.size.unit}`;
  return [t(`export.format.${r.format}`), `${r.depth.replace("f", "")} ${t(r.depth.endsWith("f") ? "export.bitsFloat" : "export.bits")}`, size, t(`export.space.${r.color}`)].join(" · ");
}

/** Diálogo "Exportar": fotografias, plano (várias receitas), editor da receita, destino e estimativa. */
export function ExportDialog({ selected, visible, onClose }: { selected: number[]; visible: number[]; onClose: () => void }) {
  const { t } = useTranslation();
  const toast = useStore((s) => s.toast);
  const project = useStore((s) => s.project);
  const moments = useStore((s) => s.moments);
  const upsert = useStore((s) => s.upsertExportJob);
  const [caps, setCaps] = useState<ExportCaps[]>([]);
  const [library, setLibrary] = useState<ExportRecipe[]>([]);
  const [plans, setPlans] = useState<ExportPlan[]>([]);
  const [planName, setPlanName] = useState("");
  const [recipes, setRecipes] = useState<ExportRecipe[]>([]);
  const [current, setCurrent] = useState(0);
  const [scope, setScope] = useState<"selected" | "visible">(selected.length > 0 ? "selected" : "visible");
  const [dest, setDest] = useState(loadDest);
  const [estimate, setEstimate] = useState<ExportEstimate | null>(null);
  const [busy, setBusy] = useState(false);
  const [iccInfo, setIccInfo] = useState<Record<string, string>>({});
  const fail = (e: unknown) => toast("error", errorText(e, t));

  useEffect(() => {
    Promise.all([api.exportCapabilities(), api.exportRecipes(), api.exportPlans()])
      .then(([c, l, p]) => {
        setCaps(c);
        setLibrary(l);
        setPlans(p);
        const first = p[0];
        if (first) {
          setPlanName(first.name);
          setRecipes(first.recipes);
        }
      })
      .catch(fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const keys = scope === "selected" ? selected : visible;
  const momentNames = useMemo(() => Object.fromEntries(moments.map((m) => [m.id, momentName(m, t)])), [moments, t]);
  const request = useMemo(() => ({ keys, recipes, destination: dest, name: planName || t("export.title"), momentNames }), [keys, recipes, dest, planName, momentNames, t]);

  // Estimativa ao vivo (tamanho, espaço livre, ajustes às receitas).
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => {
    window.clearTimeout(timer.current);
    if (recipes.length === 0 || keys.length === 0) {
      setEstimate(null);
      return;
    }
    timer.current = window.setTimeout(() => {
      api.estimateExport(request).then(setEstimate).catch(() => setEstimate(null));
    }, 350);
    return () => window.clearTimeout(timer.current);
  }, [request, recipes.length, keys.length]);

  const capsOf = (f: string) => caps.find((c) => c.format === f);
  const r = recipes[current];
  const set = (patch: Partial<ExportRecipe>) => setRecipes((all) => all.map((x, i) => (i === current ? { ...x, ...patch } : x)));

  const pickDest = async () => {
    const dir = await open({ directory: true, multiple: false, title: t("export.destination") });
    if (typeof dir === "string") {
      setDest(dir);
      try {
        localStorage.setItem(DEST_KEY, dir);
      } catch {
        /* armazenamento indisponível */
      }
    }
  };

  const choosePlan = (name: string) => {
    const p = plans.find((x) => x.name === name);
    setPlanName(name);
    if (p) {
      setRecipes(p.recipes);
      setCurrent(0);
    }
  };

  const addRecipe = (id: string) => {
    const base = id === "__new" ? newRecipe(t("export.newRecipeName")) : library.find((x) => x.id === id);
    if (!base) return;
    setRecipes((all) => [...all, { ...base }]);
    setCurrent(recipes.length);
  };

  const removeRecipe = (i: number) => {
    setRecipes((all) => all.filter((_, j) => j !== i));
    setCurrent((c) => Math.max(0, c >= i ? c - 1 : c));
  };

  const saveRecipe = async () => {
    if (!r) return;
    try {
      const saved = await api.saveExportRecipe(r);
      set(saved);
      setLibrary(await api.exportRecipes());
      toast("info", t("export.recipeSaved", { name: saved.name }));
    } catch (e) {
      fail(e);
    }
  };

  const savePlan = async () => {
    const name = planName.trim() || t("export.title");
    try {
      const existing = plans.find((p) => p.name === name && !p.id.startsWith("builtin:"));
      const saved = await api.saveExportPlan({ id: existing?.id ?? "", name, recipes });
      setPlans(await api.exportPlans());
      setPlanName(saved.name);
      toast("info", t("export.planSaved", { name: saved.name }));
    } catch (e) {
      fail(e);
    }
  };

  const pickIcc = async () => {
    const path = await open({ multiple: false, filters: [{ name: t("export.iccFilter"), extensions: ["icc", "icm"] }] });
    if (!path || Array.isArray(path)) return;
    try {
      const desc = await api.inspectIccProfile(path);
      setIccInfo((m) => ({ ...m, [path]: desc }));
      set({ iccPath: path, color: "icc" });
    } catch (e) {
      fail(e);
    }
  };

  const pickWatermarkImage = async () => {
    const path = await open({ multiple: false, filters: [{ name: t("export.watermark.imageFilter"), extensions: ["png", "jpg", "jpeg"] }] });
    if (typeof path === "string" && r) set({ watermark: { ...r.watermark, imagePath: path } });
  };

  const start = async () => {
    setBusy(true);
    try {
      const id = await api.startExport(request);
      const jobs = await api.exportJobs();
      const job = jobs.find((j) => j.id === id);
      if (job) upsert(job);
      toast("info", t("export.started", { count: estimate?.files ?? keys.length * recipes.length }));
      onClose();
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const noSpace = estimate != null && !estimate.enoughSpace;
  const canStart = keys.length > 0 && recipes.length > 0 && dest !== "" && !busy && (estimate?.errors.length ?? 0) === 0;
  const vars = {
    stem: "IMG_0001",
    seq: r?.naming.seqStart ?? 1,
    date: "2026-06-15",
    time: "174205",
    project: project?.name ?? "",
    client: project?.clientName ?? "",
    recipe: r?.name ?? "",
    moment: moments[0] ? momentName(moments[0], t) : "",
    width: 2048,
    height: 1365,
  };

  return (
    <Modal
      title={t("export.title")}
      onClose={onClose}
      wide
      footer={
        <>
          <span className="export-summary small" role="status">
            {estimate
              ? t("export.estimate", {
                  files: estimate.files,
                  size: formatBytes(estimate.bytes),
                  free: estimate.freeBytes != null ? formatBytes(estimate.freeBytes) : "—",
                })
              : keys.length === 0
                ? t("export.noPhotos")
                : ""}
            {noSpace && <strong className="warn"> · {t("export.noSpace")}</strong>}
          </span>
          <button type="button" className="btn" onClick={onClose} data-close>
            {t("common.cancel")}
          </button>
          <button type="button" className="btn primary" disabled={!canStart} onClick={() => void start()}>
            {t("export.start", { count: estimate?.files ?? keys.length * recipes.length })}
          </button>
        </>
      }
    >
      <div className="export-layout">
        <aside className="export-side">
          <fieldset className="export-scope">
            <legend>{t("export.photos")}</legend>
            <label>
              <input type="radio" name="scope" checked={scope === "selected"} disabled={selected.length === 0} onChange={() => setScope("selected")} /> {t("export.scopeSelected", { count: selected.length })}
            </label>
            <label>
              <input type="radio" name="scope" checked={scope === "visible"} onChange={() => setScope("visible")} /> {t("export.scopeVisible", { count: visible.length })}
            </label>
          </fieldset>
          <label className="field compact">
            <span>{t("export.plan")}</span>
            <select value={plans.some((p) => p.name === planName) ? planName : ""} onChange={(e) => choosePlan(e.target.value)} aria-label={t("export.plan")}>
              <option value="" disabled>
                {t("export.customPlan")}
              </option>
              {plans.map((p) => (
                <option key={p.id} value={p.name}>
                  {p.name}
                </option>
              ))}
            </select>
          </label>
          <ul className="export-recipes" aria-label={t("export.recipes")}>
            {recipes.map((x, i) => (
              <li key={i} className={i === current ? "active" : ""}>
                <button type="button" className="export-recipe" aria-current={i === current} onClick={() => setCurrent(i)}>
                  <strong>{x.name}</strong>
                  <span className="small muted">{summary(x, t)}</span>
                  {x.naming.subfolder && <span className="small muted">/{x.naming.subfolder}</span>}
                </button>
                <button type="button" className="btn ghost small" aria-label={t("export.removeRecipe", { name: x.name })} title={t("export.removeRecipe", { name: x.name })} onClick={() => removeRecipe(i)}>
                  ✕
                </button>
              </li>
            ))}
          </ul>
          <label className="field compact">
            <span>{t("export.addRecipe")}</span>
            <select value="" onChange={(e) => addRecipe(e.target.value)} aria-label={t("export.addRecipe")}>
              <option value="">{t("export.addRecipeChoose")}</option>
              <option value="__new">{t("export.newRecipe")}</option>
              {library.map((x) => (
                <option key={x.id} value={x.id} disabled={!capsOf(x.format)?.available}>
                  {x.name}
                  {!capsOf(x.format)?.available ? ` — ${t("export.unavailableShort")}` : ""}
                </option>
              ))}
            </select>
          </label>
          <div className="dev-row">
            <input className="grow" value={planName} onChange={(e) => setPlanName(e.target.value)} aria-label={t("export.planName")} placeholder={t("export.planName")} />
            <button type="button" className="btn small" disabled={recipes.length === 0} onClick={() => void savePlan()}>
              {t("export.savePlan")}
            </button>
          </div>
          <label className="field compact">
            <span>{t("export.destination")}</span>
            <div className="dev-row">
              <input className="grow" readOnly value={dest} placeholder={t("export.destinationNone")} aria-label={t("export.destination")} />
              <button type="button" className="btn small" onClick={() => void pickDest()}>
                {t("export.choose")}
              </button>
            </div>
          </label>
          {estimate && (estimate.notes.length > 0 || estimate.errors.length > 0) && (
            <ul className="export-notes small">
              {estimate.errors.map((n, i) => (
                <li key={`e${i}`} className="warn">
                  {n.recipe}: {n.code}
                </li>
              ))}
              {estimate.notes.map((n, i) => (
                <li key={`n${i}`}>
                  {n.recipe}: {t(n.code)}
                </li>
              ))}
            </ul>
          )}
        </aside>
        {r ? (
          <RecipeEditor
            r={r}
            caps={caps}
            set={set}
            onSave={() => void saveRecipe()}
            onPickIcc={() => void pickIcc()}
            iccDescription={iccInfo[r.iccPath]}
            onPickWatermark={() => void pickWatermarkImage()}
            example={examplePath(r, capsOf(r.format)?.extension ?? "jpg", vars)}
          />
        ) : (
          <p className="muted">{t("export.noRecipes")}</p>
        )}
      </div>
    </Modal>
  );
}

function Num({ label, value, onChange, min, max, step = 1 }: { label: string; value: number; onChange: (v: number) => void; min?: number; max?: number; step?: number }) {
  return (
    <label className="field compact">
      <span>{label}</span>
      <input type="number" value={value} min={min} max={max} step={step} onChange={(e) => onChange(Number(e.target.value))} aria-label={label} />
    </label>
  );
}

function RecipeEditor({
  r,
  caps,
  set,
  onSave,
  onPickIcc,
  iccDescription,
  onPickWatermark,
  example,
}: {
  r: ExportRecipe;
  caps: ExportCaps[];
  set: (p: Partial<ExportRecipe>) => void;
  onSave: () => void;
  onPickIcc: () => void;
  iccDescription?: string;
  onPickWatermark: () => void;
  example: string;
}) {
  const { t } = useTranslation();
  const c = caps.find((x) => x.format === r.format);
  const spaces = allowedSpaces(c, EXPORT_SPACES);
  const lossless = r.compression === "lossless";
  // JPEG 2000 sem perdas só a 8 bits (ver o motor: compatibilidade com o OpenJPEG).
  const depths = r.format === "jpeg2000" && lossless ? (["8"] as const) : (c?.depths ?? []);
  const twoValues = r.size.mode === "fit" || r.size.mode === "fill";
  const size = (p: Partial<ExportRecipe["size"]>) => set({ size: { ...r.size, ...p } });
  const wm = (p: Partial<ExportRecipe["watermark"]>) => set({ watermark: { ...r.watermark, ...p } });
  const md = (p: Partial<ExportRecipe["metadata"]>) => set({ metadata: { ...r.metadata, ...p } });
  const nm = (p: Partial<ExportRecipe["naming"]>) => set({ naming: { ...r.naming, ...p } });
  return (
    <section className="export-editor" aria-label={t("export.recipe")}>
      <div className="dev-row">
        <label className="field compact grow">
          <span>{t("export.recipeName")}</span>
          <input value={r.name} onChange={(e) => set({ name: e.target.value })} aria-label={t("export.recipeName")} />
        </label>
        <button type="button" className="btn small" onClick={onSave} title={t("export.saveRecipeHint")}>
          {t("export.saveRecipe")}
        </button>
      </div>

      <h4>{t("export.section.file")}</h4>
      <div className="export-grid">
        <label className="field compact">
          <span>{t("export.formatLabel")}</span>
          <select
            value={r.format}
            aria-label={t("export.formatLabel")}
            onChange={(e) => {
              const nc = caps.find((x) => x.format === e.target.value);
              if (nc) set(fitToFormat(r, nc, EXPORT_SPACES));
            }}
          >
            {caps.map((x) => (
              <option key={x.format} value={x.format} disabled={!x.available}>
                {t(`export.format.${x.format}`)}
                {!x.available ? ` — ${t("export.unavailableShort")}` : ""}
              </option>
            ))}
          </select>
        </label>
        <label className="field compact">
          <span>{t("export.depth")}</span>
          <select value={r.depth} onChange={(e) => set({ depth: e.target.value as ExportRecipe["depth"] })} aria-label={t("export.depth")}>
            {depths.map((d) => (
              <option key={d} value={d}>
                {t(`export.depthOption.${d}`)}
              </option>
            ))}
          </select>
        </label>
        {(c?.compressions.length ?? 0) > 0 && (
          <label className="field compact">
            <span>{t("export.compression")}</span>
            <select
              value={r.compression}
              onChange={(e) => {
                const compression = e.target.value as ExportRecipe["compression"];
                set(r.format === "jpeg2000" && compression === "lossless" ? { compression, depth: "8" } : { compression });
              }}
              aria-label={t("export.compression")}
            >
              {c?.compressions.map((x) => (
                <option key={x} value={x}>
                  {t(`export.compressionOption.${x}`)}
                </option>
              ))}
            </select>
          </label>
        )}
        {c?.quality && !lossless && (
          <label className="field compact">
            <span>
              {t("export.quality")}: {r.quality}
            </span>
            <input type="range" min={1} max={100} value={r.quality} onChange={(e) => set({ quality: Number(e.target.value) })} aria-label={t("export.quality")} />
          </label>
        )}
      </div>
      {!c?.available && c?.reason && <p className="small warn">{t(c.reason)}</p>}
      {c && !c.metadata && <p className="small muted">{t("export.noMetadataFormat")}</p>}

      <h4>{t("export.section.color")}</h4>
      <div className="export-grid">
        <label className="field compact">
          <span>{t("export.colorSpace")}</span>
          <select value={r.color} onChange={(e) => (e.target.value === "icc" ? onPickIcc() : set({ color: e.target.value as ExportRecipe["color"] }))} aria-label={t("export.colorSpace")}>
            {spaces.map((s) => (
              <option key={s} value={s}>
                {t(`export.space.${s}`)}
              </option>
            ))}
          </select>
        </label>
        {r.color === "icc" && (
          <>
            <label className="field compact">
              <span>{t("export.intent")}</span>
              <select value={r.intent} onChange={(e) => set({ intent: e.target.value as ExportRecipe["intent"] })} aria-label={t("export.intent")}>
                <option value="perceptual">{t("export.intentPerceptual")}</option>
                <option value="relativeColorimetric">{t("export.intentRelative")}</option>
              </select>
            </label>
            <p className="small span2">
              {iccDescription ?? r.iccPath}{" "}
              <button type="button" className="btn small" onClick={onPickIcc}>
                {t("export.chooseIcc")}
              </button>
            </p>
          </>
        )}
      </div>
      {c?.linear && <p className="small muted">{t("export.linearNote")}</p>}

      <h4>{t("export.section.size")}</h4>
      <div className="export-grid">
        <label className="field compact">
          <span>{t("export.sizeMode")}</span>
          <select value={r.size.mode} onChange={(e) => size({ mode: e.target.value as ExportRecipe["size"]["mode"] })} aria-label={t("export.sizeMode")}>
            {SIZE_MODES.map((m) => (
              <option key={m} value={m}>
                {t(`export.size.${m}`)}
              </option>
            ))}
          </select>
        </label>
        {r.size.mode !== "original" && (
          <>
            <Num label={r.size.mode === "megapixels" ? t("export.size.mp") : twoValues ? t("export.size.w") : t("export.size.value")} value={r.size.a} min={0.01} step={r.size.mode === "megapixels" ? 0.5 : 1} onChange={(v) => size({ a: v })} />
            {twoValues && <Num label={t("export.size.h")} value={r.size.b} min={1} onChange={(v) => size({ b: v })} />}
            {r.size.mode !== "megapixels" && (
              <label className="field compact">
                <span>{t("export.unit")}</span>
                <select value={r.size.unit} onChange={(e) => size({ unit: e.target.value as ExportRecipe["size"]["unit"] })} aria-label={t("export.unit")}>
                  <option value="px">px</option>
                  <option value="cm">cm</option>
                  <option value="in">in</option>
                </select>
              </label>
            )}
          </>
        )}
        <Num label={t("export.ppi")} value={r.size.ppi} min={1} max={4800} onChange={(v) => size({ ppi: v })} />
        <label className="check">
          <input type="checkbox" checked={r.size.noEnlarge} onChange={(e) => size({ noEnlarge: e.target.checked })} /> {t("export.noEnlarge")}
        </label>
        {twoValues && (
          <label className="check">
            <input type="checkbox" checked={r.size.matchOrientation} onChange={(e) => size({ matchOrientation: e.target.checked })} /> {t("export.matchOrientation")}
          </label>
        )}
      </div>
      {r.size.mode === "fill" && <p className="small muted">{t("export.fillNote")}</p>}

      <h4>{t("export.section.sharpen")}</h4>
      <div className="export-grid">
        <label className="field compact">
          <span>{t("export.sharpenFor")}</span>
          <select value={r.sharpen.target} onChange={(e) => set({ sharpen: { ...r.sharpen, target: e.target.value as ExportRecipe["sharpen"]["target"] } })} aria-label={t("export.sharpenFor")}>
            {(["none", "screen", "matte", "glossy"] as const).map((x) => (
              <option key={x} value={x}>
                {t(`export.sharpen.${x}`)}
              </option>
            ))}
          </select>
        </label>
        {r.sharpen.target !== "none" && (
          <label className="field compact">
            <span>{t("export.sharpenAmount")}</span>
            <select value={r.sharpen.amount} onChange={(e) => set({ sharpen: { ...r.sharpen, amount: e.target.value as ExportRecipe["sharpen"]["amount"] } })} aria-label={t("export.sharpenAmount")}>
              {(["low", "standard", "high"] as const).map((x) => (
                <option key={x} value={x}>
                  {t(`export.sharpenLevel.${x}`)}
                </option>
              ))}
            </select>
          </label>
        )}
      </div>

      <h4>{t("export.section.metadata")}</h4>
      <div className="export-grid">
        <label className="field compact">
          <span>{t("export.metadataMode")}</span>
          <select value={r.metadata.mode} onChange={(e) => md({ mode: e.target.value as ExportRecipe["metadata"]["mode"] })} aria-label={t("export.metadataMode")}>
            {(["all", "copyright", "none"] as const).map((x) => (
              <option key={x} value={x}>
                {t(`export.metadata.${x}`)}
              </option>
            ))}
          </select>
        </label>
        {r.metadata.mode === "all" && (
          <label className="check">
            <input type="checkbox" checked={r.metadata.removeGps} onChange={(e) => md({ removeGps: e.target.checked })} /> {t("export.removeGps")}
          </label>
        )}
        {r.metadata.mode !== "none" && (
          <>
            <label className="field compact">
              <span>{t("export.copyright")}</span>
              <input value={r.metadata.copyright} placeholder="© 2026 …" onChange={(e) => md({ copyright: e.target.value })} aria-label={t("export.copyright")} />
            </label>
            <label className="field compact">
              <span>{t("export.creator")}</span>
              <input value={r.metadata.creator} onChange={(e) => md({ creator: e.target.value })} aria-label={t("export.creator")} />
            </label>
            <label className="field compact">
              <span>{t("export.contact")}</span>
              <input value={r.metadata.contact} onChange={(e) => md({ contact: e.target.value })} aria-label={t("export.contact")} />
            </label>
          </>
        )}
      </div>

      <h4>{t("export.section.watermark")}</h4>
      <label className="check">
        <input type="checkbox" checked={r.watermark.enabled} onChange={(e) => wm({ enabled: e.target.checked })} /> {t("export.watermark.enable")}
      </label>
      {r.watermark.enabled && (
        <div className="export-grid">
          <label className="field compact">
            <span>{t("export.watermark.kind")}</span>
            <select value={r.watermark.kind} onChange={(e) => wm({ kind: e.target.value as "text" | "image" })} aria-label={t("export.watermark.kind")}>
              <option value="text">{t("export.watermark.text")}</option>
              <option value="image">{t("export.watermark.image")}</option>
            </select>
          </label>
          {r.watermark.kind === "text" ? (
            <>
              <label className="field compact">
                <span>{t("export.watermark.text")}</span>
                <input value={r.watermark.text} onChange={(e) => wm({ text: e.target.value })} aria-label={t("export.watermark.text")} />
              </label>
              <label className="field compact">
                <span>{t("export.watermark.color")}</span>
                <input type="color" value={r.watermark.color} onChange={(e) => wm({ color: e.target.value })} aria-label={t("export.watermark.color")} />
              </label>
            </>
          ) : (
            <p className="small span2">
              {r.watermark.imagePath || t("export.watermark.noImage")}{" "}
              <button type="button" className="btn small" onClick={onPickWatermark}>
                {t("export.choose")}
              </button>
            </p>
          )}
          <Num label={t("export.watermark.size")} value={r.watermark.size} min={0.5} max={50} step={0.5} onChange={(v) => wm({ size: v })} />
          <Num label={t("export.watermark.opacity")} value={r.watermark.opacity} min={0} max={100} onChange={(v) => wm({ opacity: v })} />
          <Num label={t("export.watermark.margin")} value={r.watermark.margin} min={0} max={40} step={0.5} onChange={(v) => wm({ margin: v })} />
          <div className="field compact">
            <span>{t("export.watermark.position")}</span>
            <div className="anchor-grid" role="radiogroup" aria-label={t("export.watermark.position")}>
              {ANCHORS.map((a) => (
                <button key={a} type="button" role="radio" aria-checked={r.watermark.anchor === a} aria-label={t(`export.anchor.${a}`)} title={t(`export.anchor.${a}`)} className={r.watermark.anchor === a ? "active" : ""} onClick={() => wm({ anchor: a })} />
              ))}
            </div>
          </div>
        </div>
      )}

      <h4>{t("export.section.naming")}</h4>
      <div className="export-grid">
        <label className="field compact span2">
          <span>{t("export.template")}</span>
          <input value={r.naming.template} onChange={(e) => nm({ template: e.target.value })} aria-label={t("export.template")} />
        </label>
        <label className="field compact">
          <span>{t("export.prefix")}</span>
          <input value={r.naming.prefix} onChange={(e) => nm({ prefix: e.target.value })} aria-label={t("export.prefix")} />
        </label>
        <label className="field compact">
          <span>{t("export.suffix")}</span>
          <input value={r.naming.suffix} onChange={(e) => nm({ suffix: e.target.value })} aria-label={t("export.suffix")} />
        </label>
        <Num label={t("export.seqStart")} value={r.naming.seqStart} min={0} onChange={(v) => nm({ seqStart: v })} />
        <Num label={t("export.seqDigits")} value={r.naming.seqDigits} min={1} max={9} onChange={(v) => nm({ seqDigits: v })} />
        <label className="field compact">
          <span>{t("export.subfolder")}</span>
          <input value={r.naming.subfolder} onChange={(e) => nm({ subfolder: e.target.value })} aria-label={t("export.subfolder")} />
        </label>
        <label className="field compact">
          <span>{t("export.conflict")}</span>
          <select value={r.naming.conflict} onChange={(e) => nm({ conflict: e.target.value as ExportRecipe["naming"]["conflict"] })} aria-label={t("export.conflict")}>
            {(["rename", "overwrite", "skip"] as const).map((x) => (
              <option key={x} value={x}>
                {t(`export.conflictOption.${x}`)}
              </option>
            ))}
          </select>
        </label>
      </div>
      <p className="small muted">{t("export.tokens")}</p>
      <p className="small">
        {t("export.example")}: <code data-testid="export-example">{example}</code>
      </p>
    </section>
  );
}

/** Fila de exportação (canto inferior direito): progresso, restantes, tamanho, espaço, tempo, erros. */
export function ExportQueue() {
  const { t } = useTranslation();
  const jobs = useStore((s) => s.exportJobs);
  const setJobs = useStore((s) => s.setExportJobs);
  const toast = useStore((s) => s.toast);
  const [open, setOpen] = useState<number | null>(null);
  if (jobs.length === 0) return null;
  const fail = (e: unknown) => toast("error", errorText(e, t));
  const clear = async () => {
    try {
      await api.clearFinishedExports();
      setJobs(jobs.filter(isActive));
    } catch (e) {
      fail(e);
    }
  };
  return (
    <section className="auto-queue export-queue" aria-label={t("export.queueTitle")}>
      <header>
        <strong>{t("export.queueTitle")}</strong>
        {jobs.some((j) => !isActive(j)) && (
          <button type="button" className="btn ghost small" onClick={() => void clear()}>
            {t("export.clearFinished")}
          </button>
        )}
      </header>
      {jobs.map((j) => (
        <JobRow key={j.id} j={j} expanded={open === j.id} onToggle={() => setOpen(open === j.id ? null : j.id)} fail={fail} />
      ))}
    </section>
  );
}

function JobRow({ j, expanded, onToggle, fail }: { j: ExportProgress; expanded: boolean; onToggle: () => void; fail: (e: unknown) => void }) {
  const { t } = useTranslation();
  const pct = Math.round(jobFraction(j) * 100);
  const processed = j.filesDone + j.filesFailed + j.filesSkipped;
  return (
    <div className="export-job">
      <div className="dev-row">
        <span className="grow ellipsis">{j.name}</span>
        <span className="muted small">{t(`export.state.${j.state}`)}</span>
      </div>
      <div className="progress" role="progressbar" aria-valuemin={0} aria-valuemax={j.filesTotal} aria-valuenow={processed} aria-label={j.name}>
        <div className="progress-bar" style={{ width: `${pct}%` }} />
      </div>
      <p className="small">
        {t("export.filesCount", { done: processed, total: j.filesTotal })} · {t("export.photosLeft", { count: j.photosTotal - j.photosDone })}
      </p>
      <p className="small muted">
        {t("export.sizeLine", { written: formatBytes(j.bytesWritten), total: formatBytes(j.bytesEstimated) })}
        {j.freeBytes != null && ` · ${t("export.freeLine", { free: formatBytes(j.freeBytes) })}`}
        {isActive(j) && j.etaSeconds != null && j.photosDone > 0 && ` · ${t("export.eta", { time: formatDuration(j.etaSeconds) })}`}
      </p>
      {j.pausedReason === "noSpace" && <p className="small warn">{t("export.pausedNoSpace")}</p>}
      {isActive(j) && j.current && <p className="small muted ellipsis">{j.current}</p>}
      {j.errors.length > 0 && (
        <button type="button" className="btn ghost small warn" aria-expanded={expanded} onClick={onToggle}>
          {t("export.errors", { count: j.errors.length })}
        </button>
      )}
      {expanded && (
        <ul className="export-errors small">
          {j.errors.map((e, i) => (
            <li key={i}>
              {e.file && <strong>{e.file}: </strong>}
              {e.message}
            </li>
          ))}
        </ul>
      )}
      <div className="dev-row">
        {isActive(j) && (
          <>
            {j.state === "paused" ? (
              <button type="button" className="btn small" onClick={() => api.pauseExport(j.id, false).catch(fail)}>
                ▶ {t("export.resume")}
              </button>
            ) : (
              <button type="button" className="btn small" onClick={() => api.pauseExport(j.id, true).catch(fail)}>
                ‖ {t("export.pause")}
              </button>
            )}
            <button type="button" className="btn small" onClick={() => api.cancelExport(j.id).catch(fail)}>
              {t("export.cancel")}
            </button>
          </>
        )}
        {j.state === "done" && (
          <button type="button" className="btn small" onClick={() => api.revealFolder(j.destination).catch(fail)}>
            {t("export.showFolder")}
          </button>
        )}
      </div>
    </div>
  );
}
