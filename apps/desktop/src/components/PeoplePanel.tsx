import { useTranslation } from "react-i18next";
import { useStore, type View } from "../store";
import type { PersonRow, Role } from "../types";
import { FaceThumb } from "./FaceThumb";

export function personName(p: PersonRow | undefined, t: (k: string, o?: Record<string, unknown>) => string): string {
  if (!p) return "";
  if (p.name) return p.name;
  if (p.role) return t(`role.${p.role}`);
  return t("people.unnamed", { id: p.id });
}

/** Pesquisas rápidas por papel: [chave, papéis que têm de aparecer juntos]. */
export const QUICK_SEARCHES: [string, Role[]][] = [
  ["bride", ["bride"]],
  ["groom", ["groom"]],
  ["couple", ["bride", "groom"]],
  ["brideMother", ["bride", "bride_mother"]],
  ["groomFather", ["groom", "groom_father"]],
];

/** Ids das pessoas com cada papel (a primeira pessoa com esse papel). */
export function personsForRoles(persons: PersonRow[], roles: Role[]): number[] | null {
  const ids: number[] = [];
  for (const r of roles) {
    const p = persons.find((x) => x.role === r && !x.hidden);
    if (!p) return null;
    ids.push(p.id);
  }
  return ids;
}

export function PeoplePanel({ active, onPersons, view, onView }: { active: number[]; onPersons: (ids: number[]) => void; view: View; onView: (v: View) => void }) {
  const { t } = useTranslation();
  const persons = useStore((s) => s.persons);
  const named = persons.filter((p) => !p.hidden && (p.name || p.role));
  const same = (ids: number[]) => ids.length === active.length && ids.every((x) => active.includes(x));
  return (
    <section className="people-panel" aria-label={t("people.title")}>
      <h3 className="sidebar-title">{t("people.title")}</h3>
      {persons.length === 0 ? (
        <p className="small muted side-note">{t("people.empty")}</p>
      ) : (
        <ul>
          {QUICK_SEARCHES.map(([key, roles]) => {
            const ids = personsForRoles(persons, roles);
            return (
              <li key={key}>
                <button
                  type="button"
                  className={`side-item${ids && same(ids) ? " active" : ""}`}
                  disabled={!ids}
                  title={ids ? undefined : t("people.needRoles")}
                  aria-pressed={!!ids && same(ids)}
                  onClick={() => ids && onPersons(same(ids) ? [] : ids)}
                >
                  <span>{t(`people.quick.${key}`)}</span>
                </button>
              </li>
            );
          })}
          {named.map((p) => (
            <li key={p.id}>
              <button type="button" className={`side-item person-item${same([p.id]) ? " active" : ""}`} aria-pressed={same([p.id])} onClick={() => onPersons(same([p.id]) ? [] : [p.id])}>
                <span className="person-label">
                  <FaceThumb face={p.cover} size={22} />
                  <span>{personName(p, t)}</span>
                </span>
                <span className="count">{p.photoCount.toLocaleString()}</span>
              </button>
            </li>
          ))}
          <li>
            <button type="button" className={`side-item${view === "people" ? " active" : ""}`} aria-pressed={view === "people"} onClick={() => onView(view === "people" ? "grid" : "people")}>
              <span>{t("people.manage")}</span>
              <span className="count">{persons.length.toLocaleString()}</span>
            </button>
          </li>
        </ul>
      )}
    </section>
  );
}
