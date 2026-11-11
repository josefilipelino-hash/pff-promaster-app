import { useTranslation } from "react-i18next";
import { useStore, type View } from "../store";
import { Icon, type IconName } from "./Icon";

type Step = "import" | "select" | "edit" | "deliver";

/**
 * Fluxo do trabalho (pedido do utilizador: design "prático e eficiente" para o seu fluxo):
 * Importar → Selecionar → Editar → Entregar, com o progresso real de cada passo e um clique
 * para ir para ele. O passo seguinte por fazer fica destacado.
 */
export function WorkflowBar({
  view,
  onView,
  onImport,
  onExport,
}: {
  view: View;
  onView: (v: View) => void;
  onImport: () => void;
  onExport: () => void;
}) {
  const { t } = useTranslation();
  const photos = useStore((s) => s.photos);
  const analysis = useStore((s) => s.analysis);
  const total = photos.filter((p) => !p.virtualCopy).length;
  const picked = photos.filter((p) => p.flag === 1 || p.stars > 0).length;
  const edited = photos.filter((p) => p.edited).length;
  const analyzed = analysis?.counts.analyzed ?? 0;
  const done: Record<Step, boolean> = {
    import: total > 0,
    select: total > 0 && (analyzed >= total || picked > 0),
    edit: edited > 0,
    deliver: false,
  };
  const next: Step = !done.import ? "import" : !done.select ? "select" : !done.edit ? "edit" : "deliver";
  const steps: { id: Step; icon: IconName; detail: string; active: boolean; go: () => void }[] = [
    { id: "import", icon: "import", detail: t("workflow.importDetail", { count: total }), active: false, go: onImport },
    {
      id: "select",
      icon: "star",
      detail: analyzed > 0 ? t("workflow.selectDetail", { count: picked, analyzed }) : t("workflow.selectStart"),
      active: view === "grid" || view === "sequences" || view === "people",
      go: () => onView("grid"),
    },
    { id: "edit", icon: "sliders", detail: t("workflow.editDetail", { count: edited }), active: view === "develop" || view === "consistency", go: () => onView("develop") },
    { id: "deliver", icon: "export", detail: t("workflow.deliverDetail"), active: view === "album", go: onExport },
  ];
  return (
    <nav className="workflow" aria-label={t("workflow.label")}>
      {steps.map((s, i) => (
        <button
          key={s.id}
          type="button"
          className={`wf-step${s.active ? " active" : ""}${done[s.id] ? " done" : ""}${next === s.id ? " next" : ""}`}
          aria-current={s.active ? "step" : undefined}
          onClick={s.go}
          title={t(`workflow.hint.${s.id}`)}
        >
          <span className="wf-num" aria-hidden="true">
            {done[s.id] ? <Icon name="check" size={12} /> : i + 1}
          </span>
          <span className="wf-text">
            <span className="wf-name">{t(`workflow.step.${s.id}`)}</span>
            <span className="wf-detail">{s.detail}</span>
          </span>
          <Icon name={s.icon} size={14} />
        </button>
      ))}
    </nav>
  );
}
