// Testa uma galeria gerada num navegador real (Chromium via Playwright).
//
//   node scripts/test-gallery.cjs <pasta-da-galeria> [pin]
//
// Abre a galeria pelo disco (file://) e por HTTP (servidor local mínimo) e verifica:
// fotografias e capítulos, mosaico sem sobreposições e colunas por dispositivo, grelha,
// favoritas (persistentes, filtro, lista), visualizador (setas, Esc, ligação direta),
// apresentação, downloads (individual e ZIP existem) e, com PIN, a entrada protegida.
const { chromium, devices } = require("playwright");
const http = require("http");
const fs = require("fs");
const path = require("path");

const dir = path.resolve(process.argv[2]);
const pin = process.argv[3];
let failures = 0;
function check(ok, msg) {
  console.log((ok ? "  ok  " : "  FALHA ") + msg);
  if (!ok) failures++;
}

function serve(root) {
  const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".jpg": "image/jpeg", ".woff2": "font/woff2", ".zip": "application/zip", ".png": "image/png" };
  const s = http.createServer((req, res) => {
    const p = path.join(root, decodeURIComponent(req.url.split("?")[0]));
    if (!p.startsWith(root) || !fs.existsSync(p) || fs.statSync(p).isDirectory()) {
      const index = path.join(p, "index.html");
      if (fs.existsSync(index)) { res.writeHead(200, { "content-type": "text/html" }); return fs.createReadStream(index).pipe(res); }
      res.writeHead(404); return res.end();
    }
    res.writeHead(200, { "content-type": types[path.extname(p)] || "application/octet-stream" });
    fs.createReadStream(p).pipe(res);
  });
  return new Promise((r) => s.listen(0, "127.0.0.1", () => r(s)));
}

let unlockedOnce = false;
async function unlock(page) {
  if (!pin) return;
  await page.waitForSelector(".gate input, .tile");
  if (await page.locator(".tile").count()) {
    check(unlockedOnce, "código lembrado neste separador (sessionStorage)");
    return;
  }
  unlockedOnce = true;
  await page.fill(".gate input", "0000");
  await page.click(".gate button");
  await page.waitForSelector(".gate .err:not(:empty)");
  check((await page.textContent(".gate .err")).length > 0, "PIN errado → mensagem");
  check((await page.locator(".tile").count()) === 0, "PIN errado → nada visível");
  await page.fill(".gate input", pin);
  await page.click(".gate button");
  await page.waitForSelector(".tile");
  check(true, "PIN certo → galeria aberta");
}

