import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { open, save } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import { groupPresets } from "../logic/develop";
import { errorText, useStore } from "../store";
import { LOOK_COMPONENTS, PRESET_COMPONENTS, type DevelopSettings, type Preset, type PresetComponent, type PresetImportResult } from "../types";
import { Slider } from "./develop/Slider";
import { Modal } from "./Modal";
import { momentName } from "./MomentsPanel";

/** Nome do grupo para mostrar (internos e "sem grupo"). */
export function groupLabel(g: string, t: (k: string) => string): string {
  return g ? t(`presets.groups.${g}`) !== `presets.groups.${g}` ? t(`presets.groups.${g}`) : g : t("presets.noGroup");
}

/** Lista de presets agrupada (usada no editor e no diálogo de aplicar). */
export function PresetList({
  presets,
  selected,
  onPick,
  onHover,
}: {
  presets: Preset[];
  selected?: string | null;
  onPick: (p: Preset) => void;
  onHover?: (p: Preset | null) => void;
}) {
  const { t } = useTranslation();
  const [closed, setClosed] = useState<Record<string, boolean>>({});
  const groups = useMemo(() => groupPresets(presets, ""), [presets]);
  return (
    <div className="preset-list" onMouseLeave={() => onHover?.(null)}>
      {groups.map(([g, list]) => (
        <div key={g || "_"} className="preset-group">
          <button type="button" className="preset-group-title" aria-expanded={!closed[g]} onClick={() => setClosed((c) => ({ ...c, [g]: !c[g] }))}>
            {closed[g] ? "▸" : "▾"} {groupLabel(g, t)}
          </button>
          {!closed[g] && (
            <ul>
              {list.map((p) => (
                <li key={p.id}>
                  <button
                    type="button"
                    className={`preset-item${selected === p.id ? " active" : ""}`}
                    title={p.description || p.name}
                    onMouseEnter={() => onHover?.(p)}
                    onFocus={() => onHover?.(p)}
                    onClick={() => onPick(p)}
                  >
                    {p.name}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      ))}
    </div>
  );
}

function ComponentChecks({ value, onChange }: { value: PresetComponent[]; onChange: (v: PresetComponent[]) => void }) {
  const { t } = useTranslation();
  return (
    <fieldset className="auto-groups preset-components">
      <legend className="small muted">{t("presets.components")}</legend>
      {PRESET_COMPONENTS.map((c) => (
        <label key={c} className="small">
          <input type="checkbox" checked={value.includes(c)} onChange={(e) => onChange(e.target.checked ? [...value, c] : value.filter((x) => x !== c))} /> {t(`presets.component.${c}`)}
        </label>
      ))}
    </fieldset>
  );
}

/** Criar preset a partir dos ajustes atuais. */
export function CreatePresetDialog({ settings, onClose }: { settings: DevelopSettings; onClose: () => void }) {
  const { t } = useTranslation();
  const presets = useStore((s) => s.presets);
  const reload = useStore((s) => s.reloadPresets);
  const toast = useStore((s) => s.toast);
  const [name, setName] = useState("");
  const [group, setGroup] = useState("");
  const [comps, setComps] = useState<PresetComponent[]>(settings.lut.id ? LOOK_COMPONENTS : LOOK_COMPONENTS.filter((c) => c !== "lut"));
  const groups = [...new Set(presets.filter((p) => !p.builtin && p.group).map((p) => p.group))];
  const create = async () => {
    try {
      await api.createPreset(name, group, comps, settings);
      await reload();
      toast("info", t("presets.created", { name }));
      onClose();
    } catch (e) {
      toast("error", errorText(e, t));
    }
  };
  return (
    <Modal
      title={t("presets.createTitle")}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn" onClick={onClose} data-close>
            {t("common.cancel")}
          </button>
          <button type="button" className="btn primary" disabled={!name.trim() || comps.length === 0} onClick={() => void create()}>
            {t("presets.create")}
          </button>
        </>
      }
    >
      <label className="field">
        {t("presets.name")}
        <input value={name} onChange={(e) => setName(e.target.value)} aria-label={t("presets.name")} />
      </label>
      <label className="field">
        {t("presets.group")}
        <input value={group} list="preset-groups" onChange={(e) => setGroup(e.target.value)} aria-label={t("presets.group")} placeholder={t("presets.noGroup")} />
        <datalist id="preset-groups">
          {groups.map((g) => (
            <option key={g} value={g} />
          ))}
        </datalist>
      </label>
      <ComponentChecks value={comps} onChange={setComps} />
      <p className="small muted">{t("presets.componentsNote")}</p>
    </Modal>
  );
}

/** Relatório de importação (XMP aproximado / LUT). */
export function ImportReportDialog({ result, onClose }: { result: PresetImportResult; onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <Modal
      title={t(`presets.imported.${result.kind}`, { count: result.presets.length })}
      onClose={onClose}
      wide
      footer={
        <button type="button" className="btn primary" onClick={onClose}>
          {t("common.close")}
        </button>
      }
    >
      <p>{result.presets.map((p) => p.name).join(", ")}</p>
      {result.kind === "xmp" && result.report && (
        <>
          <p className="notice warn small">{t("presets.xmpWarning")}</p>
          <h4>{t("presets.interpreted", { count: result.report.interpreted.length })}</h4>
          <p className="small muted">{result.report.interpreted.join(", ")}</p>
          {result.report.ignored.length > 0 && (
            <>
              <h4>{t("presets.ignored", { count: result.report.ignored.length })}</h4>
              <ul className="small compat-list">
                {result.report.ignored.map(([k, why]) => (
                  <li key={k}>
                    <code>{k}</code> — {why}
                  </li>
                ))}
              </ul>
            </>
          )}
        </>
      )}
      {result.kind === "cube" && <p className="small muted">{t("presets.cubeNote")}</p>}
    </Modal>
  );
}

/** Importar ficheiro de presets (com relatório). */
export async function importPresets(t: (k: string, o?: Record<string, unknown>) => string, onReport: (r: PresetImportResult) => void) {
  const st = useStore.getState();
  const path = await open({ multiple: false, filters: [{ name: t("presets.fileFilter"), extensions: ["momentopreset", "xmp", "cube"] }] });
  if (!path || Array.isArray(path)) return;
  try {
    const r = await api.importPresetFile(path);
    await st.reloadPresets();
    if (r.kind === "momentopreset") st.toast("info", t("presets.imported.momentopreset", { count: r.presets.length }));
    else onReport(r);
  } catch (e) {
    st.toast("error", errorText(e, t));
  }
}

/** Gerir presets: renomear, grupo, ordem, duplicar, eliminar, exportar. */
export function PresetManager({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const presets = useStore((s) => s.presets);
  const reload = useStore((s) => s.reloadPresets);
  const toast = useStore((s) => s.toast);
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [editing, setEditing] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<Preset | null>(null);
  const [report, setReport] = useState<PresetImportResult | null>(null);
  const fail = (e: unknown) => toast("error", errorText(e, t));
  const run = async (f: () => Promise<unknown>) => {
    try {
      await f();
      await reload();
    } catch (e) {
      fail(e);
    }
  };
  const move = (p: Preset, dir: -1 | 1) => {
    const same = presets.filter((x) => !x.builtin && x.group === p.group);
    const i = same.findIndex((x) => x.id === p.id);
    const j = i + dir;
    if (j < 0 || j >= same.length) return;
    const ids = same.map((x) => x.id);
    [ids[i], ids[j]] = [ids[j]!, ids[i]!];
    void run(() => api.reorderPresets(ids));
  };
  const exportSel = async () => {
    const path = await save({ defaultPath: "presets.momentopreset", filters: [{ name: "Momento", extensions: ["momentopreset"] }] });
    if (!path) return;
    try {
      const n = await api.exportPresets([...sel], path);
      toast("info", t("presets.exported", { count: n }));
    } catch (e) {
      fail(e);
    }
  };
  const groups = groupPresets(presets, "");
  return (
    <Modal
      title={t("presets.manageTitle")}
      onClose={onClose}
      wide
      footer={
        <>
          <button type="button" className="btn" onClick={() => void importPresets(t, setReport)}>
            {t("presets.import")}
          </button>
          <button type="button" className="btn" disabled={sel.size === 0} onClick={() => void exportSel()}>
            {t("presets.export", { count: sel.size })}
          </button>
          <span className="spacer" />
          <button type="button" className="btn primary" onClick={onClose} data-close>
            {t("common.close")}
          </button>
        </>
      }
    >
      <div className="preset-manager">
        {groups.map(([g, list]) => (
          <section key={g || "_"} aria-label={groupLabel(g, t)}>
            <h4>{groupLabel(g, t)}</h4>
            <ul>
              {list.map((p) => (
                <li key={p.id} className="preset-row">
                  <input type="checkbox" aria-label={t("presets.select", { name: p.name })} checked={sel.has(p.id)} onChange={(e) => setSel((s) => { const n = new Set(s); if (e.target.checked) n.add(p.id); else n.delete(p.id); return n; })} />
                  {editing === p.id ? (
                    <input
                      autoFocus
                      defaultValue={p.name}
                      aria-label={t("presets.name")}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                        if (e.key === "Escape") setEditing(null);
                      }}
                      onBlur={(e) => {
                        setEditing(null);
                        const v = e.target.value.trim();
                        if (v && v !== p.name) void run(() => api.renamePreset(p.id, v));
                      }}
                    />
                  ) : (
                    <span className="preset-name" title={p.description}>
                      {p.name}
                      {p.builtin && <span className="badge">{t("presets.builtin")}</span>}
                    </span>
                  )}
                  <span className="small muted preset-comps">{p.components.map((c) => t(`presets.component.${c}`)).join(" · ")}</span>
                  {!p.builtin && (
                    <>
                      <button type="button" className="btn small ghost" aria-label={t("presets.up", { name: p.name })} onClick={() => move(p, -1)}>
                        ↑
                      </button>
                      <button type="button" className="btn small ghost" aria-label={t("presets.down", { name: p.name })} onClick={() => move(p, 1)}>
                        ↓
                      </button>
                      <button type="button" className="btn small" onClick={() => setEditing(p.id)}>
                        {t("presets.rename")}
                      </button>
                      <input
                        className="preset-group-input"
                        list="preset-groups-m"
                        defaultValue={p.group}
                        placeholder={t("presets.noGroup")}
                        aria-label={t("presets.groupOf", { name: p.name })}
                        onBlur={(e) => {
                          if (e.target.value.trim() !== p.group) void run(() => api.movePreset(p.id, e.target.value.trim()));
                        }}
                        onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
                      />
                    </>
                  )}
                  <button type="button" className="btn small" onClick={() => void run(() => api.duplicatePreset(p.id, t("presets.copySuffix")))}>
                    {t("presets.duplicate")}
                  </button>
                  {!p.builtin && (
                    <button type="button" className="btn small ghost" onClick={() => setConfirm(p)}>
                      {t("presets.delete")}
                    </button>
                  )}
                </li>
              ))}
            </ul>
          </section>
        ))}
        <datalist id="preset-groups-m">
          {[...new Set(presets.filter((p) => !p.builtin && p.group).map((p) => p.group))].map((g) => (
            <option key={g} value={g} />
          ))}
        </datalist>
      </div>
      {confirm && (
        <Modal
          title={t("presets.deleteTitle", { name: confirm.name })}
          onClose={() => setConfirm(null)}
          footer={
            <>
              <button type="button" className="btn" onClick={() => setConfirm(null)} data-close>
                {t("common.cancel")}
              </button>
              <button
                type="button"
                className="btn primary"
                onClick={() => {
                  const id = confirm.id;
                  setConfirm(null);
                  void run(() => api.deletePreset(id));
                }}
              >
                {t("presets.delete")}
              </button>
            </>
          }
        >
          <p>{t("presets.deleteNote")}</p>
        </Modal>
      )}
      {report && <ImportReportDialog result={report} onClose={() => setReport(null)} />}
    </Modal>
  );
}

type Scope = "selection" | "moment" | "all";

/** Aplicar um preset à seleção, a um momento ou ao casamento completo. */
export function ApplyPresetDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const presets = useStore((s) => s.presets);
  const selection = useStore((s) => s.selection);
  const photos = useStore((s) => s.photos);
  const moments = useStore((s) => s.moments);
  const photoMoment = useStore((s) => s.photoMoment);
  const toast = useStore((s) => s.toast);
  const reload = useStore((s) => s.reloadPresets);
  const [preset, setPreset] = useState<Preset | null>(null);
  const [amount, setAmount] = useState(100);
  const [scope, setScope] = useState<Scope>(selection.ids.size > 0 ? "selection" : "all");
  const [moment, setMoment] = useState<number | null>(moments[0]?.id ?? null);
  useEffect(() => {
    if (presets.length === 0) void reload();
  }, [presets.length, reload]);
  const keys = useMemo(() => {
    if (scope === "selection") return photos.filter((p) => selection.ids.has(p.id)).map((p) => p.id);
    if (scope === "moment") return photos.filter((p) => photoMoment.get(p.asset) === moment).map((p) => p.id);
    return photos.map((p) => p.id);
  }, [scope, photos, selection, photoMoment, moment]);
  const apply = async () => {
    if (!preset) return;
    try {
      const n = await api.applyPreset(keys, preset.id, amount);
      toast("info", t("presets.applied", { name: preset.name, count: n }));
      onClose();
    } catch (e) {
      toast("error", errorText(e, t));
    }
  };
  return (
    <Modal
      title={t("presets.applyTitle")}
      onClose={onClose}
      wide
      footer={
        <>
          <button type="button" className="btn" onClick={onClose} data-close>
            {t("common.cancel")}
          </button>
          <button type="button" className="btn primary" disabled={!preset || keys.length === 0} onClick={() => void apply()}>
            {t("presets.applyN", { count: keys.length })}
          </button>
        </>
      }
    >
      <div className="apply-preset">
        <PresetList presets={presets} selected={preset?.id} onPick={setPreset} />
        <div className="apply-preset-side">
          <Slider label={t("presets.amount")} value={amount} min={0} max={150} def={100} onChange={(v) => setAmount(v)} />
          <fieldset className="options">
            <legend>{t("presets.scope")}</legend>
            <label>
              <input type="radio" name="scope" checked={scope === "selection"} disabled={selection.ids.size === 0} onChange={() => setScope("selection")} /> {t("presets.scopeSelection", { count: selection.ids.size })}
            </label>
            <label>
              <input type="radio" name="scope" checked={scope === "moment"} disabled={moments.length === 0} onChange={() => setScope("moment")} /> {t("presets.scopeMoment")}
              <select aria-label={t("presets.scopeMoment")} value={moment ?? ""} disabled={scope !== "moment"} onChange={(e) => setMoment(Number(e.target.value))}>
                {moments.map((m) => (
                  <option key={m.id} value={m.id}>
                    {momentName(m, t)}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <input type="radio" name="scope" checked={scope === "all"} onChange={() => setScope("all")} /> {t("presets.scopeAll", { count: photos.length })}
            </label>
          </fieldset>
          {preset && <p className="small muted">{preset.description || preset.components.map((c) => t(`presets.component.${c}`)).join(" · ")}</p>}
          <p className="small muted">{t("presets.applyNote")}</p>
        </div>
      </div>
    </Modal>
  );
}
