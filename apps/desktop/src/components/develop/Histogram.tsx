import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

/** Histograma RGB do último render (escala raiz quadrada, como os editores profissionais). */
export function Histogram({ data }: { data: [Uint32Array, Uint32Array, Uint32Array] | null }) {
  const { t } = useTranslation();
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = ref.current;
    const ctx = c?.getContext?.("2d");
    if (!c || !ctx) return;
    const w = c.width;
    const h = c.height;
    ctx.clearRect(0, 0, w, h);
    if (!data) return;
    // Ignora os extremos (0 e 255) para a escala, para os picos de recorte não esmagarem o resto.
    let max = 1;
    for (const ch of data) for (let i = 1; i < 255; i++) max = Math.max(max, ch[i]!);
    const colors = ["rgba(229,72,77,0.65)", "rgba(70,167,88,0.65)", "rgba(62,142,208,0.65)"];
    ctx.globalCompositeOperation = "lighter";
    data.forEach((ch, k) => {
      ctx.fillStyle = colors[k]!;
      ctx.beginPath();
      ctx.moveTo(0, h);
      for (let i = 0; i < 256; i++) {
        const v = Math.sqrt(Math.min(1, ch[i]! / max));
        ctx.lineTo((i / 255) * w, h - v * h);
      }
      ctx.lineTo(w, h);
      ctx.closePath();
      ctx.fill();
    });
    ctx.globalCompositeOperation = "source-over";
  }, [data]);
  const clipped = data ? [data[0]![255]! + data[1]![255]! + data[2]![255]!, data[0]![0]! + data[1]![0]! + data[2]![0]!] : [0, 0];
  return (
    <div className="dev-histogram" aria-label={t("develop.histogram")} role="img">
      <canvas ref={ref} width={256} height={96} />
      <div className="dev-clip small muted">
        <span className={clipped[1]! > 0 ? "warn" : ""}>{t("develop.clipShadows")}</span>
        <span className={clipped[0]! > 0 ? "warn" : ""}>{t("develop.clipHighlights")}</span>
      </div>
    </div>
  );
}