(async () => {
  const browser = await chromium.launch();
  const server = await serve(dir);
  const base = `http://127.0.0.1:${server.address().port}/`;
  const errors = [];
  for (const [label, url] of [["disco (file://)", "file://" + dir + "/index.html"], ["HTTP", base]]) {
    console.log(`\n${label}`);
    const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, acceptDownloads: true });
    const page = await ctx.newPage();
    page.on("pageerror", (e) => errors.push(e.message));
    page.on("console", (m) => m.type() === "error" && !/favicon/.test(m.text()) && errors.push(m.text()));
    await page.goto(url);
    await unlock(page);
    await page.waitForSelector(".tile");
    const data = await page.evaluate(() => ({ n: document.querySelectorAll(".tile").length, sections: document.querySelectorAll(".chapter").length }));
    check(data.n > 0 && data.sections > 0, `${data.n} fotografias em ${data.sections} capítulos`);
    // Mosaico: sem sobreposições, 4 colunas a 1440 px.
    const geo = await page.evaluate(() => {
      const box = document.querySelector(".photos");
      const r = Array.from(box.children).filter((c) => !c.hidden).map((c) => c.getBoundingClientRect());
      let overlap = 0;
      for (let i = 0; i < r.length; i++) for (let j = i + 1; j < r.length; j++) {
        const a = r[i], b = r[j];
        if (a.left < b.right - 1 && b.left < a.right - 1 && a.top < b.bottom - 1 && b.top < a.bottom - 1) overlap++;
      }
      return { overlap, cols: new Set(r.map((x) => Math.round(x.left))).size, layout: box.className };
    });
    check(geo.layout.includes("masonry") && geo.overlap === 0, "mosaico sem sobreposições");
    check(geo.cols === 4, `desktop: ${geo.cols} colunas`);
    // Grelha (persistente).
    await page.click(".tool.layout");
    const sq = await page.evaluate(() => { const r = document.querySelector(".photos .tile").getBoundingClientRect(); return [document.querySelector(".photos").className, Math.abs(r.width - r.height)]; });
    check(sq[0].includes("grid") && sq[1] < 2, "grelha com fotografias quadradas");
    await page.reload();
    await unlock(page);
    await page.waitForSelector(".tile");
    check((await page.getAttribute(".photos", "class")).includes("grid"), "a escolha da grelha mantém-se");
    await page.click(".tool.layout");
    // Favoritas.
    await page.hover(".tile >> nth=0");
    await page.click(".tile >> nth=0 >> .heart");
    await page.hover(".tile >> nth=2");
    await page.click(".tile >> nth=2 >> .heart");
    check((await page.textContent(".fav-filter .n")) === "2", "2 favoritas contadas");
    await page.reload();
    await unlock(page);
    await page.waitForSelector(".tile");
    check((await page.textContent(".fav-filter .n")) === "2", "favoritas guardadas no navegador");
    await page.click(".fav-filter");
    check((await page.locator(".tile:visible").count()) === 2, "filtro: só as favoritas");
    await page.click(".fav-filter");
    await page.click(".fav-list");
    const list = await page.inputValue(".dlg textarea");
    check(list.split("\n").length === 2, "lista das favoritas para o fotógrafo: " + list.replace(/\n/g, ", "));
    const contact = await page.evaluate(() => (window.MOMENTO_GALLERY || {}).contact || document.querySelector("footer a[href^='mailto:']") !== null);
    check((await page.locator(".dlg a[href^='mailto:']").count()) === (contact ? 1 : 0), contact ? "enviar por email" : "sem email de contacto: sem botão de email");
    check((await page.locator(".dlg button", { hasText: /favoritas|favourites/i }).count()) === (url.startsWith("http") ? 1 : 0), "ZIP das favoritas só por HTTP");
    if (url.startsWith("http")) {
      const [dl] = await Promise.all([page.waitForEvent("download"), page.click(".dlg .btn:has-text('Descarregar favoritas')")]);
      const p = await dl.path();
      const buf = fs.readFileSync(p);
      check(buf.readUInt32LE(0) === 0x04034b50 && buf.length > 1000, `ZIP das favoritas criado no navegador (${buf.length} bytes)`);
    }
    await page.keyboard.press("Escape");
    // Visualizador.
    await page.click(".tile >> nth=1");
    await page.waitForSelector(".lb.open");
    const w1 = await page.textContent(".lb .where");
    await page.keyboard.press("ArrowRight");
    await page.waitForTimeout(700);
    const w2 = await page.textContent(".lb .where");
    check(w1.startsWith("2 ") && w2.startsWith("3 "), `setas: "${w1}" → "${w2}"`);
    const href = await page.getAttribute(".lb-dl", "href");
    check(href && fs.existsSync(path.join(dir, href)), "download individual aponta para um ficheiro existente");
    const hash = await page.evaluate(() => location.hash);
    check(/^#foto=[0-9a-f]{32}$/.test(hash), "endereço direto da fotografia");
    await page.keyboard.press("Escape");
    await page.waitForTimeout(400);
    check(await page.locator(".lb").isHidden(), "Esc fecha");
    await page.goto(url.replace(/index\.html$/, "index.html") + hash);
    if (url.startsWith("file")) await page.reload();
    await unlock(page);
    await page.waitForSelector(".lb.open", { timeout: 5000 }).catch(() => {});
    check(await page.locator(".lb.open").count() === 1, "ligação direta abre a fotografia");
    await page.keyboard.press("Escape");
    await page.waitForTimeout(400);
    // Apresentação.
    await page.click(".tool.slideshow");
    await page.waitForSelector(".lb.open.playing");
    const s1 = await page.textContent(".lb .where");
    await page.waitForTimeout(4800);
    const s2 = await page.textContent(".lb .where");
    check(s1 !== s2, `apresentação avança sozinha ("${s1}" → "${s2}")`);
    await page.keyboard.press(" ");
    check(!(await page.getAttribute(".lb", "class")).includes("playing"), "espaço pausa");
    await page.keyboard.press("Escape");
    // ZIP.
    const zips = await page.locator(".tool.dl-all").count();
    if (zips) {
      await page.click(".tool.dl-all");
      const hrefs = await page.$$eval(".dlg .zips a", (as) => as.map((a) => a.getAttribute("href")));
      check(hrefs.length > 0 && hrefs.every((h) => fs.existsSync(path.join(dir, h))), `${hrefs.length} ZIP disponíveis`);
      await page.keyboard.press("Escape");
    }
    await ctx.close();
  }
  // Dispositivos: colunas do mosaico.
  for (const [name, dev, want] of [["telemóvel", devices["iPhone 13"], 2], ["tablet", devices["iPad (gen 7)"], 3]]) {
    const ctx = await browser.newContext({ ...dev });
    const page = await ctx.newPage();
    await page.goto("file://" + dir + "/index.html");
    await unlock(page);
    await page.waitForSelector(".tile");
    const cols = await page.evaluate(() => new Set(Array.from(document.querySelector(".photos").children).map((c) => Math.round(c.getBoundingClientRect().left))).size);
    check(cols === want, `${name}: ${cols} colunas`);
    const sw = await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1);
    check(sw, `${name}: sem deslocamento horizontal`);
    await ctx.close();
  }
  check(errors.length === 0, "sem erros de JavaScript " + (errors.length ? JSON.stringify(errors) : ""));
  server.close();
  await browser.close();
  console.log(failures ? `\n${failures} falha(s)` : "\nTudo bem.");
  process.exit(failures ? 1 : 0);
})();
