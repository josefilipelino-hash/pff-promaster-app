import { useTranslation } from "react-i18next";
import { AUTO_INTENSITIES, type AiDenoiseMode, type AutoOptions, type AutoReport } from "../../types";
import { Slider } from "./Slider";

/** Grupos de ajustes que se podem desligar na intensidade "Personalizada". */
const GROUPS = ["tone", "whiteBalance", "color", "sharpen", "denoise", "lens", "straighten"] as const;
/** Extras opcionais (valem para todas as intensidades). */
const EXTRAS = ["crop", "redEye", "dust"] as const;

/** Opções da edição automática (editor e "Editar automaticamente seleção"). */
export function AutoOptionsForm({ options, onChange, aiAvailable }: { options: AutoOptions; onChange: (o: AutoOptions) => void; aiAvailable: boolean }) {
  const { t } = useTranslation();
  const custom = options.intensity === "custom";
  return (
    <div className="auto-options">
      <div className="segmented small wrap" role="group" aria-label={t("auto.intensity")}>
        {AUTO_INTENSITIES.map((i) => (
          <button
            key={i}
            type="button"
            className={`btn small${options.intensity === i ? " active" : ""}`}
            aria-pressed={options.intensity === i}
            title={t(`auto.intensityHint.${i}`)}
            onClick={() =>
              onChange(
                i === "custom"
                  ? { ...options, intensity: i }
                  : // As intensidades predefinidas ajustam todos os grupos.
                    { ...options, intensity: i, tone: true, whiteBalance: true, color: true, sharpen: true, denoise: true, lens: true, straighten: true },
              )
            }
          >
            {t(`auto.intensities.${i}`)}
          </button>
        ))}
      </div>
      {custom && (
        <>
          <Slider label={t("auto.strength")} value={options.strength} min={0} max={150} def={100} onChange={(v) => onChange({ ...options, strength: v })} />
          <fieldset className="auto-groups">
            <legend className="small muted">{t("auto.groupsTitle")}</legend>
            {GROUPS.map((g) => (
              <label key={g} className="small">
                <input type="checkbox" checked={options[g]} onChange={(e) => onChange({ ...options, [g]: e.target.checked })} /> {t(`auto.groups.${g}`)}
              </label>
            ))}
          </fieldset>
        </>
      )}
      <fieldset className="auto-groups">
        <legend className="small muted">{t("auto.extrasTitle")}</legend>
        {EXTRAS.map((g) => (
          <label key={g} className="small" title={t(`auto.extrasHint.${g}`)}>
            <input type="checkbox" checked={options[g]} onChange={(e) => onChange({ ...options, [g]: e.target.checked })} /> {t(`auto.extras.${g}`)}
          </label>
        ))}
      </fieldset>
      <label className="small auto-ai">
        {t("auto.aiDenoise")}
        <select
          value={aiAvailable ? options.aiDenoise : "off"}
          disabled={!aiAvailable}
          onChange={(e) => onChange({ ...options, aiDenoise: e.target.value as AiDenoiseMode })}
          aria-label={t("auto.aiDenoise")}
        >
          {(["off", "whenNeeded", "always"] as const).map((m) => (
            <option key={m} value={m}>
              {t(`auto.aiModes.${m}`)}
            </option>
          ))}
        </select>
      </label>
      {!aiAvailable && <p className="small muted dev-note">{t("auto.aiMissing")}</p>}
    </div>
  );
}

const fmt = (v: number, digits = 0) => (v > 0 ? "+" : v < 0 ? "−" : "") + Math.abs(v).toFixed(digits).replace(".", ",");

/** O que a edição automática mediu e fez nesta fotografia. */
export function AutoReportView({ report, onApplyAi }: { report: AutoReport; onApplyAi?: () => void }) {
  const { t } = useTranslation();
  const n = report.noise;
  const lines = [
    t("auto.report.exposure", { value: fmt(report.exposure, 2) }),
    t("auto.report.wb", { temperature: fmt(report.temperature), tint: fmt(report.tint) }),
    n.iso ? t("auto.report.noiseIso", { level: t(`auto.noise.${n.level}`), iso: n.iso }) : t("auto.report.noise", { level: t(`auto.noise.${n.level}`) }),
  ];
  if (report.tilt !== null && Math.abs(report.tilt) >= 0.25) lines.push(t("auto.report.tilt", { value: Math.abs(report.tilt).toFixed(1).replace(".", ",") }));
  if (report.cropped) lines.push(t("auto.report.cropped"));
  if (report.faces > 0) lines.push(t("auto.report.faces", { count: report.faces }));
  if (report.redEyes > 0) lines.push(t("auto.report.redEyes", { count: report.redEyes }));
  if (report.spots > 0) lines.push(t("auto.report.spots", { count: report.spots }));
  return (
    <div className="auto-report" role="status">
      <ul className="small">
        {lines.map((l) => (
          <li key={l}>{l}</li>
        ))}
      </ul>
      {n.aiRecommended && onApplyAi && (
        <p className="small">
          {t("auto.aiRecommended")}{" "}
          <button type="button" className="btn small" onClick={onApplyAi}>
            {t("auto.applyAi")}
          </button>
        </p>
      )}
    </div>
  );
}
