// Auditoria visual da interface num browser real (Chromium — o motor do WebView2 do
// Windows), com a API simulada (audit/api.ts). Percorre os ecrãs, guarda capturas e verifica
// automaticamente: imagens esmagadas, texto cortado, elementos fora do ecrã e erros na consola.
//
// Uso: node audit/run.mjs <pasta-de-saída> [largura] [altura]
import { createServer } from "vite";
import { createRequire } from "node:module";
// Playwright pré-instalado no sistema (fora do projeto): carregado como CommonJS.
const { chromium } = createRequire(import.meta.url)("playwright");
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const out = path.resolve(process.argv[2] ?? "audit-out");
const W = Number(process.argv[3] ?? 1920);
const H = Number(process.argv[4] ?? 1000);
fs.mkdirSync(out, { recursive: true });

const server = await createServer({ configFile: path.join(here, "vite.config.mjs") });
await server.listen();
const browser = await chromium.launch();
const page = await browser.newPage({
  viewport: { width: W, height: H },
  deviceScaleFactor: 1,
  userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36 Edg/131.0",
});
const problems = [];
page.on("console", (m) => m.type() === "error" && problems.push({ screen: current, kind: "consola", detail: m.text().slice(0, 300) }));
page.on("pageerror", (e) => problems.push({ screen: current, kind: "exceção", detail: String(e).slice(0, 300) }));
// Imagens do protocolo da aplicação: verticais quando id % 5 === 4 (como em audit/api.ts).
await page.route("http://momento.localhost/**", (route) => {
  const [, , , , level, id] = route.request().url().split("/");
  const portrait = Number(id) % 5 === 4;
  const file = portrait ? "port.jpg" : level === "thumb" ? "land.jpg" : "land_big.jpg";
  route.fulfill({ path: path.join(here, "img", file), contentType: "image/jpeg" });
});

let current = "início";
async function check(name) {
  current = name;
  await page.waitForTimeout(600);
  await page.screenshot({ path: path.join(out, `${name}.png`) });
  const found = await page.evaluate(({ W, H }) => {
    const res = [];
    const visible = (el) => {
      const r = el.getBoundingClientRect();
      const s = getComputedStyle(el);
      return r.width > 0 && r.height > 0 && s.visibility !== "hidden" && s.display !== "none" && r.bottom > 0 && r.top < H;
    };
    const label = (el) => (el.getAttribute("aria-label") || el.textContent || el.className || el.tagName).toString().trim().slice(0, 60);
    // Imagens esmagadas: caixa muito mais baixa do que a imagem pede.
    for (const img of document.querySelectorAll("img")) {
      if (!visible(img) || !img.naturalWidth) continue;
      const r = img.getBoundingClientRect();
      const box = r.width / r.height;
      const nat = img.naturalWidth / img.naturalHeight;
      const fit = getComputedStyle(img).objectFit;
      if (r.height < 24 || (fit !== "contain" && fit !== "cover" && Math.abs(Math.log(box / nat)) > 0.25)) {
        res.push({ kind: "imagem deformada", detail: `${Math.round(r.width)}×${Math.round(r.height)} (natural ${img.naturalWidth}×${img.naturalHeight}) em ${label(img.parentElement)}` });
      } else if ((fit === "contain" || fit === "cover") && r.height < Math.min(60, r.width * 0.25)) {
        res.push({ kind: "imagem esmagada", detail: `${Math.round(r.width)}×${Math.round(r.height)} em ${label(img.parentElement)}` });
      }
    }
    // Texto cortado sem reticências; elementos fora do ecrã na horizontal.
    for (const el of document.querySelectorAll("button, label, h1, h2, h3, h4, span, a, p, th, td, option, .side-item")) {
      if (!visible(el)) continue;
      const s = getComputedStyle(el);
      const r = el.getBoundingClientRect();
      // Dentro de uma zona com deslocamento (ex.: tira de miniaturas) não conta.
      let scroller = el.parentElement;
      while (scroller && !/(auto|scroll)/.test(getComputedStyle(scroller).overflowX)) scroller = scroller.parentElement;
      if (r.width <= 1 || r.height <= 1) continue; // só para leitores de ecrã (sr-only)
      if (r.right > W + 1 && s.position !== "fixed" && !scroller) res.push({ kind: "fora do ecrã", detail: `${label(el)} (direita ${Math.round(r.right)})` });
      const clipsX = el.scrollWidth > el.clientWidth + 2 && (s.overflow === "hidden" || s.overflowX === "hidden") && s.textOverflow !== "ellipsis";
      const clipsY = el.scrollHeight > el.clientHeight + 2 && (s.overflow === "hidden" || s.overflowY === "hidden") && el.children.length === 0;
      if (clipsX || clipsY) res.push({ kind: "texto cortado", detail: `${label(el)} (${el.scrollWidth}×${el.scrollHeight} em ${el.clientWidth}×${el.clientHeight})` });
    }
    // Página com deslocamento horizontal.
    if (document.documentElement.scrollWidth > W + 1) res.push({ kind: "página mais larga que a janela", detail: `${document.documentElement.scrollWidth}px` });
    return res;
  }, { W, H });
  // Uma entrada por tipo+detalhe (as grelhas repetem o mesmo problema).
  const seen = new Set();
  for (const f of found) {
    const k = f.kind + f.detail.replace(/\d+/g, "#");
    if (!seen.has(k)) {
      seen.add(k);
      problems.push({ screen: name, ...f });
    }
  }
}
const store = (fn, arg) => page.evaluate(([f, a]) => window.__store.getState()[f](a), [fn, arg]);

await page.goto(`http://localhost:1430/audit/index.html`);
await page.waitForSelector("#root *");
await check("01-inicio");
await page.evaluate(() => window.__openProject());
await page.waitForTimeout(800);
await check("02-grelha");
for (const [i, v] of ["sequences", "people", "consistency", "album"].entries()) {
  await store("setView", v);
  await check(`0${3 + i}-${v}`);
}
await store("setView", "grid");
await page.evaluate(() => window.__store.getState().setSelection({ ids: new Set([1]), focus: 1, anchor: 1 }));
await store("setView", "develop");
await page.waitForTimeout(800);
await check("07-editor");
// Abrir todas as secções do painel de edição.
for (const b of await page.$$(".develop-panel .section-head, .develop-panel button[aria-expanded='false']")) {
  await b.click().catch(() => {});
}
await check("08-editor-secoes");
await store("setView", "grid");
await page.evaluate(() => window.__store.getState().openViewer(1));
await check("09-visualizador");
await page.evaluate(() => window.__store.getState().openViewer(null));
await page.evaluate(() => window.__store.getState().setExportDialog(true));
await check("10-exportar");
await page.evaluate(() => window.__store.getState().setExportDialog(false));

fs.writeFileSync(path.join(out, "problemas.json"), JSON.stringify(problems, null, 1));
const byScreen = problems.reduce((a, p) => ((a[p.screen] = (a[p.screen] ?? 0) + 1), a), {});
console.log(`${problems.length} problemas`, byScreen);
for (const p of problems.slice(0, 80)) console.log(`[${p.screen}] ${p.kind}: ${p.detail}`);
await browser.close();
await server.close();
