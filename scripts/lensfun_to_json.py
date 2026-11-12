#!/usr/bin/env python3
"""Converte a base de dados de objetivas da lensfun (XML, CC BY-SA 3.0) num JSON compacto
usado pela aplicação (packages/project-management/data/lensfun.json).

Uso: python3 scripts/lensfun_to_json.py <pasta lensfun/data/db> <saida.json> <versão>
Só guarda o necessário: câmaras (fabricante, modelo, encaixe, fator de corte), encaixes
compatíveis e, por objetiva, a calibração de distorção, TCA e vinheta.
"""
import json, sys, glob, os
import xml.etree.ElementTree as ET

src, out, version = sys.argv[1], sys.argv[2], sys.argv[3]

def text(el, tag):
    # Nome sem atributo lang (o nome "de origem").
    best = None
    for c in el.findall(tag):
        if "lang" not in c.attrib:
            return (c.text or "").strip()
        best = best or (c.text or "").strip()
    return best or ""

def num(v, d=0.0):
    try:
        return float(v)
    except (TypeError, ValueError):
        return d

cameras, lenses, mounts = [], [], {}
for path in sorted(glob.glob(os.path.join(src, "*.xml"))):
    root = ET.parse(path).getroot()
    for m in root.findall("mount"):
        mounts.setdefault(text(m, "name"), sorted({(c.text or "").strip() for c in m.findall("compat")}))
    for c in root.findall("camera"):
        cameras.append([text(c, "maker"), text(c, "model"), text(c, "variant"), text(c, "mount"), num(text(c, "cropfactor"), 1.0)])
    for l in root.findall("lens"):
        typ = text(l, "type") or "rectilinear"
        cal = l.find("calibration")
        if cal is None:
            continue
        dist, tca, vig = [], [], []
        for d in cal.findall("distortion"):
            m = d.get("model")
            f = num(d.get("focal"))
            if m == "ptlens":
                dist.append([f, 3, num(d.get("a")), num(d.get("b")), num(d.get("c"))])
            elif m == "poly3":
                dist.append([f, 1, num(d.get("k1")), 0.0, 0.0])
            elif m == "poly5":
                dist.append([f, 2, num(d.get("k1")), num(d.get("k2")), 0.0])
        for t in cal.findall("tca"):
            m = t.get("model")
            f = num(t.get("focal"))
            if m == "poly3":
                tca.append([f, num(t.get("vr"), 1), num(t.get("vb"), 1), num(t.get("cr")), num(t.get("cb")), num(t.get("br")), num(t.get("bb"))])
            elif m == "linear":
                tca.append([f, num(t.get("kr"), 1), num(t.get("kb"), 1), 0.0, 0.0, 0.0, 0.0])
        for v in cal.findall("vignetting"):
            if v.get("model") == "pa":
                vig.append([num(v.get("focal")), num(v.get("aperture")), num(v.get("distance"), 1000), num(v.get("k1")), num(v.get("k2")), num(v.get("k3"))])
        if not (dist or tca or vig):
            continue
        ar = text(l, "aspect-ratio")
        if ":" in ar:
            a, b = ar.split(":")
            ar = num(a) / max(num(b), 1e-9)
        else:
            ar = num(ar, 1.5) if ar else 1.5
        lenses.append({
            "k": text(l, "maker"), "m": text(l, "model"),
            "mt": [(x.text or "").strip() for x in l.findall("mount")],
            "c": num(text(l, "cropfactor"), 1.0), "a": ar, "t": typ,
            "d": dist, "tc": tca, "v": vig,
        })

json.dump({"source": "lensfun " + version + " (CC BY-SA 3.0, https://lensfun.github.io)",
           "mounts": mounts, "cameras": cameras, "lenses": lenses},
          open(out, "w"), separators=(",", ":"), ensure_ascii=False)
print(f"{len(cameras)} câmaras, {len(lenses)} objetivas, {os.path.getsize(out)/1e6:.2f} MB")
