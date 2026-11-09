import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import { errorText, useStore } from "../store";
import { ROLES, type FaceRef, type PersonRow, type Role } from "../types";
import { FaceThumb } from "./FaceThumb";
import { personName } from "./PeoplePanel";

function PersonCard({ p, selected, onToggle, onShow, onChanged }: { p: PersonRow; selected: boolean; onToggle: () => void; onShow: () => void; onChanged: () => void }) {
  const { t } = useTranslation();
  const toast = useStore((s) => s.toast);
  const [faces, setFaces] = useState<FaceRef[]>([]);
  const [name, setName] = useState(p.name ?? "");
  const fail = (e: unknown) => toast("error", errorText(e, t));
  useEffect(() => {
    api.personFaces(p.id, 12).then(setFaces).catch(() => setFaces([]));
  }, [p.id, p.faceCount]);
  useEffect(() => setName(p.name ?? ""), [p.name]);
  const save = async (patch: { name?: string | null; role?: Role | null; hidden?: boolean }) => {
    try {
            // O nome vem do campo (pode ainda não ter chegado ao núcleo quando se muda o papel).
      const current = patch.name !== undefined ? patch.name : name.trim() || null;
      await api.updatePerson(p.id, current, patch.role !== undefined ? patch.role : p.role, patch.hidden ?? p.hidden);
      onChanged();
    } catch (e) {
      fail(e);
    }
  };
  const exclude = async (f: FaceRef) => {
    try {
      await api.excludeFaces([f.faceId]);
      setFaces((l) => l.filter((x) => x.faceId !== f.faceId));
      onChanged();
    } catch (e) {
      fail(e);
    }
  };
  return (
    <article className={`person-card${selected ? " selected" : ""}${p.hidden ? " hidden-person" : ""}`} aria-label={personName(p, t)}>
      <header>
        <label className="person-select">
          <input type="checkbox" checked={selected} onChange={onToggle} aria-label={t("people.select")} />
        </label>
        <FaceThumb face={p.cover} size={48} />
        <div className="person-fields">
          <input
            value={name}
            placeholder={t("people.namePlaceholder")}
            aria-label={t("people.name")}
            onChange={(e) => setName(e.target.value)}
            onBlur={() => name !== (p.name ?? "") && void save({ name: name.trim() || null })}
            onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
          />
          <select value={p.role ?? ""} aria-label={t("people.role")} onChange={(e) => void save({ role: (e.target.value || null) as Role | null })}>
            <option value="">{t("people.noRole")}</option>
            {ROLES.map((r) => (
              <option key={r} value={r}>
                {t(`role.${r}`)}
              </option>
            ))}
          </select>
        </div>
        <span className="muted small">{t("people.photos", { count: p.photoCount })}</span>
      </header>
      <div className="person-faces">
        {faces.map((f) => (
          <span key={f.faceId} className={`person-face${f.source === 1 ? " confirmed" : ""}`}>
            <FaceThumb face={f} size={64} />
            <button type="button" className="face-x" title={t("people.notThisPerson")} aria-label={t("people.notThisPerson")} onClick={() => void exclude(f)}>
              ×
            </button>
          </span>
        ))}
      </div>
      <footer>
        <button type="button" className="btn small" onClick={onShow}>
          {t("people.showPhotos")}
        </button>
        <label className="small">
          <input type="checkbox" checked={p.hidden} onChange={(e) => void save({ hidden: e.target.checked })} /> {t("people.hide")}
        </label>
      </footer>
    </article>
  );
}

/** Gestão de pessoas: identificar (nome + papel), juntar grupos, excluir rostos errados. */
export function PeopleView({ onShowPerson }: { onShowPerson: (id: number) => void }) {
  const { t } = useTranslation();
  const persons = useStore((s) => s.persons);
  const reload = useStore((s) => s.reloadOrganization);
  const toast = useStore((s) => s.toast);
  const [sel, setSel] = useState<number[]>([]);
  const [showHidden, setShowHidden] = useState(false);
  const fail = (e: unknown) => toast("error", errorText(e, t));
    // Ordem estável enquanto a vista está aberta (identificar alguém não o faz saltar de sítio).
  const order = useRef<number[]>([]);
  for (const p of persons) if (!order.current.includes(p.id)) order.current.push(p.id);
  const rank = (id: number) => order.current.indexOf(id);
  const list = persons.filter((p) => showHidden || !p.hidden).sort((a, b) => rank(a.id) - rank(b.id));
  const merge = async () => {
    if (sel.length < 2) return;
    // Junta tudo na pessoa identificada (ou na maior).
    const chosen = persons.filter((p) => sel.includes(p.id));
    const into = (chosen.find((p) => p.name || p.role) ?? chosen.sort((a, b) => b.photoCount - a.photoCount)[0])!;
    try {
      for (const p of chosen) if (p.id !== into.id) await api.mergePersons(p.id, into.id);
      setSel([]);
      await reload();
      toast("info", t("people.merged", { count: chosen.length }));
    } catch (e) {
      fail(e);
    }
  };
  const recluster = async () => {
    try {
      await api.reclusterPeople();
      await reload();
    } catch (e) {
      fail(e);
    }
  };
  if (persons.length === 0) {
    return (
      <div className="empty-state">
        <p className="muted">{t("people.emptyView")}</p>
      </div>
    );
  }
  return (
    <div className="people-view">
      <div className="sequences-bar">
        <span className="muted small">{t("people.count", { count: list.length })}</span>
        <span className="muted small">{t("people.privacy")}</span>
        <span className="spacer" />
        <button type="button" className="btn small" disabled={sel.length < 2} onClick={() => void merge()}>
          {t("people.merge", { count: sel.length })}
        </button>
        <button type="button" className="btn small ghost" onClick={() => void recluster()} title={t("people.reclusterHint")}>
          {t("people.recluster")}
        </button>
        <label className="small">
          <input type="checkbox" checked={showHidden} onChange={(e) => setShowHidden(e.target.checked)} /> {t("people.showHidden")}
        </label>
      </div>
      <div className="people-grid">
        {list.map((p) => (
          <PersonCard key={p.id} p={p} selected={sel.includes(p.id)} onToggle={() => setSel((s) => (s.includes(p.id) ? s.filter((x) => x !== p.id) : [...s, p.id]))} onShow={() => onShowPerson(p.id)} onChanged={() => void reload()} />
        ))}
      </div>
    </div>
  );
}
