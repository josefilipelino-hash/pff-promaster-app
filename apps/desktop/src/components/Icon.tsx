import type { SVGProps } from "react";

/**
 * Iconografia (Fase 14): traço de 1,5 px numa grelha de 16, `currentColor`, nítida em
 * qualquer densidade (SVG). Decorativa por omissão (`aria-hidden`): o nome acessível fica
 * no botão. Desenhos próprios, sem bibliotecas.
 */
const PATHS = {
  grid: "M2.5 2.5h4v4h-4zM9.5 2.5h4v4h-4zM2.5 9.5h4v4h-4zM9.5 9.5h4v4h-4z",
  stack: "M3 5.5h10M4.5 3h7M2.5 8h11v5.5h-11z",
  people: "M6 7.5a2.25 2.25 0 1 0 0-4.5 2.25 2.25 0 0 0 0 4.5zM2 13.5c.4-2.4 2-3.75 4-3.75s3.6 1.35 4 3.75M10.75 7a1.9 1.9 0 1 0 0-3.8M11.5 9.9c1.4.3 2.3 1.45 2.5 3.6",
  sliders: "M2.5 4.5h6M11.5 4.5h2M2.5 11.5h2M7.5 11.5h6M8.5 3v3M4.5 10v3",
  balance: "M8 2.5v11M3 13.5h10M4.5 5l-2 4.5h4zM11.5 5l-2 4.5h4zM4.5 5h7",
  album: "M2.5 3.5h5.5v9H2.5zM8 3.5h5.5v9H8z",
  compare: "M2.5 3.5h4.5v9H2.5zM9 3.5h4.5v9H9z",
  preset: "M3 2.5h10v11H3zM5.5 5.5h5M5.5 8h5M5.5 10.5h3",
  export: "M8 10V2.5M5 5.5l3-3 3 3M3 9.5v4h10v-4",
  import: "M8 2.5V10M5 7l3 3 3-3M3 9.5v4h10v-4",
  file: "M4 2.5h5.5l2.5 2.5v8.5H4zM9.5 2.5V5H12",
  folder: "M2.5 4.5h4l1.5 1.5h5.5v7h-11z",
  close: "M4 4l8 8M12 4l-8 8",
  chevronLeft: "M10 3.5L5.5 8l4.5 4.5",
  chevronRight: "M6 3.5L10.5 8 6 12.5",
  chevronDown: "M3.5 6L8 10.5 12.5 6",
  star: "M8 2.3l1.7 3.6 3.9.5-2.9 2.7.7 3.9L8 11.1 4.6 13l.7-3.9-2.9-2.7 3.9-.5z",
  heart: "M8 13.2S2.5 10 2.5 6.1A2.9 2.9 0 0 1 8 4.6a2.9 2.9 0 0 1 5.5 1.5C13.5 10 8 13.2 8 13.2z",
  reject: "M3.5 3.5l9 9M12.5 3.5l-9 9",
  check: "M3 8.5l3 3 7-7",
  alert: "M8 2.5l6 11H2zM8 6.5v3.5M8 11.75v.5",
  offline: "M2.5 10.5h11v3h-11zM8 2.5v5M5.5 5l2.5 2.5L10.5 5",
  info: "M8 14A6 6 0 1 0 8 2a6 6 0 0 0 0 12zM8 7.25V11M8 5v.5",
  help: "M6 6.2a2 2 0 1 1 2.8 1.8c-.5.2-.8.7-.8 1.2V10M8 12v.5",
  search: "M7 12a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM10.5 10.5l3 3",
  more: "M3.5 8h.01M8 8h.01M12.5 8h.01",
  sun: "M8 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM8 1.5v1.5M8 13v1.5M1.5 8H3M13 8h1.5M3.4 3.4l1.1 1.1M11.5 11.5l1.1 1.1M3.4 12.6l1.1-1.1M11.5 4.5l1.1-1.1",
  moon: "M13 9.5A5.5 5.5 0 0 1 6.5 3a5.5 5.5 0 1 0 6.5 6.5z",
  shield: "M8 2l5 2v4c0 3-2.2 5.2-5 6-2.8-.8-5-3-5-6V4z",
  play: "M5 3.5l7 4.5-7 4.5z",
  pause: "M5 3.5v9M11 3.5v9",
  gallery: "M2.5 3.5h11v9h-11zM2.5 10.5l3-3 2.5 2.5 2-2 3.5 3.5",
  wand: "M3 13l7-7M9 3.5v2M12.5 7h-2M11.5 4.5l-1.25 1.25M12 2.5l.5 1M13.5 4l-1 .5",
  trash: "M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.5 9h6l.5-9",
  eye: "M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8zM8 10a2 2 0 1 0 0-4 2 2 0 0 0 0 4z",
  undo: "M5.5 4L2.5 7l3 3M2.5 7h7a3.5 3.5 0 0 1 0 7H7",
  redo: "M10.5 4l3 3-3 3M13.5 7h-7a3.5 3.5 0 0 0 0 7H9",
  reset: "M3 8a5 5 0 1 0 1.5-3.6M3 2.5v2.5h2.5",
  beforeAfter: "M8 2.5v11M2.5 3.5h11v9h-11z",
  copy: "M5.5 5.5h7.5v8H5.5zM3 10.5V2.5h7.5",
  paste: "M5.5 3h-2v10.5h9V3h-2M5.5 2.5h5v2h-5z",
  sync: "M13 6.5A5 5 0 0 0 3.6 5M3 9.5a5 5 0 0 0 9.4 1.5M3.5 2.5V5H6M12.5 13.5V11H10",
  duplicate: "M2.5 5.5h8v8h-8zM5.5 5.5V2.5h8v8h-3",
  plus: "M8 3v10M3 8h10",
  expand: "M2.5 6V2.5H6M10 2.5h3.5V6M13.5 10v3.5H10M6 13.5H2.5V10",
  zoomIn: "M7 12a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM10.5 10.5l3 3M5 7h4M7 5v4",
  zoomOut: "M7 12a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM10.5 10.5l3 3M5 7h4",
  keyboard: "M1.5 4.5h13v7h-13zM4 7h.01M6.5 7h.01M9 7h.01M11.5 7h.01M5 9.5h6",
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name, size = 16, title, ...rest }: { name: IconName; size?: number; title?: string } & SVGProps<SVGSVGElement>) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      // Traço visual constante (~1,5 px a 16, ~2 px a 32): ícones grandes não ficam pesados.
      strokeWidth={size > 20 ? (2 * 16) / size : 1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden={title ? undefined : true}
      role={title ? "img" : undefined}
      className="icon"
      focusable="false"
      {...rest}
    >
      {title && <title>{title}</title>}
      <path d={PATHS[name]} />
    </svg>
  );
}
