import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";

type Pt = [number, number];
type Channel = "rgb" | "red" | "green" | "blue";

/** Interpolação cúbica monótona (igual à do núcleo) para desenhar a curva. */
export function monotone(points: Pt[]): (x: number) => number {
  const p = [...points].sort((a, b) => a[0] - b[0]);
  if (p.length < 2) return (x) => x;
  const n = p.length;
  const d = Array.from({ length: n - 1 }, (_, i) => (p[i + 1]![1] - p[i]![1]) / Math.max(1e-6, p[i + 1]![0] - p[i]![0]));
  const m = Array.from({ length: n }, (_, i) => (i === 0 ? d[0]! : i === n - 1 ? d[n - 2]! : d[i - 1]! * d[i]! <= 0 ? 0 : (d[i - 1]! + d[i]!) / 2));
  for (let i = 0; i < n - 1; i++) {
    if (d[i] === 0) {
      m[i] = 0;
      m[i + 1] = 0;
    } else {
      const a = m[i]! / d[i]!;
      const b = m[i + 1]! / d[i]!;
      const s = a * a + b * b;
      if (s > 9) {
        const t = 3 / Math.sqrt(s);
        m[i] = t * a * d[i]!;
        m[i + 1] = t * b * d[i]!;
      }
    }
  }
  return (x) => {
    if (x <= p[0]![0]) return p[0]![1];
    if (x >= p[n - 1]![0]) return p[n - 1]![1];
    let i = 0;
    while (i < n - 2 && x > p[i + 1]![0]) i++;
    const h = p[i + 1]![0] - p[i]![0];
    const t = (x - p[i]![0]) / h;
    const t2 = t * t;
    const t3 = t2 * t;
    return (2 * t3 - 3 * t2 + 1) * p[i]![1] + (t3 - 2 * t2 + t) * h * m[i]! + (-2 * t3 + 3 * t2) * p[i + 1]![1] + (t3 - t2) * h * m[i + 1]!;
  };
}

const IDENTITY: Pt[] = [
  [0, 0],
  [1, 1],
];

/**
 * Curva de pontos: clicar acrescenta um ponto, arrastar move, duplo clique remove.
 * Canais RGB, vermelho, verde e azul.
 */
export function CurveEditor({ curves, onChange }: { curves: Record<Channel, Pt[]>; onChange: (ch: Channel, pts: Pt[], done: boolean) => void }) {
  const { t } = useTranslation();
  const [ch, setCh] = useState<Channel>("rgb");
  const svg = useRef<SVGSVGElement>(null);
  const drag = useRef<number | null>(null);
  const pts = curves[ch].length >= 2 ? curves[ch] : IDENTITY;
  const f = monotone(pts);
  const S = 200;
  const pos = (e: { clientX: number; clientY: number }): Pt => {
    const r = svg.current!.getBoundingClientRect();
    const w = r.width || S;
    const h = r.height || S;
    return [Math.min(1, Math.max(0, (e.clientX - r.left) / w)), Math.min(1, Math.max(0, 1 - (e.clientY - r.top) / h))];
  };
  const path = Array.from({ length: 65 }, (_, i) => {
    const x = i / 64;
    return `${i ? "L" : "M"}${x * S},${(1 - Math.min(1, Math.max(0, f(x)))) * S}`;
  }).join(" ");
  const color = { rgb: "#ddd", red: "#e5484d", green: "#46a758", blue: "#3e8ed0" }[ch];
  return (
    <div className="curve-editor">
      <div className="segmented small" role="group" aria-label={t("develop.curveChannel")}>
        {(["rgb", "red", "green", "blue"] as Channel[]).map((c) => (
          <button key={c} type="button" className={`btn small${ch === c ? " active" : ""}`} aria-pressed={ch === c} onClick={() => setCh(c)}>
            {t(`develop.channel.${c}`)}
          </button>
        ))}
      </div>
      <svg
        ref={svg}
        viewBox={`0 0 ${S} ${S}`}
        className="curve-svg"
        role="img"
        aria-label={t("develop.pointCurve")}
        onPointerDown={(e) => {
          if (e.target instanceof SVGCircleElement) return;
          const p = pos(e);
          const next = [...pts, p].sort((a, b) => a[0] - b[0]);
          drag.current = next.findIndex((q) => q === p);
          (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
          onChange(ch, next, false);
        }}
        onPointerMove={(e) => {
          if (drag.current === null) return;
          const p = pos(e);
          const next = pts.map((q, i) => (i === drag.current ? p : q));
          onChange(ch, next, false);
        }}
        onPointerUp={() => {
          if (drag.current !== null) onChange(ch, pts, true);
          drag.current = null;
        }}
      >
        {[0.25, 0.5, 0.75].map((g) => (
          <g key={g} stroke="#333">
            <line x1={g * S} x2={g * S} y1={0} y2={S} />
            <line y1={g * S} y2={g * S} x1={0} x2={S} />
          </g>
        ))}
        <line x1={0} y1={S} x2={S} y2={0} stroke="#444" strokeDasharray="3 3" />
        <path d={path} fill="none" stroke={color} strokeWidth={2} />
        {pts.map((p, i) => (
          <circle
            key={i}
            cx={p[0] * S}
            cy={(1 - p[1]) * S}
            r={5}
            fill={color}
            onPointerDown={(e) => {
              e.stopPropagation();
              drag.current = i;
              (e.currentTarget.ownerSVGElement as Element | null)?.setPointerCapture?.(e.pointerId);
            }}
            onDoubleClick={() => {
              if (pts.length > 2) onChange(ch, pts.filter((_, k) => k !== i), true);
            }}
          />
        ))}
      </svg>
      <button type="button" className="btn small ghost" onClick={() => onChange(ch, [], true)}>
        {t("develop.resetCurve")}
      </button>
    </div>
  );
}
