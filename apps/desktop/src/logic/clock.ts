/**
 * Desvios de relógio entre câmaras: “+3min42s”, “-1min15s”, “+00:03:42”, “-1:15”, “+1h2m”.
 * Positivo = somar à hora da câmara (o relógio estava atrasado).
 */
export function parseOffset(text: string): number | null {
  const t = text.trim().replace(/\s+/g, "").replace("−", "-").toLowerCase();
  if (t === "" || t === "0" || t === "+0" || t === "-0") return 0;
  const sign = t.startsWith("-") ? -1 : 1;
  const body = t.replace(/^[+-]/, "");
  // hh:mm:ss ou mm:ss
  const colon = /^(\d+):(\d{1,2})(?::(\d{1,2}))?$/.exec(body);
  if (colon) {
    const [a, b, c] = [Number(colon[1]), Number(colon[2]), colon[3] !== undefined ? Number(colon[3]) : null];
    const secs = c === null ? a * 60 + b : a * 3600 + b * 60 + c;
    return sign * secs * 1000;
  }
  // 1h2min3s / 3m42s / 42s / 5min
  const re = /^(?:(\d+)h)?(?:(\d+)(?:min|m))?(?:(\d+)s)?$/;
  const m = re.exec(body);
  if (!m || (!m[1] && !m[2] && !m[3])) return null;
  const secs = Number(m[1] ?? 0) * 3600 + Number(m[2] ?? 0) * 60 + Number(m[3] ?? 0);
  return sign * secs * 1000;
}

export function formatOffset(ms: number): string {
  if (ms === 0) return "0";
  const sign = ms < 0 ? "-" : "+";
  let s = Math.round(Math.abs(ms) / 1000);
  const h = Math.floor(s / 3600);
  s -= h * 3600;
  const m = Math.floor(s / 60);
  s -= m * 60;
  return sign + (h ? `${h}h` : "") + (m ? `${m}min` : "") + (s || (!h && !m) ? `${s}s` : "");
}
