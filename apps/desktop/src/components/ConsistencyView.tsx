import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, imageUrl } from "../api";
import { defaultMatchOptions } from "../logic/develop";
import { errorText, useStore } from "../store";
import { CRITERIA, type Criterion, type LookGroup, type MatchOptions, type MomentRow } from "../types";
import { momentName } from "./MomentsPanel";
import { Icon } from "./Icon";

const hhmm = (t: string | null) => (t ? t.slice(11, 16) : "");

/** Título de um grupo (traduzido). */
export function groupTitle(g: LookGroup, t: (k: string) => string, moments: MomentRow[]): string {
  if (g.code === "camera") return g.detail;
  if (g.code === "scene.moment") {
    const m = moments.find((x) => x.id === g.moment);
    return m ? momentName(m, t) : t("look.scene.unclassified");
  }
  if (g.code === "time") return `${hhmm(g.start)}–${hhmm(g.end)}`;
  return t(`look.${g.code}`);
}

/** Consistência: grupos de fotografias com luz/condições semelhantes e sincronização com uma referência. */
export function ConsistencyView({ onEdit }: { onEdit: (key: number) => void }) {
  const { t } = useTranslation();
  const project = useStore((s) => s.project);
  const photos = useStore((s) => s.photos);
  const moments = useStore((s) => s.moments);
  const versions = useStore((s) => s.thumbVersion);
  const toast = useStore((s) => s.toast);
  const setSelection = useStore((s) => s.setSelection);
  const setPresetDialog = useStore((s) => s.setPresetDialog);
  const setAutoProgress = useStore((s) => s.setAutoProgress);
  const [criterion, setCriterion] = useState<Criterion>("combined");
  const [groups, setGroups] = useState<LookGroup[]>([]);
  const [missing, setMissing] = useState(0);
  const [measuring, setMeasuring] = useState(false);
  const [active, setActive] = useState<string | null>(null);
  const [reference, setReference] = useState<number | null>(null);
  const [opts, setOpts] = useState<MatchOptions>(defaultMatchOptions());
  const fail = useCallback((e: unknown) => toast("error", errorText(e, t)), [toast, t]);

  const load = useCallback(() => {
    api
      .consistencyGroups(criterion)
      .then((r) => {
        setGroups(r.groups);
        setMissing(r.missing);
      })
      .catch(fail);
  }, [criterion, fail]);
  useEffect(load, [load]);

  const measure = async () => {
    setMeasuring(true);
    try {
      await api.measureConsistency();
      load();
    } catch (e) {
      fail(e);
    } finally {
      setMeasuring(false);
    }
  };

  const group = groups.find((g) => g.id === active) ?? groups[0] ?? null;
  // Chaves da fotografia principal (os grupos são por ficheiro).
  const byAsset = useMemo(() => new Map(photos.filter((p) => !p.virtualCopy).map((p) => [p.asset, p])), [photos]);
  const members = (group?.ids ?? []).map((a) => byAsset.get(a)).filter((p) => p !== undefined);
  useEffect(() => {
    if (group && (reference === null || !members.some((p) => p.id === reference))) {
      // Referência por omissão: a primeira fotografia já editada do grupo.
      setReference(members.find((p) => p.edited)?.id ?? null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [group?.id]);

  const sync = async () => {
    if (reference === null || !group) return;
    try {
      const total = await api.startSync(
        reference,
        members.map((p) => p.id),
        opts,
      );
      setAutoProgress({ kind: "sync", total, done: 0, failed: 0, paused: false, cancelled: false, finished: false, current: null });
    } catch (e) {
      fail(e);
    }
  };
  const selectGroup = () => {
    const ids = members.map((p) => p.id);
    if (ids.length) setSelection({ ids: new Set(ids), focus: ids[0]!, anchor: ids[0]! });
    return ids.length;
  };

  if (!project) return null;
  return (
    <div className="consistency">
      <aside className="consistency-groups" aria-label={t("look.groupsTitle")}>
        <label className="small">
          {t("look.criterion")}
          <select value={criterion} onChange={(e) => setCriterion(e.target.value as Criterion)} aria-label={t("look.criterion")}>
            {CRITERIA.map((c) => (
              <option key={c} value={c}>
                {t(`look.criteria.${c}`)}
              </option>
            ))}
          </select>
        </label>
        {missing > 0 && (
          <div className="notice warn small">
            <p>{t("look.missing", { count: missing })}</p>
            <button type="button" className="btn small primary" disabled={measuring} onClick={() => void measure()}>
              {measuring ? t("look.measuring") : t("look.measure")}
            </button>
          </div>
        )}
        <ul>
          {groups.map((g) => (
            <li key={g.id}>
              <button type="button" className={`side-item look-group${group?.id === g.id ? " active" : ""}`} onClick={() => setActive(g.id)}>
                <span className="look-title">{groupTitle(g, t, moments)}</span>
                <span className="count">{g.ids.length}</span>
                <span className="small muted look-sub">
                  {g.code !== "time" && g.start ? `${hhmm(g.start)}–${hhmm(g.end)}` : ""}
                  {g.tags.map((x) => ` · ${t(`look.${x}`)}`).join("")}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </aside>
      <section className="consistency-main" aria-label={group ? groupTitle(group, t, moments) : t("look.groupsTitle")}>
        {group ? (
          <>
            <div className="develop-toolbar">
              <strong>{groupTitle(group, t, moments)}</strong>
              <span className="muted">· {t("look.photos", { count: members.length })}</span>
              <span className="spacer" />
              <button type="button" className="btn small" onClick={() => selectGroup()}>
                {t("look.selectGroup")}
              </button>
              <button type="button" className="btn small" onClick={() => selectGroup() && setPresetDialog(true)}>
                {t("look.presetGroup")}
              </button>
            </div>
            <p className="small muted look-hint">{t("look.referenceHint")}</p>
            <div className="look-grid">
              {members.map((p) => (
                <button
                  type="button"
                  key={p.id}
                  className={`look-cell${reference === p.id ? " reference" : ""}`}
                  aria-pressed={reference === p.id}
                  aria-label={p.fileName}
                  onClick={() => setReference(p.id)}
                  onDoubleClick={() => onEdit(p.id)}
                >
                  <img src={`${imageUrl(project.imageToken, "thumb", p.id)}?v=${versions.get(p.id) ?? 0}`} alt="" draggable={false} decoding="async" />
                  {reference === p.id && <span className="ref-badge">{t("look.reference")}</span>}
                  {p.edited && <span className="cell-edited">✎</span>}
                </button>
              ))}
            </div>
            <div className="look-actions">
              <div className="copy-blocks">
                {(["look", "exposure", "whiteBalance", "skin", "contrast", "color"] as const).map((k) => (
                  <label key={k} className="small">
                    <input type="checkbox" checked={opts[k]} onChange={(e) => setOpts({ ...opts, [k]: e.target.checked })} /> {t(`sync.option.${k}`)}
                  </label>
                ))}
              </div>
              <div className="dev-row">
                <button type="button" className="btn" disabled={reference === null} onClick={() => reference !== null && onEdit(reference)}>
                  {t("look.editReference")}
                </button>
                <button type="button" className="btn primary" disabled={reference === null || members.length < 2 || !Object.values(opts).some(Boolean)} onClick={() => void sync()}>
                  {t("look.syncGroup", { count: Math.max(0, members.length - 1) })}
                </button>
              </div>
              <p className="small muted">{t("sync.smartNote")}</p>
            </div>
          </>
        ) : (
          <div className="empty-state">
            <span className="empty-icon" aria-hidden="true"><Icon name="balance" size={32} /></span>
            <p className="muted">{t("look.empty")}</p>
          </div>
        )}
      </section>
    </div>
  );
}
