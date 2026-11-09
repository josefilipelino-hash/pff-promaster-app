import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { DevelopSettings } from "../../types";
import { CurveEditor } from "./CurveEditor";
import { Histogram } from "./Histogram";
import { Slider } from "./Slider";

/** Campo numérico: [bloco, campo, mín, máx, passo, neutro, casas decimais]. */
type Field = [keyof DevelopSettings, string, number, number, number, number, number?];

const SECTIONS: [string, Field[]][] = [
  ["light", [
    ["light", "exposure", -5, 5, 0.01, 0, 2],
    ["light", "contrast", -100, 100, 1, 0],
    ["light", "highlights", -100, 100, 1, 0],
    ["light", "shadows", -100, 100, 1, 0],
    ["light", "whites", -100, 100, 1, 0],
    ["light", "blacks", -100, 100, 1, 0],
  ]],
  ["wb", [
    ["wb", "temperature", -100, 100, 1, 0],
    ["wb", "tint", -100, 100, 1, 0],
  ]],
  ["color", [
    ["color", "vibrance", -100, 100, 1, 0],
    ["color", "saturation", -100, 100, 1, 0],
  ]],
  ["detail", [
    ["detail", "sharpenAmount", 0, 150, 1, 0],
    ["detail", "sharpenRadius", 0.5, 3, 0.1, 1, 1],
    ["detail", "sharpenDetail", 0, 100, 1, 25],
    ["detail", "sharpenMasking", 0, 100, 1, 0],
    ["detail", "noiseLuminance", 0, 100, 1, 0],
    ["detail", "noiseDetail", 0, 100, 1, 50],
    ["detail", "noiseColor", 0, 100, 1, 0],
  ]],
  ["effects", [
    ["effects", "texture", -100, 100, 1, 0],
    ["effects", "clarity", -100, 100, 1, 0],
    ["effects", "dehaze", -100, 100, 1, 0],
    ["effects", "vignette", -100, 100, 1, 0],
    ["effects", "vignetteMidpoint", 0, 100, 1, 50],
    ["effects", "vignetteFeather", 0, 100, 1, 50],
    ["effects", "vignetteRoundness", -100, 100, 1, 0],
    ["effects", "grain", 0, 100, 1, 0],
    ["effects", "grainSize", 0, 100, 1, 25],
    ["effects", "grainRoughness", 0, 100, 1, 50],
  ]],
  ["grading", [
    ["grading", "shadowsHue", 0, 360, 1, 220],
    ["grading", "shadowsSat", 0, 100, 1, 0],
    ["grading", "highlightsHue", 0, 360, 1, 45],
    ["grading", "highlightsSat", 0, 100, 1, 0],
    ["grading", "balance", -100, 100, 1, 0],
  ]],
  ["lens", [
    ["lens", "distortion", -100, 100, 1, 0],
    ["lens", "caRed", -100, 100, 1, 0],
    ["lens", "caBlue", -100, 100, 1, 0],
    ["lens", "vignetting", -100, 100, 1, 0],
    ["lens", "vignettingMidpoint", 0, 100, 1, 50],
  ]],
];

const HUE_GRADIENT = "linear-gradient(90deg, #e5484d, #e2c23b, #46a758, #3cc0c0, #3e8ed0, #8e4ec6, #e5484d)";
const GRADIENTS: Record<string, string> = {
  "grading.shadowsHue": HUE_GRADIENT,
  "grading.highlightsHue": HUE_GRADIENT,
  "wb.temperature": "linear-gradient(90deg, #4a7bd0, #ddd, #e0a94a)",
  "wb.tint": "linear-gradient(90deg, #4ab04a, #ddd, #c04ac0)",
};

const HSL_COLORS = ["#e5484d", "#f08a3c", "#e2c23b", "#46a758", "#3cc0c0", "#3e8ed0", "#8e4ec6", "#d04aa0"];
const HSL_KEYS = ["red", "orange", "yellow", "green", "aqua", "blue", "purple", "magenta"];

export type SetField = (label: string, mutate: (s: DevelopSettings) => void, done: boolean) => void;

function Section({ id, open, toggle, children, onReset }: { id: string; open: boolean; toggle: () => void; children: React.ReactNode; onReset?: () => void }) {
  const { t } = useTranslation();
  return (
    <section className={`dev-section${open ? " open" : ""}`} aria-label={t(`develop.section.${id}`)}>
      <header>
        <button type="button" className="dev-section-title" aria-expanded={open} onClick={toggle}>
          {open ? "▾" : "▸"} {t(`develop.section.${id}`)}
        </button>
        {onReset && (
          <button type="button" className="btn small ghost" onClick={onReset} title={t("develop.resetSection")}>
            ↺
          </button>
        )}
      </header>
      {open && <div className="dev-section-body">{children}</div>}
    </section>
  );
}

