#!/usr/bin/env python3
"""Descarrega os modelos MediaPipe oficiais (Google, Apache-2.0), verifica o SHA-256 e
converte-os para ONNX com o tf2onnx, para serem executados pelo `tract-onnx` (Rust puro).

Os modelos não são guardados no Git (ver .gitignore e LICENSES.md). As origens são verificadas por
SHA-256; os ONNX gerados pelo tf2onnx podem diferir byte a byte entre execuções (nomes
internos), mas são numericamente equivalentes. O CI executa-o antes de compilar.

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
    {
        # Fase 4: classificador de cena/objetos (1000 classes ImageNet) para os momentos.
        "id": "scene_classifier",
        "url": BASE + "image_classifier/efficientnet_lite2/float32/latest/efficientnet_lite2.tflite",
        "sha256": "94b6c84a5d6de20f932558de213b50e0338dcc06ab83d8d64b12128a476b4b83",
        "file": "scene_classifier.onnx",
    },
]

# Fase 4: identidade facial SFace (OpenCV Zoo, Apache-2.0), já em ONNX. O SHA-256 é o do
# objeto Git LFS oficial (opencv_zoo/models/face_recognition_sface). O LFS do GitHub nem
# sempre é acessível; como alternativa usa-se uma cópia publicada no npm e só se aceita se
# os bytes forem **idênticos** ao oficial (mesmo SHA-256).
SFACE = {
    "sha256": "0ba9fbfa01b5270c96627c4ef784da859931e02f04419c829e83484087c34e79",
    "urls": [
        "https://media.githubusercontent.com/media/opencv/opencv_zoo/main/models/face_recognition_sface/face_recognition_sface_2021dec.onnx",
    ],
    "npm_mirror": ("https://registry.npmjs.org/maystar-facecompare/-/maystar-facecompare-1.0.2.tgz", "package/weights/sface.onnx"),
    "file": "face_identity.onnx",
}


def fetch_sface() -> bytes:
    for url in SFACE["urls"]:
        try:
            data = urllib.request.urlopen(url, timeout=120).read()
            if sha256(data) == SFACE["sha256"]:
                return data
        except Exception as e:  # noqa: BLE001 — tenta a alternativa
            print(f"  {url}: {e}")
    import tarfile

    url, member = SFACE["npm_mirror"]
    print(f"  cópia verificada: {url}")
    raw = urllib.request.urlopen(url, timeout=600).read()
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as t:
        data = t.extractfile(member).read()
    if sha256(data) != SFACE["sha256"]:
        sys.exit("SFace: os bytes não coincidem com o modelo oficial do OpenCV Zoo")
    return data


def prepare_sface() -> dict:
    """Remove os inicializadores declarados como entradas (formato ONNX antigo)."""
    import onnx

    data = fetch_sface()
    m = onnx.load_from_string(data)
    inits = {i.name for i in m.graph.initializer}
    keep = [i for i in m.graph.input if i.name not in inits]
    del m.graph.input[:]
    m.graph.input.extend(keep)
    dst = os.path.join(OUT, SFACE["file"])
    onnx.save(m, dst)
    with open(dst, "rb") as f:
        out = f.read()
    print(f"  {SFACE['file']}: {len(out) // 1024} KB")
    return {"file": SFACE["file"], "sha256": sha256(out), "bytes": len(out), "source_sha256": SFACE["sha256"]}


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
    print("face_identity (SFace)")
    produced.append(prepare_sface())
    with open(os.path.join(OUT, "models.lock.json"), "w") as f:
        json.dump({"generator": "scripts/prepare-models.py", "models": produced}, f, indent=2)
    print(f"Modelos prontos em {OUT}")


if __name__ == "__main__":
    main()
