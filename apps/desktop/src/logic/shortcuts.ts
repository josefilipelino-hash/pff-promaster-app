import type { Color } from "../types";

/** Ações de teclado. Ver a ajuda (tecla ?) e DEVELOPMENT.md. */
export type Action =
  | { type: "stars"; value: number }
  | { type: "flag"; value: -1 | 0 | 1 }
  | { type: "color"; value: Color }
  | { type: "move"; dx: number; dy: number; extend: boolean }
  | { type: "open" }
  | { type: "close" }
  | { type: "toggleZoom" }
  | { type: "zoom"; dir: 1 | -1 }
  | { type: "fullscreen" }
  | { type: "info" }
  | { type: "selectAll" }
  | { type: "import" }
  | { type: "help" }
    | { type: "compare" }
  | { type: "develop" };

export interface KeyLike {
  key: string;
  shiftKey: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
}

const COLOR_KEYS: Record<string, Color> = { "6": "red", "7": "yellow", "8": "green", "9": "blue" };

export function keyToAction(e: KeyLike, mac: boolean): Action | null {
  const mod = mac ? e.metaKey : e.ctrlKey;
  if (mod && !e.altKey) {
    if (e.key.toLowerCase() === "a" && !e.shiftKey) return { type: "selectAll" };
    if (e.key.toLowerCase() === "i" && e.shiftKey) return { type: "import" };
    return null;
  }
  if (e.metaKey || e.ctrlKey || e.altKey) return null;
  const k = e.key;
  if (k >= "0" && k <= "5" && k.length === 1) return { type: "stars", value: Number(k) };
  if (COLOR_KEYS[k]) return { type: "color", value: COLOR_KEYS[k]! };
  switch (k.toLowerCase()) {
    case "p":
      return { type: "flag", value: 1 };
    case "x":
      return { type: "flag", value: -1 };
    case "u":
      return { type: "flag", value: 0 };
    case "f":
      return { type: "fullscreen" };
    case "i":
      return { type: "info" };
    case "e":
    case "enter":
      return { type: "open" };
    case "g":
    case "escape":
      return { type: "close" };
    case "z":
    case " ":
      return { type: "toggleZoom" };
    case "+":
    case "=":
      return { type: "zoom", dir: 1 };
    case "-":
      return { type: "zoom", dir: -1 };
    case "?":
      return { type: "help" };
        case "c":
      return { type: "compare" };
    case "d":
      return { type: "develop" };
    case "arrowleft":
      return { type: "move", dx: -1, dy: 0, extend: e.shiftKey };
    case "arrowright":
      return { type: "move", dx: 1, dy: 0, extend: e.shiftKey };
    case "arrowup":
      return { type: "move", dx: 0, dy: -1, extend: e.shiftKey };
    case "arrowdown":
      return { type: "move", dx: 0, dy: 1, extend: e.shiftKey };
  }
  return null;
}

/** Lista para o ecrã de ajuda: [teclas, chave de tradução da descrição]. */
export const SHORTCUT_HELP: [string, string][] = [
  ["0 – 5", "shortcuts.stars"],
  ["P", "shortcuts.pick"],
  ["X", "shortcuts.reject"],
  ["U", "shortcuts.unflag"],
  ["6 / 7 / 8 / 9", "shortcuts.colors"],
  ["← → ↑ ↓", "shortcuts.move"],
  ["Shift + ← → ↑ ↓", "shortcuts.extend"],
  ["Mod + A", "shortcuts.selectAll"],
  ["Enter / E", "shortcuts.open"],
  ["Esc / G", "shortcuts.close"],
  ["Z / Espaço", "shortcuts.toggleZoom"],
  ["+ / −", "shortcuts.zoom"],
  ["F", "shortcuts.fullscreen"],
  ["I", "shortcuts.info"],
    ["C", "shortcuts.compare"],
  ["D", "shortcuts.develop"],
  ["Mod + Z / Mod + Shift + Z", "shortcuts.undoRedo"],
  ["\\", "shortcuts.beforeAfter"],
  ["R", "shortcuts.crop"],
  ["Mod + Shift + C / V", "shortcuts.copyPaste"],
  ["Y", "shortcuts.split"],
  ["Mod + U", "shortcuts.autoEdit"],
  ["O", "shortcuts.maskOverlay"],
  ["Mod + Shift + I", "shortcuts.import"],
  ["?", "shortcuts.help"],
];
