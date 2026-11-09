import { useState } from "react";
import { useTranslation } from "react-i18next";
import { emptyAdjust } from "../../logic/develop";
import { LOCAL_FIELDS, MASK_KINDS, type DevelopSettings, type LocalAdjust, type MaskComponent, type MaskKind, type MaskOp } from "../../types";
import type { SetField } from "./DevelopPanel";
import { Slider } from "./Slider";

/** Ferramenta ativa sobre a imagem. */
export type Tool =
  | { kind: "brush"; erase: boolean }
  | { kind: "linear" }
  | { kind: "radial" }
  | { kind: "point"; purpose: "object" | "color" }
  | { kind: "spot"; heal: boolean }
  | { kind: "patch" }
  | null;

export interface BrushSettings {
  /** Raio em % do lado maior. */
  size: number;
  feather: number;
  flow: number;
}

const FIELD_RANGE: Record<keyof LocalAdjust, [number, number, number, number]> = {
  exposure: [-4, 4, 0.01, 2],
  contrast: [-100, 100, 1, 0],
  highlights: [-100, 100, 1, 0],
  shadows: [-100, 100, 1, 0],
  whites: [-100, 100, 1, 0],
  blacks: [-100, 100, 1, 0],
  temperature: [-100, 100, 1, 0],
  tint: [-100, 100, 1, 0],
  saturation: [-100, 100, 1, 0],
  texture: [-100, 100, 1, 0],
  clarity: [-100, 100, 1, 0],
  sharpness: [-100, 100, 1, 0],
  noise: [0, 100, 1, 0],
};

function componentLabel(c: MaskComponent, t: (k: string) => string): string {
  const s = c.shape;
  return s.type === "raster" ? t(`masks.kind.${s.kind}`) : t(`masks.kind.${s.type}`);
}

