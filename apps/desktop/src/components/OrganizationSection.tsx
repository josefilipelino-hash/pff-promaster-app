import { useTranslation } from "react-i18next";
import { useStore } from "../store";
import type { PhotoRow } from "../types";
import { momentName } from "./MomentsPanel";
import { personName } from "./PeoplePanel";

/** Momento e pessoas da fotografia, no painel de informação. */
export function OrganizationSection({ photo, onMoment, onReset, onPerson }: { photo: PhotoRow; onMoment: (moment: number | null) => void; onReset: () => void; onPerson: (id: number) => void }) {
  const { t } = useTranslation();
  const moments = useStore((s) => s.moments);
  const am = useStore((s) => s.assetMoments.get(photo.asset ?? photo.id));
  const ids = useStore((s) => s.photoPersons.get(photo.asset ?? photo.id));
  const personById = useStore((s) => s.personById);
  if (moments.length === 0) return null;
  const current = am ? (am.momentId === null ? "none" : String(am.momentId)) : "none";
  return (
    <section className="analysis-section" aria-label={t("organization.title")}>
      <h4>{t("organization.title")}</h4>
      <label className="field compact org-moment">
        <span className="small muted">{t("organization.moment")}</span>
        <select value={current} onChange={(e) => onMoment(e.target.value === "none" ? null : Number(e.target.value))}>
          <option value="none">{t("moments.unclassified")}</option>
          {moments.map((m) => (
            <option key={m.id} value={m.id}>
              {momentName(m, t)}
            </option>
          ))}
        </select>
      </label>
      <p className="small muted">
        {!am
          ? t("organization.notClassified")
          : am.source === "user"
            ? t("organization.byYou")
            : t("organization.auto", { confidence: Math.round((am.confidence ?? 0) * 100) })}
        {am?.source === "user" && (
          <>
            {" · "}
            <button type="button" className="link-button" onClick={onReset}>
              {t("organization.backToAuto")}
            </button>
          </>
        )}
      </p>
      {ids && ids.length > 0 && (
        <div className="person-chips">
          {ids.map((id) => (
            <button key={id} type="button" className="person-chip" onClick={() => onPerson(id)} title={t("organization.showPerson")}>
              {personName(personById.get(id), t)}
            </button>
          ))}
        </div>
      )}
    </section>
  );
}
