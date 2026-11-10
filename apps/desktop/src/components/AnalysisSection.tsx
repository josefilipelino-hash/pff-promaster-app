import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { explain, parseReasons } from "../logic/explain";
import { useStore } from "../store";
import type { PhotoRow, RecommendationRow } from "../types";
import { LabelChip } from "./LabelChip";

type ScoreKey = "overall" | "technical" | "focus" | "motion" | "exposure" | "noise" | "people" | "eyes" | "expression" | "composition";
const SCORES: ScoreKey[] = ["overall", "technical", "focus", "motion", "exposure", "noise", "people", "eyes", "expression", "composition"];

function pct(v: number): string {
  return `${Math.round(v * 100)}`;
}

function ScoreBar({ label, value }: { label: string; value: number | null }) {
  const { t } = useTranslation();
  return (
    <div className="score-row">
      <span className="score-name">{label}</span>
      {value === null ? (
        <span className="muted small score-na">{t("analysis.notApplicable")}</span>
      ) : (
        <>
          <span className="score-bar" aria-hidden>
            <span className={`score-fill${value < 0.4 ? " low" : value < 0.7 ? " mid" : ""}`} style={{ width: `${Math.max(2, value * 100)}%` }} />
          </span>
          <span className="score-value">{pct(value)}</span>
        </>
      )}
    </div>
  );
}

/** Secção “Análise” do painel de informação: etiqueta, explicação e pontuações individuais. */
export function AnalysisSection({ photo, onCompareGroup, onAccept }: { photo: PhotoRow; onCompareGroup: (ids: number[]) => void; onAccept: (id: number) => void }) {
  const { t, i18n } = useTranslation();
  const asset = photo.asset ?? photo.id;
  const d = useStore((s) => s.analysisDetails.get(asset));
  const brief = useStore((s) => s.recs.get(asset));
  const group = useStore((s) => (brief?.groupId != null ? s.groupById.get(brief.groupId) : undefined));
  const analysisLoaded = useStore((s) => s.analysis !== null);
  const load = useStore((s) => s.loadAnalysisDetails);

  useEffect(() => {
    if (analysisLoaded) void load(asset).catch(() => {});
  }, [asset, brief, analysisLoaded, load]);

  if (!analysisLoaded) return null;
  if (!d || (!d.analysis && !d.error)) {
    return (
      <section className="analysis-section">
        <h4>{t("analysis.title")}</h4>
        <p className="muted small">{t("analysis.notAnalyzed")}</p>
      </section>
    );
  }
  if (d.error && !d.analysis) {
    return (
      <section className="analysis-section">
        <h4>{t("analysis.title")}</h4>
        <p className="notice warn small">{t("analysis.failed", { detail: d.error })}</p>
      </section>
    );
  }
  const r: RecommendationRow | null = d.recommendation;
  const a = d.analysis!;
  const reasons = parseReasons(r?.reasons);
  const canAccept = r && photo.flag === 0 && (r.label === "best" || r.label === "reject");
  return (
    <section className="analysis-section" aria-label={t("analysis.title")}>
      <h4>{t("analysis.title")}</h4>
      {r && (
        <>
          <div className="analysis-head">
            <LabelChip label={r.label} protectedByUser={photo.flag === 1 || photo.stars > 0} />
            {group && <span className="muted small">{t(`analysis.group.${group.kind}`, { rank: r.rank + 1, count: group.members.length })}</span>}
          </div>
          <p className="analysis-why">{explain(r.summary, reasons, t, i18n.language)}</p>
          <div className="analysis-actions">
            {canAccept && (
              <button type="button" className="btn small" onClick={() => onAccept(asset)} title={t("analysis.acceptHint")}>
                {r.label === "best" ? t("analysis.acceptPick") : t("analysis.acceptReject")}
              </button>
            )}
            {group && group.members.length > 1 && (
              <button
                type="button"
                className="btn small"
                onClick={() => {
                  const others = group.members.filter((m) => m !== asset).slice(0, 3);
                  onCompareGroup([asset, ...others]);
                }}
              >
                {t("analysis.compareGroup")}
              </button>
            )}
          </div>
          <div className="scores">
            {SCORES.map((k) => (
              <ScoreBar key={k} label={t(`score.${k}`)} value={r[k]} />
            ))}
          </div>
        </>
      )}
      <dl className="info-list analysis-facts">
        <div>
          <dt>{t("analysis.faces")}</dt>
          <dd>{d.faces.length === 0 ? t("analysis.noFaces") : String(d.faces.length)}</dd>
        </div>
        {d.faces.slice(0, 6).map((f, i) => (
          <div key={i}>
            <dt>{t("analysis.face", { n: i + 1 })}</dt>
            <dd>
              {f.px < 48
                ? t("analysis.faceSmall", { px: Math.round(f.px) })
                : t("analysis.faceDetail", {
                    px: Math.round(f.px),
                    eyes: pct(Math.max(f.blinkLeft, f.blinkRight)),
                    smile: pct(f.smile),
                  })}
              {f.cut ? ` · ${t("analysis.faceCut")}` : ""}
            </dd>
          </div>
        ))}
        <div>
          <dt>{t("analysis.highlights")}</dt>
          <dd>{(a.highlightsClipped * 100).toLocaleString(i18n.language, { maximumFractionDigits: 1 })} %</dd>
        </div>
        <div>
          <dt>{t("analysis.shadows")}</dt>
          <dd>{(a.shadowsCrushed * 100).toLocaleString(i18n.language, { maximumFractionDigits: 1 })} %</dd>
        </div>
        {a.tiltDegrees !== null && (
          <div>
            <dt>{t("analysis.tilt")}</dt>
            <dd>{a.tiltDegrees.toLocaleString(i18n.language, { maximumFractionDigits: 1 })}°</dd>
          </div>
        )}
        <div>
          <dt>{t("analysis.engine")}</dt>
          <dd>{a.models ? t("analysis.engineAi") : t("analysis.engineTechnical")}</dd>
        </div>
      </dl>
    </section>
  );
}