/** Menu de tipos de máscara (botão + lista; uma ação só ao escolher). */
function KindMenu({ label, onPick }: { label: string; onPick: (k: MaskKind) => void }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  return (
    <div className="kind-menu" onKeyDown={(e) => e.key === "Escape" && (e.stopPropagation(), setOpen(false))}>
      <button type="button" className="btn small" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(!open)}>
        {label}
      </button>
      {open && (
        <div className="kind-menu-list" role="menu" aria-label={label}>
          {MASK_KINDS.map((k) => (
            <button
              key={k}
              type="button"
              role="menuitem"
              className="kind-menu-item"
              onClick={() => {
                setOpen(false);
                onPick(k);
              }}
            >
              {t(`masks.kind.${k}`)}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** Painel das máscaras (ajustes locais). */
export function MasksPanel({
  settings,
  set,
  selected,
  setSelected,
  tool,
  setTool,
  brush,
  setBrush,
  onAdd,
  overlay,
  setOverlay,
  busy,
}: {
  settings: DevelopSettings;
  set: SetField;
  selected: number | null;
  setSelected: (i: number | null) => void;
  tool: Tool;
  setTool: (t: Tool) => void;
  brush: BrushSettings;
  setBrush: (b: BrushSettings) => void;
  /** Acrescenta uma parte (`mask` = null → máscara nova). */
  onAdd: (kind: MaskKind, op: MaskOp, mask: number | null) => void;
  overlay: boolean;
  setOverlay: (v: boolean) => void;
  busy: boolean;
}) {
  const { t } = useTranslation();
  const masks = settings.masks;
  const m = selected !== null ? masks[selected] : undefined;
  const upd = (label: string, f: (s: DevelopSettings) => void, done = true) => set(label, f, done);
  const brushActive = tool?.kind === "brush";
  return (
    <div className="dev-masks">
      <div className="dev-row">
        <KindMenu label={t("masks.new")} onPick={(k) => onAdd(k, "add", null)} />
        {busy && <span className="small muted">{t("masks.computing")}</span>}
      </div>
      <ul className="mask-list">
        {masks.map((mk, i) => (
          <li key={mk.id} className={`mask-item${selected === i ? " active" : ""}`}>
            <input type="checkbox" checked={mk.enabled} aria-label={t("masks.enable", { name: mk.name })} onChange={(e) => upd("masks.enable", (s) => void (s.masks[i]!.enabled = e.target.checked))} />
            <button type="button" className="side-item" onClick={() => setSelected(selected === i ? null : i)}>
              {mk.name}
            </button>
            <button
              type="button"
              className="btn small ghost"
              aria-label={t("masks.delete", { name: mk.name })}
              onClick={() => {
                setSelected(null);
                setTool(null);
                upd("masks.delete", (s) => void s.masks.splice(i, 1));
              }}
            >
              ×
            </button>
          </li>
        ))}
        {masks.length === 0 && <li className="small muted">{t("masks.none")}</li>}
      </ul>
      {m && selected !== null && (
        <div className="mask-detail" aria-label={t("masks.detail", { name: m.name })} role="group">
          <div className="dev-row">
            <input
              className="mask-name"
              defaultValue={m.name}
              key={m.id}
              aria-label={t("masks.name")}
              onBlur={(e) => e.target.value.trim() && e.target.value !== m.name && upd("masks.rename", (s) => void (s.masks[selected]!.name = e.target.value.trim()))}
            />
            <label className="small">
              <input type="checkbox" checked={overlay} onChange={(e) => setOverlay(e.target.checked)} /> {t("masks.showOverlay")}
            </label>
          </div>
          <ul className="component-list">
            {m.components.map((c, j) => (
              <li key={j} className="component-item">
                <select
                  aria-label={t("masks.opOf", { name: componentLabel(c, t) })}
                  value={c.op}
                  disabled={j === 0}
                  onChange={(e) => upd("masks.op", (s) => void (s.masks[selected]!.components[j]!.op = e.target.value as MaskOp))}
                >
                  {(["add", "subtract", "intersect"] as const).map((o) => (
                    <option key={o} value={o}>
                      {t(`masks.op.${o}`)}
                    </option>
                  ))}
                </select>
                <span className="small">{componentLabel(c, t)}</span>
                <label className="small">
                  <input type="checkbox" checked={c.invert} onChange={(e) => upd("masks.invertPart", (s) => void (s.masks[selected]!.components[j]!.invert = e.target.checked))} /> {t("masks.invert")}
                </label>
                <button type="button" className="btn small ghost" aria-label={t("masks.deletePart", { name: componentLabel(c, t) })} onClick={() => upd("masks.deletePart", (s) => void s.masks[selected]!.components.splice(j, 1))}>
                  ×
                </button>
                <ComponentParams c={c} onChange={(f, done) => upd("masks.part", (s) => f(s.masks[selected]!.components[j]!), done)} />
              </li>
            ))}
          </ul>
          <div className="dev-row">
            <KindMenu label={t("masks.add")} onPick={(k) => onAdd(k, "add", selected)} />
            <KindMenu label={t("masks.subtract")} onPick={(k) => onAdd(k, "subtract", selected)} />
            <KindMenu label={t("masks.intersect")} onPick={(k) => onAdd(k, "intersect", selected)} />
          </div>
          {m.components.some((c) => c.shape.type === "brush") && (
            <div className="dev-row">
              <button type="button" className={`btn small${brushActive && !tool.erase ? " active" : ""}`} aria-pressed={brushActive && !tool.erase} onClick={() => setTool(brushActive && !tool.erase ? null : { kind: "brush", erase: false })}>
                🖌 {t("masks.paint")}
              </button>
              <button type="button" className={`btn small${brushActive && tool.erase ? " active" : ""}`} aria-pressed={brushActive && tool.erase} onClick={() => setTool(brushActive && tool.erase ? null : { kind: "brush", erase: true })}>
                {t("masks.erase")}
              </button>
            </div>
          )}
          {(brushActive || tool?.kind === "patch") && <BrushSliders brush={brush} setBrush={setBrush} />}
          <label className="small">
            <input type="checkbox" checked={m.invert} onChange={(e) => upd("masks.invert", (s) => void (s.masks[selected]!.invert = e.target.checked))} /> {t("masks.invertMask")}
          </label>
          <Slider label={t("masks.density")} value={m.density} min={0} max={100} def={100} onChange={(v, done) => upd("masks.density", (s) => void (s.masks[selected]!.density = v), done)} />
          <h4 className="small muted">{t("masks.adjustments")}</h4>
          {LOCAL_FIELDS.map((f) => {
            const [min, max, step, digits] = FIELD_RANGE[f];
            return (
              <Slider
                key={f}
                label={t(`masks.field.${f}`)}
                value={m.adjust[f]}
                min={min}
                max={max}
                step={step}
                digits={digits}
                onChange={(v, done) => upd(`masks.${f}`, (s) => void (s.masks[selected]!.adjust[f] = v), done)}
              />
            );
          })}
          <button type="button" className="btn small ghost" onClick={() => upd("masks.resetAdjust", (s) => void (s.masks[selected]!.adjust = emptyAdjust()))}>
            ↺ {t("masks.resetAdjust")}
          </button>
        </div>
      )}
    </div>
  );
}

export function BrushSliders({ brush, setBrush }: { brush: BrushSettings; setBrush: (b: BrushSettings) => void }) {
  const { t } = useTranslation();
  return (
    <div className="brush-sliders">
      <Slider label={t("masks.brushSize")} value={brush.size} min={0.3} max={25} step={0.1} digits={1} def={3} onChange={(v) => setBrush({ ...brush, size: v })} />
      <Slider label={t("masks.feather")} value={brush.feather} min={0} max={100} def={50} onChange={(v) => setBrush({ ...brush, feather: v })} />
      <Slider label={t("masks.flow")} value={brush.flow} min={1} max={100} def={100} onChange={(v) => setBrush({ ...brush, flow: v })} />
    </div>
  );
}

function ComponentParams({ c, onChange }: { c: MaskComponent; onChange: (f: (c: MaskComponent) => void, done: boolean) => void }) {
  const { t } = useTranslation();
  const s = c.shape;
  if (s.type === "luminance")
    return (
      <div className="component-params">
        <Slider label={t("masks.lumMin")} value={s.min} min={0} max={1} step={0.01} digits={2} onChange={(v, d) => onChange((x) => void ((x.shape as typeof s).min = v), d)} />
        <Slider label={t("masks.lumMax")} value={s.max} min={0} max={1} step={0.01} digits={2} def={1} onChange={(v, d) => onChange((x) => void ((x.shape as typeof s).max = v), d)} />
        <Slider label={t("masks.smooth")} value={s.smooth} min={0} max={0.5} step={0.01} digits={2} def={0.1} onChange={(v, d) => onChange((x) => void ((x.shape as typeof s).smooth = v), d)} />
      </div>
    );
  if (s.type === "color")
    return (
      <div className="component-params">
        <span className="color-swatch" style={{ background: `hsl(${s.hue}, 80%, 50%)` }} aria-hidden />
        <Slider label={t("masks.colorRange")} value={s.range} min={2} max={90} def={20} onChange={(v, d) => onChange((x) => void ((x.shape as typeof s).range = v), d)} />
        <Slider label={t("masks.smooth")} value={s.smooth} min={0} max={1} step={0.01} digits={2} def={0.3} onChange={(v, d) => onChange((x) => void ((x.shape as typeof s).smooth = v), d)} />
      </div>
    );
  if (s.type === "radial")
    return (
      <div className="component-params">
        <Slider label={t("masks.feather")} value={s.feather} min={0} max={100} def={50} onChange={(v, d) => onChange((x) => void ((x.shape as typeof s).feather = v), d)} />
      </div>
    );
  if (s.type === "raster")
    return (
      <div className="component-params">
        <Slider label={t("masks.feather")} value={s.feather} min={0} max={100} def={0} onChange={(v, d) => onChange((x) => void ((x.shape as typeof s).feather = v), d)} />
      </div>
    );
  return null;
}

/** Retoque: manchas (corrigir/clonar), remoção de objetos, pó do sensor, olhos vermelhos. */
export function RetouchTools({
  settings,
  set,
  tool,
  setTool,
  brush,
  setBrush,
  busy,
  onDetect,
}: {
  settings: DevelopSettings;
  set: SetField;
  tool: Tool;
  setTool: (t: Tool) => void;
  brush: BrushSettings;
  setBrush: (b: BrushSettings) => void;
  busy: boolean;
  onDetect: (kind: "redEye" | "dust") => void;
}) {
  const { t } = useTranslation();
  const r = settings.retouch;
  const spotOn = tool?.kind === "spot";
  return (
    <div className="dev-retouch">
      <div className="dev-row" role="group" aria-label={t("retouch.tools")}>
        <button type="button" className={`btn small${spotOn && tool.heal ? " active" : ""}`} aria-pressed={spotOn && tool.heal} onClick={() => setTool(spotOn && tool.heal ? null : { kind: "spot", heal: true })} title={t("retouch.healHint")}>
          {t("retouch.heal")}
        </button>
        <button type="button" className={`btn small${spotOn && !tool.heal ? " active" : ""}`} aria-pressed={spotOn && !tool.heal} onClick={() => setTool(spotOn && !tool.heal ? null : { kind: "spot", heal: false })} title={t("retouch.cloneHint")}>
          {t("retouch.clone")}
        </button>
        <button type="button" className={`btn small${tool?.kind === "patch" ? " active" : ""}`} aria-pressed={tool?.kind === "patch"} onClick={() => setTool(tool?.kind === "patch" ? null : { kind: "patch" })} title={t("retouch.objectHint")}>
          {t("retouch.object")}
        </button>
      </div>
      {(spotOn || tool?.kind === "patch") && <BrushSliders brush={brush} setBrush={setBrush} />}
      <ul className="spot-list">
        {r.spots.map((sp, i) => (
          <li key={`s${i}`} className="dev-row">
            <span className="small">{t("retouch.spotN", { n: i + 1 })}</span>
            <select aria-label={t("retouch.modeOf", { n: i + 1 })} value={sp.heal ? "heal" : "clone"} onChange={(e) => set("retouch.mode", (s) => void (s.retouch.spots[i]!.heal = e.target.value === "heal"), true)}>
              <option value="heal">{t("retouch.heal")}</option>
              <option value="clone">{t("retouch.clone")}</option>
            </select>
            <span className="spacer" />
            <button type="button" className="btn small ghost" aria-label={t("retouch.removeSpot", { n: i + 1 })} onClick={() => set("retouch.removeSpot", (s) => void s.retouch.spots.splice(i, 1), true)}>
              ×
            </button>
          </li>
        ))}
        {r.patches.map((_, i) => (
          <li key={`p${i}`} className="dev-row">
            <span className="small">{t("retouch.objectN", { n: i + 1 })}</span>
            <span className="spacer" />
            <button type="button" className="btn small ghost" aria-label={t("retouch.removeObject", { n: i + 1 })} onClick={() => set("retouch.removeObject", (s) => void s.retouch.patches.splice(i, 1), true)}>
              ×
            </button>
          </li>
        ))}
      </ul>
      {(["redEye", "dust"] as const).map((kind) => {
        const n = kind === "redEye" ? r.redEyes.length : r.spots.length;
        return (
          <div key={kind} className="dev-row">
            <span className="small">{t(`develop.retouch.${kind}`, { count: n })}</span>
            <span className="spacer" />
            <button type="button" className="btn small" disabled={busy} onClick={() => onDetect(kind)}>
              {t("develop.retouch.detect")}
            </button>
            <button
              type="button"
              className="btn small ghost"
              disabled={n === 0}
              onClick={() =>
                set(`retouch.${kind}`, (s) => {
                  if (kind === "redEye") s.retouch.redEyes = [];
                  else s.retouch.spots = [];
                }, true)
              }
            >
              {t("develop.retouch.remove")}
            </button>
          </div>
        );
      })}
      <p className="small muted dev-note">{t("retouch.note")}</p>
    </div>
  );
}

/** Retrato: subtil, só nas regiões de cada rosto; tudo removível. */
export function PortraitPanel({ settings, set, onDetect, busy }: { settings: DevelopSettings; set: SetField; onDetect: () => void; busy: boolean }) {
  const { t } = useTranslation();
  const p = settings.portrait;
  const [hint] = useState(t("portrait.note"));
  return (
    <div className="dev-portrait">
      <div className="dev-row">
        <button type="button" className="btn small" disabled={busy} onClick={onDetect}>
          {busy ? t("portrait.detecting") : t("portrait.detect")}
        </button>
        <span className="small muted">{t("portrait.faces", { count: p.faces.length })}</span>
        <span className="spacer" />
        <button
          type="button"
          className="btn small ghost"
          disabled={p.faces.length === 0}
          onClick={() => set("portrait.remove", (s) => void (s.portrait = { skin: 0, darkCircles: 0, shine: 0, teeth: 0, eyes: 0, faces: [] }), true)}
        >
          {t("portrait.remove")}
        </button>
      </div>
      {(["skin", "darkCircles", "shine", "teeth", "eyes"] as const).map((k) => (
        <Slider key={k} label={t(`portrait.field.${k}`)} value={p[k]} min={0} max={100} onChange={(v, done) => set(`portrait.${k}`, (s) => void (s.portrait[k] = v), done)} />
      ))}
      <p className="small muted dev-note">{hint}</p>
    </div>
  );
}
