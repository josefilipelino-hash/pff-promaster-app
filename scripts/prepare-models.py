#!/usr/bin/env python3
"""Descarrega os modelos MediaPipe oficiais (Google, Apache-2.0), verifica o SHA-256 e
converte-os para ONNX com o tf2onnx, para serem executados pelo `tract-onnx` (Rust puro).

Os modelos não são guardados no Git (ver .gitignore e LICENSES.md). As origens são verificadas por
SHA-256; os ONNX gerados pelo tf2onnx podem diferir byte a byte entre execuções (nomes
internos), mas são numericamente equivalentes. O CI executa-o antes de compilar.

Fase 6: também o AI Denoise (FFDNet, pesos MIT do projeto KAIR), reconstruído em ONNX a
partir dos pesos sem PyTorch (`scripts/torch_legacy.py`). Só esse: `--only denoise`.

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
    {
        # Fase 8: máscaras de IA — fundo, cabelo, pele do corpo, pele do rosto, roupa, outros.
        "id": "segment_multiclass",
        "url": BASE + "image_segmenter/selfie_multiclass_256x256/float32/latest/selfie_multiclass_256x256.tflite",
        "sha256": "c6748b1253a99067ef71f7e26ca71096cd449baefa8f101900ea23016507e0e0",
        "file": "segment_multiclass.onnx",
    },
    {
        # Fase 8: objeto/pessoa sob um ponto clicado ("magic touch").
        "id": "segment_object",
        "url": BASE + "interactive_segmenter/magic_touch/float32/latest/magic_touch.tflite",
        "sha256": "e24338a717c1b7ad8d159666677ef400babb7f33b8ad60c4d96db4ecf694cd25",
        "file": "segment_object.onnx",
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


# Fase 6: AI Denoise — FFDNet (Zhang, Zuo & Zhang, IEEE TIP 2018), pesos oficiais do
# projeto KAIR (MIT, github.com/cszn/KAIR, release v1.0). A rede (12 convoluções 3×3 a
# meia resolução com o mapa do nível de ruído como entrada) é reconstruída em ONNX a partir
# dos pesos, lidos sem PyTorch por `torch_legacy.py`. O "pixel unshuffle" e o "pixel
# shuffle" são feitos no Rust (packages/face-analysis/src/denoise.rs).
FFDNET = {
    "url": "https://github.com/cszn/KAIR/releases/download/v1.0/ffdnet_color_clip.pth",
    "sha256": "99a5081e32afdaa25df3ded11aa16447556692e550906caed320db31324af5ce",
    "file": "denoise_ffdnet.onnx",
}


def prepare_ffdnet() -> dict:
    import numpy as np
    import onnx
    from onnx import TensorProto, helper, numpy_helper

    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import torch_legacy

    data = urllib.request.urlopen(FFDNET["url"], timeout=300).read()
    digest = sha256(data)
    if digest != FFDNET["sha256"]:
        sys.exit(f"SHA-256 inesperado para o FFDNet: {digest} (esperado {FFDNET['sha256']})")
    with tempfile.TemporaryDirectory() as tmp:
        src = os.path.join(tmp, "ffdnet.pth")
        with open(src, "wb") as f:
            f.write(data)
        w = torch_legacy.load(src)
    convs = sorted({int(k.split(".")[1]) for k in w})
    if len(convs) != 12 or w[f"model.{convs[0]}.weight"].shape != (96, 13, 3, 3):
        sys.exit("FFDNet: estrutura inesperada dos pesos")
    nodes = [helper.make_node("Concat", ["x", "sigma"], ["h0"], axis=1)]
    inits = []
    cur = "h0"
    for i, c in enumerate(convs):
        inits += [
            numpy_helper.from_array(w[f"model.{c}.weight"].astype(np.float32), f"w{i}"),
            numpy_helper.from_array(w[f"model.{c}.bias"].astype(np.float32), f"b{i}"),
        ]
        nodes.append(helper.make_node("Conv", [cur, f"w{i}", f"b{i}"], [f"c{i}"], pads=[1, 1, 1, 1], kernel_shape=[3, 3]))
        cur = f"c{i}"
        if i < len(convs) - 1:
            nodes.append(helper.make_node("Relu", [cur], [f"a{i}"]))
            cur = f"a{i}"
    nodes.append(helper.make_node("Identity", [cur], ["y"]))
    graph = helper.make_graph(
        nodes,
        "ffdnet_color_clip",
        [
            helper.make_tensor_value_info("x", TensorProto.FLOAT, [1, 12, "h", "w"]),
            helper.make_tensor_value_info("sigma", TensorProto.FLOAT, [1, 1, "h", "w"]),
        ],
        [helper.make_tensor_value_info("y", TensorProto.FLOAT, [1, 12, "h", "w"])],
        inits,
    )
    model = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 13)], producer_name="momento-prepare-models")
    model.ir_version = 8
    onnx.checker.check_model(model)
    dst = os.path.join(OUT, FFDNET["file"])
    onnx.save(model, dst)
    with open(dst, "rb") as f:
        out = f.read()
    print(f"  {FFDNET['file']}: {len(out) // 1024} KB")
    return {"file": FFDNET["file"], "sha256": sha256(out), "bytes": len(out), "source_sha256": FFDNET["sha256"]}


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
    """Sem argumentos prepara todos os modelos; `--only denoise` (ou outro id) só esses."""
    os.makedirs(OUT, exist_ok=True)
    only = set(sys.argv[sys.argv.index("--only") + 1 :]) if "--only" in sys.argv else None
    lock_path = os.path.join(OUT, "models.lock.json")
    produced = []
    if only is not None and os.path.isfile(lock_path):
        with open(lock_path) as f:
            produced = json.load(f).get("models", [])

    def put(entry: dict) -> None:
        produced[:] = [e for e in produced if e["file"] != entry["file"]] + [entry]

    for s in SOURCES:
        if only is not None and s["id"] not in only:
            continue
        print(f"{s['id']}: {s['url']}")
        data = urllib.request.urlopen(s["url"], timeout=120).read()
        digest = sha256(data)
        if digest != s["sha256"]:
            sys.exit(f"SHA-256 inesperado para {s['id']}: {digest} (esperado {s['sha256']})")
        if "members" in s:
            with zipfile.ZipFile(io.BytesIO(data)) as z:
                for member, out_name in s["members"].items():
                    put(convert(z.read(member), out_name))
        else:
            put(convert(data, s["file"]))
    if only is None or "face_identity" in only:
        print("face_identity (SFace)")
        put(prepare_sface())
    if only is None or "denoise" in only:
        print(f"denoise (FFDNet): {FFDNET['url']}")
        put(prepare_ffdnet())
    with open(lock_path, "w") as f:
        json.dump({"generator": "scripts/prepare-models.py", "models": produced}, f, indent=2)
    print(f"Modelos prontos em {OUT}")


if __name__ == "__main__":
    main()