/** Painel de ajustes (à direita no modo Revelar). */
export function DevelopPanel({
  settings,
  set,
  histogram,
  wbPicking,
  onPickWb,
  geometry,
  auto,
  presets,
  retouch,
  ai,
}: {
  settings: DevelopSettings;
  set: SetField;
  histogram: [Uint32Array, Uint32Array, Uint32Array] | null;
  wbPicking: boolean;
  onPickWb: () => void;
  geometry: React.ReactNode;
  /** Secção "Automático" (opções, botão e relatório). */
  auto?: React.ReactNode;
  /** Secção "Presets". */
  presets?: React.ReactNode;
  /** Secção "Retoque" (olhos vermelhos, manchas). */
  retouch?: React.ReactNode;
  /** AI Denoise: modelo disponível e progresso do cálculo (0–1) em curso. */
  ai?: { available: boolean; progress: number | null };
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState<Record<string, boolean>>({ auto: true, presets: true, light: true, wb: true, color: true });
  const [hslTab, setHslTab] = useState<"hue" | "saturation" | "luminance">("hue");
  const toggle = (k: string) => setOpen((o) => ({ ...o, [k]: !o[k] }));
  const fieldSlider = ([block, key, min, max, step, def, digits]: Field) => {
    const value = (settings[block] as unknown as Record<string, number>)[key]!;
    const label = `${block}.${key}`;
    return (
      <Slider
        key={label}
        label={t(`develop.field.${label}`)}
        value={value}
        min={min}
        max={max}
        step={step}
        def={def}
        digits={digits ?? 0}
        gradient={GRADIENTS[label]}
        onChange={(v, done) =>
          set(label, (s) => {
            (s[block] as unknown as Record<string, number>)[key] = v;
          }, done)
        }
      />
    );
  };
  const sectionFields = (id: string) => SECTIONS.find((s) => s[0] === id)![1];
  const resetBlock = (id: keyof DevelopSettings, fields: Field[]) => () =>
    set(`reset.${id}`, (s) => {
      for (const [b, k, , , , def] of fields) (s[b] as unknown as Record<string, number>)[k] = def;
    }, true);
  const c = settings.curve;
  return (
    <aside className="develop-panel" aria-label={t("develop.panel")}>
      <Histogram data={histogram} />
      {auto && (
        <Section id="auto" open={!!open.auto} toggle={() => toggle("auto")}>
          {auto}
        </Section>
      )}
      {presets && (
        <Section id="presets" open={!!open.presets} toggle={() => toggle("presets")}>
          {presets}
        </Section>
      )}
      <Section id="light" open={!!open.light} toggle={() => toggle("light")} onReset={resetBlock("light", sectionFields("light"))}>
        {sectionFields("light").map(fieldSlider)}
      </Section>
      <Section id="wb" open={!!open.wb} toggle={() => toggle("wb")} onReset={resetBlock("wb", sectionFields("wb"))}>
        <button type="button" className={`btn small${wbPicking ? " active" : ""}`} aria-pressed={wbPicking} onClick={onPickWb} title={t("develop.wbPickerHint")}>
          ⊙ {t("develop.wbPicker")}
        </button>
        {sectionFields("wb").map(fieldSlider)}
      </Section>
      <Section id="color" open={!!open.color} toggle={() => toggle("color")} onReset={resetBlock("color", sectionFields("color"))}>
        {sectionFields("color").map(fieldSlider)}
      </Section>
      <Section
        id="hsl"
        open={!!open.hsl}
        toggle={() => toggle("hsl")}
        onReset={() =>
          set("reset.hsl", (s) => {
            s.hsl = { hue: Array(8).fill(0), saturation: Array(8).fill(0), luminance: Array(8).fill(0) };
          }, true)
        }
      >
        <div className="segmented small" role="group" aria-label={t("develop.section.hsl")}>
          {(["hue", "saturation", "luminance"] as const).map((k) => (
            <button key={k} type="button" className={`btn small${hslTab === k ? " active" : ""}`} aria-pressed={hslTab === k} onClick={() => setHslTab(k)}>
              {t(`develop.hsl.${k}`)}
            </button>
          ))}
        </div>
        {HSL_KEYS.map((name, i) => (
          <Slider
            key={`${hslTab}-${name}`}
            label={t(`develop.hslColor.${name}`)}
            value={settings.hsl[hslTab][i]!}
            min={-100}
            max={100}
            gradient={`linear-gradient(90deg, #555, ${HSL_COLORS[i]})`}
            onChange={(v, done) =>
              set(`hsl.${hslTab}`, (s) => {
                s.hsl[hslTab][i] = v;
              }, done)
            }
          />
        ))}
      </Section>
      <Section
        id="curve"
        open={!!open.curve}
        toggle={() => toggle("curve")}
        onReset={() =>
          set("reset.curve", (s) => {
            s.curve = { ...s.curve, highlights: 0, lights: 0, darks: 0, shadows: 0, splitShadows: 25, splitMidtones: 50, splitHighlights: 75, rgb: [], red: [], green: [], blue: [] };
          }, true)
        }
      >
        <CurveEditor
          curves={{ rgb: c.rgb, red: c.red, green: c.green, blue: c.blue }}
          onChange={(ch, pts, done) =>
            set("curve.points", (s) => {
              s.curve[ch] = pts;
            }, done)
          }
        />
        {(["highlights", "lights", "darks", "shadows"] as const).map((k) => (
          <Slider key={k} label={t(`develop.field.curve.${k}`)} value={c[k]} min={-100} max={100} onChange={(v, done) => set(`curve.${k}`, (s) => void (s.curve[k] = v), done)} />
        ))}
        {(["splitShadows", "splitMidtones", "splitHighlights"] as const).map((k, i) => (
          <Slider key={k} label={t(`develop.field.curve.${k}`)} value={c[k]} min={5} max={95} def={[25, 50, 75][i]} onChange={(v, done) => set(`curve.${k}`, (s) => void (s.curve[k] = v), done)} />
        ))}
      </Section>
      <Section id="detail" open={!!open.detail} toggle={() => toggle("detail")} onReset={resetBlock("detail", sectionFields("detail"))}>
        <p className="small muted dev-note">{t("develop.detailNote")}</p>
        {sectionFields("detail").map(fieldSlider)}
        <div className="dev-ai">
          <Slider
            label={t("develop.field.detail.aiDenoise")}
            value={settings.detail.aiDenoise}
            min={0}
            max={100}
            onChange={(v, done) => set("detail.aiDenoise", (s) => void (s.detail.aiDenoise = v), done)}
          />
          {ai && !ai.available && <p className="small muted dev-note">{t("auto.aiMissing")}</p>}
          {ai?.progress !== null && ai?.progress !== undefined && (
            <p className="small dev-note" role="status">
              {t("develop.aiRunning", { pct: Math.round(ai.progress * 100) })}
            </p>
          )}
        </div>
      </Section>
      <Section id="effects" open={!!open.effects} toggle={() => toggle("effects")} onReset={resetBlock("effects", sectionFields("effects"))}>
        {sectionFields("effects").map(fieldSlider)}
      </Section>
      <Section id="grading" open={!!open.grading} toggle={() => toggle("grading")} onReset={resetBlock("grading", sectionFields("grading"))}>
        {sectionFields("grading").map(fieldSlider)}
        {settings.lut.id && (
          <div className="dev-ai">
            <div className="dev-row">
              <span className="small">
                {t("develop.lut")}: <strong>{settings.lut.name || settings.lut.id}</strong>
              </span>
              <span className="spacer" />
              <button type="button" className="btn small ghost" onClick={() => set("lut.remove", (s) => void (s.lut = { id: "", name: "", amount: 100 }), true)}>
                {t("develop.retouch.remove")}
              </button>
            </div>
            <Slider label={t("develop.field.lut.amount")} value={settings.lut.amount} min={0} max={100} def={100} onChange={(v, done) => set("lut.amount", (s) => void (s.lut.amount = v), done)} />
          </div>
        )}
      </Section>
      <Section
        id="bw"
        open={!!open.bw}
        toggle={() => toggle("bw")}
        onReset={() => set("reset.bw", (s) => void (s.bw = { enabled: false, mix: Array(8).fill(0) }), true)}
      >
        <label className="small">
          <input type="checkbox" checked={settings.bw.enabled} onChange={(e) => set("bw.enabled", (s) => void (s.bw.enabled = e.target.checked), true)} /> {t("develop.bwEnable")}
        </label>
        {settings.bw.enabled &&
          HSL_KEYS.map((name, i) => (
            <Slider
              key={`bw-${name}`}
              label={t(`develop.hslColor.${name}`)}
              value={settings.bw.mix[i]!}
              min={-100}
              max={100}
              gradient={`linear-gradient(90deg, #222, ${HSL_COLORS[i]}, #eee)`}
              onChange={(v, done) => set("bw.mix", (s) => void (s.bw.mix[i] = v), done)}
            />
          ))}
      </Section>
      <Section id="geometry" open={!!open.geometry} toggle={() => toggle("geometry")}>
        {geometry}
      </Section>
      <Section id="lens" open={!!open.lens} toggle={() => toggle("lens")} onReset={resetBlock("lens", sectionFields("lens"))}>
        {sectionFields("lens").map(fieldSlider)}
      </Section>
      {retouch && (
        <Section id="retouch" open={!!open.retouch} toggle={() => toggle("retouch")}>
          {retouch}
        </Section>
      )}
    </aside>
  );
}
