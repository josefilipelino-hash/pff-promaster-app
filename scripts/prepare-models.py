#!/usr/bin/env python3
"""Descarrega os modelos MediaPipe oficiais (Google, Apache-2.0), verifica o SHA-256 e
converte-os para ONNX com o tf2onnx, para serem executados pelo `tract-onnx` (Rust puro).

Os modelos não são guardados no Git (ver .gitignore e LICENSES.md). Este script é
determinístico para as versões fixadas abaixo; o CI executa-o antes de compilar.

Requisitos (só em desenvolvimento/CI):
    python3 -m venv .venv-models
    .venv-models/bin/pip install tf2onnx==1.17.0 tensorflow-cpu==2.21.0 onnx==1.23.2
    .venv-models/bin/python scripts/prepare-models.py
"""
import hashlib
import io
import json
import os
import subprocess
import sys
import tempfile
import urllib.request
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.environ.get("MOMENTO_MODELS_DIR", os.path.join(ROOT, "models"))
BASE = "https://storage.googleapis.com/mediapipe-models/"

SOURCES = [
    {
        "id": "face_landmarker",
        "url": BASE + "face_landmarker/face_landmarker/float16/latest/face_landmarker.task",
        "sha256": "64184e229b263107bc2b804c6625db1341ff2bb731874b0bcc2fe6544e0bc9ff",
        # Ficheiros dentro do .task (zip) a converter.
        "members": {
            "face_detector.tflite": "face_detector.onnx",
            "face_landmarks_detector.tflite": "face_landmarks_detector.onnx",
            "face_blendshapes.tflite": "face_blendshapes.onnx",
        },
    },
    {
        "id": "image_embedder",
        "url": BASE + "image_embedder/mobilenet_v3_small/float32/latest/mobilenet_v3_small.tflite",
        "sha256": "bbbb4c51a55a53905af1daec995ca1aae355046f8839bb8c9f5ce9271394bc40",
        "file": "image_embedder.onnx",
    },
]


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def convert(tflite_bytes: bytes, out_name: str) -> dict:
    with tempfile.TemporaryDirectory() as tmp:
        src = os.path.join(tmp, "model.tflite")
        with open(src, "wb") as f:
            f.write(tflite_bytes)
        dst = os.path.join(OUT, out_name)
        subprocess.run(
            [sys.executable, "-m", "tf2onnx.convert", "--tflite", src, "--output", dst, "--opset", "17"],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    with open(dst, "rb") as f:
        data = f.read()
    print(f"  {out_name}: {len(data) // 1024} KB")
    return {"file": out_name, "sha256": sha256(data), "bytes": len(data), "source_sha256": sha256(tflite_bytes)}


def main() -> None:
    os.makedirs(OUT, exist_ok=True)
    produced = []
    for s in SOURCES:
        print(f"{s['id']}: {s['url']}")
        data = urllib.request.urlopen(s["url"], timeout=120).read()
        digest = sha256(data)
        if digest != s["sha256"]:
            sys.exit(f"SHA-256 inesperado para {s['id']}: {digest} (esperado {s['sha256']})")
        if "members" in s:
            with zipfile.ZipFile(io.BytesIO(data)) as z:
                for member, out_name in s["members"].items():
                    produced.append(convert(z.read(member), out_name))
        else:
            produced.append(convert(data, s["file"]))
    with open(os.path.join(OUT, "models.lock.json"), "w") as f:
        json.dump({"generator": "scripts/prepare-models.py", "models": produced}, f, indent=2)
    print(f"Modelos prontos em {OUT}")


if __name__ == "__main__":
    main()
