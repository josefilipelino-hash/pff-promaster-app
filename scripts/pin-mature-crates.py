#!/usr/bin/env python3
"""Fixa no Cargo.lock as crates publicadas há menos de N dias (por omissão 7) na versão
mais recente com pelo menos N dias, dentro da mesma versão principal.

O pnpm aplica esta regra automaticamente (`minimumReleaseAge`); o Cargo não tem uma opção
equivalente, por isso este script é executado depois de adicionar ou atualizar crates.

Uso: python3 scripts/pin-mature-crates.py [dias]
"""
import datetime
import json
import re
import subprocess
import sys
import urllib.request

days = int(sys.argv[1]) if len(sys.argv) > 1 else 7
cut = datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(days=days)


def versions(name):
    req = urllib.request.Request(f"https://crates.io/api/v1/crates/{name}/versions", headers={"User-Agent": "momento-studio-dev"})
    return json.load(urllib.request.urlopen(req, timeout=20))["versions"]


def date(v):
    return datetime.datetime.fromisoformat(v["created_at"].replace("Z", "+00:00"))


for _ in range(3):
    lock = open("Cargo.lock").read()
    changed = False
    for name, ver in re.findall(r'name = "([^"]+)"\nversion = "([^"]+)"\nsource = "registry', lock):
        vs = versions(name)
        cur = [v for v in vs if v["num"] == ver]
        if not cur or date(cur[0]) <= cut:
            continue
        prefix = ".".join(ver.split(".")[:2]) if ver.startswith("0.") else ver.split(".")[0]
        older = [v["num"] for v in vs if not v["yanked"] and "-" not in v["num"].split("+")[0] and v["num"].startswith(prefix + ".") and date(v) <= cut]
        if not older:
            print(f"sem versão madura para {name} {ver}")
            continue
        r = subprocess.run(["cargo", "update", "-p", f"{name}@{ver}", "--precise", older[0]], capture_output=True, text=True)
        print(f"{name} {ver} -> {older[0]}: {'ok' if r.returncode == 0 else 'falhou'}")
        changed = changed or r.returncode == 0
    if not changed:
        break
