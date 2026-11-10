/* Momento Studio — galeria estática.
 * Sem bibliotecas nem pedidos externos. Funciona aberta do disco (file://) e em qualquer
 * alojamento estático. Dados em data/gallery.js (simples ou cifrados com AES-GCM). */
(function () {
  "use strict";

  var TEXT = {
    pt: {
      view: "Ver galeria", photos: "fotografias", favorites: "Favoritas", grid: "Grelha", masonry: "Mosaico",
      slideshow: "Apresentação", download: "Descarregar", close: "Fechar", prev: "Anterior", next: "Seguinte",
      fullscreen: "Ecrã inteiro", pause: "Pausa", play: "Reproduzir", fav: "Favorita", unfav: "Retirar das favoritas",
      favEmpty: "Ainda não escolheu favoritas. Toque no coração de uma fotografia.",
      favTitle: "As suas favoritas", favInfo: "Envie esta lista ao fotógrafo para pedir impressões ou o álbum.",
      copy: "Copiar lista", copied: "Lista copiada", send: "Enviar por email", favZip: "Descarregar favoritas",
      favZipBusy: "A preparar…", dlTitle: "Descarregar", dlInfo: "Ficheiros ZIP com as fotografias em alta resolução.",
      photo: "Fotografia", of: "de", private: "Galeria privada", pinAsk: "Introduza o código que recebeu.",
      pwAsk: "Introduza a palavra-passe que recebeu.", enter: "Entrar", wrong: "Código errado. Tente novamente.",
      noCrypto: "Este navegador não permite abrir galerias protegidas aqui. Abra o endereço https.",
      made: "Galeria criada com Momento Studio", allPhotos: "Todas",
      favSubject: "Favoritas", favNone: "Sem favoritas"
    },
    en: {
      view: "View gallery", photos: "photos", favorites: "Favourites", grid: "Grid", masonry: "Masonry",
      slideshow: "Slideshow", download: "Download", close: "Close", prev: "Previous", next: "Next",
      fullscreen: "Full screen", pause: "Pause", play: "Play", fav: "Favourite", unfav: "Remove favourite",
      favEmpty: "No favourites yet. Tap the heart on a photo.",
      favTitle: "Your favourites", favInfo: "Send this list to your photographer to order prints or the album.",
      copy: "Copy list", copied: "List copied", send: "Send by email", favZip: "Download favourites",
      favZipBusy: "Preparing…", dlTitle: "Download", dlInfo: "ZIP files with the full-resolution photos.",
      photo: "Photo", of: "of", private: "Private gallery", pinAsk: "Enter the code you received.",
      pwAsk: "Enter the password you received.", enter: "Enter", wrong: "Wrong code. Please try again.",
      noCrypto: "This browser cannot open protected galleries here. Use the https address.",
      made: "Gallery made with Momento Studio", allPhotos: "All",
      favSubject: "Favourites", favNone: "No favourites"
    }
  };

  var ICON = {
    heart: '<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 21s-7.5-4.6-9.6-9.2C.9 8.5 2.9 4.5 6.7 4.5c2.1 0 3.6 1.1 5.3 3 1.7-1.9 3.2-3 5.3-3 3.8 0 5.8 4 4.3 7.3C19.5 16.4 12 21 12 21z"/></svg>',
    heartLine: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path d="M12 20s-7-4.3-9-8.6C1.6 8.3 3.5 5 6.8 5c2 0 3.4 1 5.2 2.9C13.8 6 15.2 5 17.2 5c3.3 0 5.2 3.3 3.8 6.4C19 15.7 12 20 12 20z"/></svg>',
    grid: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><rect x="4" y="4" width="7" height="7"/><rect x="13" y="4" width="7" height="7"/><rect x="4" y="13" width="7" height="7"/><rect x="13" y="13" width="7" height="7"/></svg>',
    masonry: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><rect x="4" y="4" width="7" height="10"/><rect x="13" y="4" width="7" height="6"/><rect x="4" y="16" width="7" height="4"/><rect x="13" y="12" width="7" height="8"/></svg>',
    play: '<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5.5v13l11-6.5z"/></svg>',
    pause: '<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="6.5" y="5" width="4" height="14"/><rect x="13.5" y="5" width="4" height="14"/></svg>',
    down: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path d="M12 4v11m0 0l-4.5-4.5M12 15l4.5-4.5M5 19.5h14"/></svg>',
    close: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18"/></svg>',
    prev: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4" aria-hidden="true"><path d="M15 5l-7 7 7 7"/></svg>',
    next: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4" aria-hidden="true"><path d="M9 5l7 7-7 7"/></svg>',
    full: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/></svg>',
    arrow: '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path d="M12 5v14m0 0l-5-5m5 5l5-5"/></svg>'
  };

  var app = document.getElementById("app");
  var G = null;
  var t = TEXT.pt;
  var favs = {};
  var favOnly = false;
  var layout = "masonry";
  var tiles = []; // { el, img, heart, i }
  var order = []; // índices das fotografias visíveis, pela ordem dos capítulos
  var chapterOf = [];

  // ---------- utilitários ----------
  function el(tag, attrs, children) {
    var e = document.createElement(tag);
    if (attrs) {
      for (var k in attrs) {
        if (!Object.prototype.hasOwnProperty.call(attrs, k) || attrs[k] == null) continue;
        if (k === "text") e.textContent = attrs[k];
        else if (k === "html") e.innerHTML = attrs[k]; // só para os ícones fixos acima
        else if (k === "class") e.className = attrs[k];
        else if (k.slice(0, 2) === "on") e.addEventListener(k.slice(2), attrs[k]);
        else e.setAttribute(k, attrs[k]);
      }
    }
    (children || []).forEach(function (c) { if (c) e.appendChild(c); });
    return e;
  }
  function store(k, v) {
    try { if (v === undefined) return localStorage.getItem(k); localStorage.setItem(k, v); } catch (e) { return null; }
    return null;
  }
  function bytes(n) {
    var u = ["B", "KB", "MB", "GB"], i = 0;
    while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
    return n.toLocaleString(G.lang === "en" ? "en" : "pt-PT", { maximumFractionDigits: i ? 1 : 0 }) + " " + u[i];
  }
  function niceDate(iso) {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(iso || "")) return iso || "";
    try {
      return new Intl.DateTimeFormat(G.lang === "en" ? "en-GB" : "pt-PT", { day: "numeric", month: "long", year: "numeric" }).format(new Date(iso + "T12:00:00"));
    } catch (e) { return iso; }
  }
  var toastTimer;
  function toast(msg) {
    var tEl = document.querySelector(".toast") || document.body.appendChild(el("div", { class: "toast", role: "status" }));
    tEl.textContent = msg;
    tEl.classList.add("on");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(function () { tEl.classList.remove("on"); }, 2200);
  }

  // ---------- entrada ----------
  function boot() {
    if (window.MOMENTO_GALLERY) return start(window.MOMENTO_GALLERY);
    if (window.MOMENTO_GALLERY_ENC) return gate(window.MOMENTO_GALLERY_ENC);
    app.appendChild(el("p", { class: "empty", text: "Galeria vazia." }));
  }

  function b64(s) {
    var bin = atob(s), out = new Uint8Array(bin.length);
    for (var i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  }

  function decrypt(sealed, secret) {
    var subtle = window.crypto && window.crypto.subtle;
    if (!subtle) return Promise.reject(new Error("nocrypto"));
    return subtle.importKey("raw", new TextEncoder().encode(secret), "PBKDF2", false, ["deriveKey"])
      .then(function (k) {
        return subtle.deriveKey({ name: "PBKDF2", salt: b64(sealed.salt), iterations: sealed.iter, hash: "SHA-256" }, k, { name: "AES-GCM", length: 256 }, false, ["decrypt"]);
      })
      .then(function (key) { return subtle.decrypt({ name: "AES-GCM", iv: b64(sealed.iv) }, key, b64(sealed.data)); })
      .then(function (buf) { return JSON.parse(new TextDecoder().decode(buf)); });
  }

  function gate(enc) {
    t = TEXT[enc.lang] || TEXT.pt;
    document.body.setAttribute("data-theme", enc.theme || "light");
    var memo = "momento:secret:" + enc.sealed.salt;
    var saved = null;
    try { saved = sessionStorage.getItem(memo); } catch (e) { saved = null; }
    var input = el("input", {
      type: enc.pin ? "text" : "password", class: enc.pin ? "pin" : "", inputmode: enc.pin ? "numeric" : null,
      autocomplete: "off", "aria-label": enc.pin ? t.pinAsk : t.pwAsk, maxlength: enc.pin ? "8" : null
    });
    var err = el("div", { class: "err", role: "alert" });
    var btn = el("button", { class: "btn primary", type: "submit", text: t.enter });
    var wrap = el("div", { class: "gate" }, [
      el("form", {
        onsubmit: function (ev) {
          ev.preventDefault();
          btn.disabled = true;
          try_(input.value.trim());
        }
      }, [el("h1", { text: t.private }), el("p", { text: enc.pin ? t.pinAsk : t.pwAsk }), input, btn, err])
    ]);
    function try_(secret) {
      decrypt(enc.sealed, secret).then(function (data) {
        try { sessionStorage.setItem(memo, secret); } catch (e) { /* sem armazenamento */ }
        wrap.remove();
        start(data);
      }, function (e) {
        btn.disabled = false;
        err.textContent = e && e.message === "nocrypto" ? t.noCrypto : t.wrong;
        wrap.classList.remove("shake");
        void wrap.offsetWidth;
        wrap.classList.add("shake");
        input.select();
      });
    }
    // Código lembrado neste separador: abre sem mostrar o formulário.
    if (saved) {
      decrypt(enc.sealed, saved).then(start, function () { app.appendChild(wrap); input.focus(); });
    } else {
      app.appendChild(wrap);
      input.focus();
    }
  }

  // ---------- galeria ----------
  function start(data) {
    G = data;
    t = TEXT[G.lang] || TEXT.pt;
    document.documentElement.lang = G.lang === "en" ? "en" : "pt-PT";
    document.body.setAttribute("data-theme", G.theme || "light");
    if (G.couple) document.title = G.couple;
    layout = store("momento:layout:" + G.id) || G.layout || "masonry";
    try { favs = JSON.parse(store("momento:fav:" + G.id) || "{}") || {}; } catch (e) { favs = {}; }
    chapterOf = [];
    G.chapters.forEach(function (c, ci) { c.photos.forEach(function (i) { chapterOf[i] = ci; }); });
    render();
    window.addEventListener("resize", debounce(relayout, 120));
    window.addEventListener("hashchange", fromHash);
    fromHash();
  }

  function debounce(f, ms) {
    var h;
    return function () { clearTimeout(h); h = setTimeout(f, ms); };
  }

  function render() {
    app.textContent = "";
    tiles = [];
    // Hero
    var meta = [niceDate(G.date), G.location].filter(Boolean).join("  ·  ");
    var hero = el("header", { class: "hero" + (G.hero ? "" : " plain") }, [
      G.hero ? el("img", { src: G.hero.src, alt: "", fetchpriority: "high" }) : null,
      G.logo ? el("img", { class: "logo", src: G.logo, alt: G.photographer || "" }) : null,
      el("div", { class: "hero-inner" }, [
        meta ? el("p", { class: "eyebrow", text: meta }) : null,
        el("h1", { class: "names", text: G.couple }),
        el("a", { class: "enter", href: "#galeria", html: ICON.arrow }, [])
      ])
    ]);
    var enter = hero.querySelector(".enter");
    enter.appendChild(document.createTextNode(t.view));
    enter.addEventListener("click", function (ev) {
      ev.preventDefault();
      document.getElementById("galeria").scrollIntoView({ behavior: "smooth" });
    });
    app.appendChild(hero);
    // Introdução
    if (G.intro) {
      var intro = el("section", { class: "intro" }, [el("span", { class: "ornament" })]);
      G.intro.split(/\n{2,}/).forEach(function (p) { intro.appendChild(el("p", { text: p.trim() })); });
      app.appendChild(intro);
    }
    // Barra
    var nav = el("nav", { "aria-label": t.photos });
    G.chapters.forEach(function (c, ci) {
      if (c.title) nav.appendChild(el("a", { href: "#c-" + ci, "data-c": ci, text: c.title }));
    });
    var tools = el("div", { class: "tools" });
    var layoutBtn = el("button", { class: "tool layout", type: "button", onclick: toggleLayout });
    var favBtn = G.features.favorites ? el("button", { class: "tool fav-filter", type: "button", "aria-pressed": "false", onclick: toggleFavOnly }) : null;
    var favList = G.features.favorites ? el("button", { class: "tool fav-list", type: "button", onclick: showFavorites, text: "" }) : null;
    tools.appendChild(layoutBtn);
    if (favBtn) tools.appendChild(favBtn);
    if (favList) tools.appendChild(favList);
    tools.appendChild(el("button", { class: "tool slideshow", type: "button", "aria-label": t.slideshow, title: t.slideshow, html: ICON.play, onclick: function () { open(order[0], true); } }));
    if (G.features.downloads === "all" && G.zips && G.zips.length) {
      tools.appendChild(el("button", { class: "tool dl-all", type: "button", "aria-label": t.download, title: t.download, html: ICON.down, onclick: showZips }));
    }
    var bar = el("div", { class: "bar", id: "galeria" }, [nav, tools]);
    app.appendChild(bar);
    // Capítulos
    var main = el("main");
    G.chapters.forEach(function (c, ci) {
      var box = el("div", { class: "photos " + layout });
      var sec = el("section", { class: "chapter", id: "c-" + ci, "aria-label": c.title || t.photos }, [
        c.title ? el("header", {}, [
          el("span", { class: "num", text: (ci + 1 < 10 ? "0" : "") + (ci + 1) }),
          el("h2", { text: c.title }),
          el("span", { class: "count", text: c.photos.length + " " + t.photos })
        ]) : null,
        box
      ]);
      c.photos.forEach(function (i) { box.appendChild(tile(i)); });
      main.appendChild(sec);
    });
    app.appendChild(main);
    main.appendChild(el("p", { class: "empty", hidden: "", text: t.favEmpty }));
    // Rodapé
    var foot = el("footer", {}, [
      G.photographer ? el("p", { class: "who", text: G.photographer }) : null,
      G.contact ? el("a", { href: "mailto:" + G.contact, text: G.contact }) : null,
      el("p", { class: "made", text: t.made })
    ]);
    app.appendChild(foot);
    updateTools();
    applyFilter();
    spy();
  }

  function tile(i) {
    var p = G.photos[i];
    var label = t.photo + " " + (i + 1) + (G.chapters[chapterOf[i]].title ? " — " + G.chapters[chapterOf[i]].title : "");
    var img = el("img", {
      src: p.t, alt: label, loading: "lazy", decoding: "async", width: p.tw, height: p.th,
      srcset: p.t + " " + p.tw + "w, " + p.s + " " + p.w + "w", sizes: "(min-width: 1600px) 20vw, (min-width: 1024px) 25vw, (min-width: 640px) 33vw, 50vw"
    });
    img.addEventListener("load", function () { img.classList.add("ok"); });
    if (img.complete && img.naturalWidth) img.classList.add("ok");
    var a = el("a", { class: "tile", href: "#foto=" + p.id, "data-i": i, style: "background:" + p.c }, [img]);
    a.addEventListener("click", function (ev) { ev.preventDefault(); open(i); });
    var heart = null;
    if (G.features.favorites) {
      heart = el("button", { class: "heart", type: "button", "aria-pressed": favs[p.id] ? "true" : "false", "aria-label": t.fav, html: ICON.heart });
      heart.addEventListener("click", function (ev) { ev.preventDefault(); ev.stopPropagation(); toggleFav(i); });
      a.appendChild(heart);
    }
    tiles[i] = { el: a, img: img, heart: heart, i: i };
    return a;
  }

  // ---------- disposição ----------
  function columns(w) {
    if (w < 640) return 2;
    if (w < 1024) return 3;
    if (w < 1600) return 4;
    return 5;
  }

  function relayout() {
    var gap = parseFloat(getComputedStyle(document.body).getPropertyValue("--gap")) || 10;
    document.querySelectorAll(".photos").forEach(function (box) {
      box.className = "photos " + layout;
      var items = Array.prototype.filter.call(box.children, function (c) { return !c.hidden; });
      if (layout !== "masonry") {
        box.style.height = "";
        items.forEach(function (c) { c.style.cssText = c.style.background ? "background:" + c.style.background : ""; });
        return;
      }
      var W = box.clientWidth, n = columns(window.innerWidth), cw = (W - gap * (n - 1)) / n;
      var hs = [];
      for (var k = 0; k < n; k++) hs.push(0);
      items.forEach(function (c) {
        var p = G.photos[+c.getAttribute("data-i")];
        var col = hs.indexOf(Math.min.apply(null, hs));
        var h = cw * p.th / p.tw;
        c.style.left = col * (cw + gap) + "px";
        c.style.top = hs[col] + "px";
        c.style.width = cw + "px";
        c.style.height = h + "px";
        hs[col] += h + gap;
      });
      box.style.height = Math.max(0, Math.max.apply(null, hs) - gap) + "px";
    });
  }

  function toggleLayout() {
    layout = layout === "masonry" ? "grid" : "masonry";
    store("momento:layout:" + G.id, layout);
    updateTools();
    relayout();
  }

  function updateTools() {
    var lb = document.querySelector(".tool.layout");
    var other = layout === "masonry" ? "grid" : "masonry";
    if (lb) {
      lb.innerHTML = ICON[other];
      lb.setAttribute("aria-label", t[other]);
      lb.title = t[other];
    }
    var n = Object.keys(favs).filter(function (k) { return favs[k]; }).length;
    var ff = document.querySelector(".fav-filter");
    if (ff) {
      ff.innerHTML = favOnly ? ICON.heart : ICON.heartLine;
      ff.appendChild(el("span", { class: "n", text: String(n) }));
      ff.setAttribute("aria-pressed", favOnly ? "true" : "false");
      ff.setAttribute("aria-label", t.favorites + " (" + n + ")");
      ff.title = t.favorites;
    }
    var fl = document.querySelector(".fav-list");
    if (fl) {
      fl.textContent = "";
      fl.appendChild(el("span", { class: "label", text: t.favTitle }));
      fl.hidden = n === 0;
    }
  }

  function applyFilter() {
    order = [];
    G.chapters.forEach(function (c, ci) {
      var shown = 0;
      c.photos.forEach(function (i) {
        var on = !favOnly || favs[G.photos[i].id];
        tiles[i].el.hidden = !on;
        if (on) { order.push(i); shown++; }
      });
      var sec = document.getElementById("c-" + ci);
      sec.hidden = shown === 0;
      var a = document.querySelector('.bar nav a[data-c="' + ci + '"]');
      if (a) a.hidden = shown === 0;
    });
    var empty = document.querySelector("main .empty");
    if (empty) empty.hidden = order.length > 0;
    relayout();
  }

  function spy() {
    if (!("IntersectionObserver" in window)) return;
    var io = new IntersectionObserver(function (es) {
      es.forEach(function (e) {
        if (!e.isIntersecting) return;
        var ci = e.target.id.slice(2);
        document.querySelectorAll(".bar nav a").forEach(function (a) {
          var on = a.getAttribute("data-c") === ci;
          a.classList.toggle("on", on);
          if (on && a.scrollIntoView && window.innerWidth < 900) a.parentNode.scrollLeft = a.offsetLeft - 20;
        });
      });
    }, { rootMargin: "-45% 0px -50% 0px" });
    document.querySelectorAll(".chapter").forEach(function (s) { io.observe(s); });
  }

  // ---------- favoritas ----------
  function toggleFav(i) {
    var id = G.photos[i].id;
    if (favs[id]) delete favs[id]; else favs[id] = 1;
    store("momento:fav:" + G.id, JSON.stringify(favs));
    var on = !!favs[id];
    if (tiles[i].heart) {
      tiles[i].heart.setAttribute("aria-pressed", on ? "true" : "false");
      tiles[i].heart.setAttribute("aria-label", on ? t.unfav : t.fav);
    }
    if (lb && lb.i === i) lbFav();
    updateTools();
    if (favOnly) applyFilter();
  }

  function toggleFavOnly() {
    favOnly = !favOnly;
    updateTools();
    applyFilter();
  }

  function favNames() {
    return G.photos.filter(function (p) { return favs[p.id]; }).map(function (p) { return p.n; });
  }

  function dialog(children) {
    var d = el("div", { class: "dlg", role: "dialog", "aria-modal": "true" }, [el("div", {}, children)]);
    function close() { d.remove(); document.removeEventListener("keydown", esc, true); }
    function esc(e) { if (e.key === "Escape") { e.stopPropagation(); close(); } }
    d.addEventListener("click", function (e) { if (e.target === d) close(); });
    document.addEventListener("keydown", esc, true);
    document.body.appendChild(d);
    var f = d.querySelector("button, a, textarea");
    if (f) f.focus();
    return close;
  }

  function showFavorites() {
    var names = favNames();
    var text = names.join("\n");
    var ta = el("textarea", { readonly: "", "aria-label": t.favTitle });
    ta.value = text || t.favNone;
    var row = el("div", { class: "row" });
    var close;
    row.appendChild(el("button", {
      class: "btn primary", type: "button", text: t.copy, onclick: function () {
        ta.select();
        var ok = function () { toast(t.copied); };
        if (navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(text).then(ok, function () { document.execCommand("copy"); ok(); });
        else { document.execCommand("copy"); ok(); }
      }
    }));
    if (G.contact) {
      var subject = encodeURIComponent(t.favSubject + " — " + G.couple + " (" + names.length + ")");
      row.appendChild(el("a", { class: "btn", href: "mailto:" + G.contact + "?subject=" + subject + "&body=" + encodeURIComponent(text), text: t.send }));
    }
    // ZIP das favoritas: precisa de servidor (os navegadores não deixam ler ficheiros locais).
    if (/^https?:$/.test(location.protocol) && G.features.downloads !== "none" && names.length) {
      var zb = el("button", { class: "btn", type: "button", text: t.favZip, onclick: function () { favZip(zb); } });
      row.appendChild(zb);
    }
    close = dialog([el("h3", { text: t.favTitle + " (" + names.length + ")" }), el("p", { text: t.favInfo }), ta, row]);
    return close;
  }

  function showZips() {
    var list = el("ul", { class: "zips" });
    G.zips.forEach(function (z) {
      list.appendChild(el("li", {}, [
        el("span", {}, [document.createTextNode(z.label), el("small", { text: z.count + " " + t.photos + " · " + bytes(z.bytes) })]),
        el("a", { class: "btn", href: z.href, download: "", text: t.download })
      ]));
    });
    dialog([el("h3", { text: t.dlTitle }), el("p", { text: t.dlInfo }), list]);
  }

  // ZIP mínimo (sem compressão) com CRC32, feito no navegador.
  var CRC;
  function crc32(b) {
    if (!CRC) {
      CRC = new Uint32Array(256);
      for (var n = 0; n < 256; n++) {
        var c = n;
        for (var k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
        CRC[n] = c >>> 0;
      }
    }
    var x = 0xffffffff;
    for (var i = 0; i < b.length; i++) x = CRC[(x ^ b[i]) & 0xff] ^ (x >>> 8);
    return (x ^ 0xffffffff) >>> 0;
  }
  function makeZip(files) {
    var parts = [], central = [], offset = 0;
    files.forEach(function (f) {
      var name = new TextEncoder().encode(f.name), crc = crc32(f.data), size = f.data.length;
      var h = new DataView(new ArrayBuffer(30));
      h.setUint32(0, 0x04034b50, true); h.setUint16(4, 20, true); h.setUint16(6, 0x0800, true);
      h.setUint32(14, crc, true); h.setUint32(18, size, true); h.setUint32(22, size, true); h.setUint16(26, name.length, true);
      parts.push(h.buffer, name, f.data);
      var c = new DataView(new ArrayBuffer(46));
      c.setUint32(0, 0x02014b50, true); c.setUint16(4, 20, true); c.setUint16(6, 20, true); c.setUint16(8, 0x0800, true);
      c.setUint32(16, crc, true); c.setUint32(20, size, true); c.setUint32(24, size, true); c.setUint16(28, name.length, true);
      c.setUint32(42, offset, true);
      central.push(c.buffer, name);
      offset += 30 + name.length + size;
    });
    var cs = central.reduce(function (s, b) { return s + b.byteLength; }, 0);
    var e = new DataView(new ArrayBuffer(22));
    e.setUint32(0, 0x06054b50, true); e.setUint16(8, files.length, true); e.setUint16(10, files.length, true);
    e.setUint32(12, cs, true); e.setUint32(16, offset, true);
    return new Blob(parts.concat(central, [e.buffer]), { type: "application/zip" });
  }
  function favZip(btn) {
    var chosen = G.photos.filter(function (p) { return favs[p.id] && (p.d || p.s); });
    btn.disabled = true;
    btn.textContent = t.favZipBusy;
    Promise.all(chosen.map(function (p) {
      return fetch(p.d || p.s).then(function (r) { if (!r.ok) throw new Error(r.status); return r.arrayBuffer(); })
        .then(function (b) { return { name: p.dn || p.n + ".jpg", data: new Uint8Array(b) }; });
    })).then(function (files) {
      var a = el("a", { href: URL.createObjectURL(makeZip(files)), download: (G.couple || "galeria") + " — " + t.favorites + ".zip" });
      document.body.appendChild(a); a.click(); a.remove();
    }).catch(function () { toast("Erro"); }).then(function () { btn.disabled = false; btn.textContent = t.favZip; });
  }

  // ---------- visualizador ----------
  var lb = null;

  function open(i, play) {
    if (i == null) return;
    if (!lb) buildLb();
    lb.root.hidden = false;
    document.body.style.overflow = "hidden";
    requestAnimationFrame(function () { lb.root.classList.add("open"); });
    lb.returnFocus = document.activeElement;
    show(i, true);
    if (play) setPlaying(true);
    lb.root.querySelector(".lb-close").focus();
  }

  function buildLb() {
    var img = el("img", { alt: "" });
    var stage = el("div", { class: "lb-stage" }, [
      img,
      el("button", { class: "nav prev", type: "button", "aria-label": t.prev, html: ICON.prev, onclick: function () { step(-1); } }),
      el("button", { class: "nav next", type: "button", "aria-label": t.next, html: ICON.next, onclick: function () { step(1); } })
    ]);
    var where = el("div", { class: "where" });
    var top = el("div", { class: "lb-top" }, [where]);
    var fav = G.features.favorites ? el("button", { class: "tool lb-fav", type: "button", onclick: function () { toggleFav(lb.i); } }) : null;
    var dl = G.features.downloads !== "none" ? el("a", { class: "tool lb-dl", "aria-label": t.download, title: t.download, html: ICON.down, download: "" }) : null;
    var playBtn = el("button", { class: "tool lb-play", type: "button", onclick: function () { setPlaying(!lb.playing); } });
    var full = document.fullscreenEnabled ? el("button", { class: "tool", type: "button", "aria-label": t.fullscreen, title: t.fullscreen, html: ICON.full, onclick: toggleFull }) : null;
    var close = el("button", { class: "tool lb-close", type: "button", "aria-label": t.close, title: t.close, html: ICON.close, onclick: closeLb });
    [fav, dl, playBtn, full, close].forEach(function (b) { if (b) top.appendChild(b); });
    var progress = el("div", { class: "lb-progress" });
    var root = el("div", { class: "lb", role: "dialog", "aria-modal": "true", "aria-label": t.photo, hidden: "" }, [top, stage, el("div", { class: "lb-bottom" }, [progress])]);
    document.body.appendChild(root);
    lb = { root: root, img: img, where: where, fav: fav, dl: dl, play: playBtn, progress: progress, i: 0, playing: false, timer: null };
    // Gestos: deslizar para mudar, tocar para alternar a barra.
    var x0 = null, y0 = 0;
    stage.addEventListener("pointerdown", function (e) { x0 = e.clientX; y0 = e.clientY; });
    stage.addEventListener("pointerup", function (e) {
      if (x0 == null) return;
      var dx = e.clientX - x0, dy = e.clientY - y0;
      x0 = null;
      if (Math.abs(dx) > 50 && Math.abs(dx) > Math.abs(dy)) step(dx < 0 ? 1 : -1);
    });
    document.addEventListener("keydown", function (e) {
      if (!lb || lb.root.hidden || document.querySelector(".dlg")) return;
      if (e.key === "ArrowRight") step(1);
      else if (e.key === "ArrowLeft") step(-1);
      else if (e.key === "Escape") closeLb();
      else if (e.key === " ") { e.preventDefault(); setPlaying(!lb.playing); }
      else if ((e.key === "f" || e.key === "F") && G.features.favorites) toggleFav(lb.i);
      else return;
    });
    setPlaying(false);
  }

  function lbFav() {
    if (!lb.fav) return;
    var on = !!favs[G.photos[lb.i].id];
    lb.fav.innerHTML = on ? ICON.heart : ICON.heartLine;
    lb.fav.setAttribute("aria-pressed", on ? "true" : "false");
    lb.fav.setAttribute("aria-label", on ? t.unfav : t.fav);
    lb.fav.title = on ? t.unfav : t.fav;
  }

  function show(i, first) {
    lb.i = i;
    var p = G.photos[i];
    var pos = order.indexOf(i);
    var chapter = G.chapters[chapterOf[i]].title;
    lb.where.textContent = (pos + 1) + " " + t.of + " " + order.length + (chapter ? "  ·  " + chapter : "");
    var swap = function () {
      lb.img.src = p.s;
      lb.img.alt = t.photo + " " + (pos + 1) + (chapter ? " — " + chapter : "");
      lb.img.classList.remove("out");
    };
    if (first) swap();
    else {
      lb.img.classList.add("out");
      var pre = new Image();
      pre.onload = pre.onerror = function () { setTimeout(swap, 120); };
      pre.src = p.s;
    }
    if (lb.dl) {
      lb.dl.hidden = !p.d;
      if (p.d) { lb.dl.href = p.d; lb.dl.setAttribute("download", p.dn || p.n + ".jpg"); }
    }
    lbFav();
    // Pré-carregar as vizinhas.
    [1, -1].forEach(function (d) {
      var j = order[(pos + d + order.length) % order.length];
      if (j != null) new Image().src = G.photos[j].s;
    });
    if (history.replaceState) history.replaceState(null, "", "#foto=" + p.id);
    if (lb.playing) restartTimer();
  }

  function step(d) {
    if (!order.length) return;
    var pos = order.indexOf(lb.i);
    show(order[(pos + d + order.length) % order.length]);
  }

  function restartTimer() {
    clearTimeout(lb.timer);
    var ms = (G.features.slideshow || 4) * 1000;
    lb.progress.style.animation = "none";
    void lb.progress.offsetWidth;
    lb.progress.style.animation = "";
    lb.progress.style.animationDuration = ms + "ms";
    lb.timer = setTimeout(function () { step(1); }, ms);
  }

  function setPlaying(on) {
    lb.playing = on;
    lb.root.classList.toggle("playing", on);
    lb.play.innerHTML = on ? ICON.pause : ICON.play;
    lb.play.setAttribute("aria-label", on ? t.pause : t.play);
    lb.play.title = on ? t.pause : t.slideshow;
    if (on) restartTimer();
    else clearTimeout(lb.timer);
  }

  function toggleFull() {
    if (document.fullscreenElement) document.exitFullscreen();
    else lb.root.requestFullscreen().catch(function () {});
  }

  function closeLb() {
    setPlaying(false);
    if (document.fullscreenElement) document.exitFullscreen().catch(function () {});
    lb.root.classList.remove("open");
    document.body.style.overflow = "";
    setTimeout(function () { lb.root.hidden = true; }, 250);
    if (history.replaceState) history.replaceState(null, "", location.pathname + location.search);
    var back = tiles[lb.i] && tiles[lb.i].el;
    if (back) back.focus({ preventScroll: true });
  }

  function fromHash() {
    var m = /#foto=([0-9a-f]+)/.exec(location.hash);
    if (!m) return;
    for (var i = 0; i < G.photos.length; i++) {
      if (G.photos[i].id === m[1]) { if (!lb || lb.root.hidden) open(i); return; }
    }
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", boot);
  else boot();
})();
