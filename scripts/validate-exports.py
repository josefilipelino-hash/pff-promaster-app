#!/usr/bin/env python3
"""Valida os ficheiros gerados por `export_samples` com descodificadores independentes.

Uso:
  cargo run --release -p momento-export-engine --example export_samples -- foto.jpg pasta/
  python3 scripts/validate-exports.py pasta/

Para cada ficheiro: descodifica com uma biblioteca independente (Pillow, libjxl, OpenJPEG C,
libavif, libwebp, tifffile, psd-tools, pypdf, OpenEXR), confirma dimensões, profundidade,
perfil ICC / sinalização de cor e metadados (copyright presente, GPS ausente), e mede a
fidelidade (PSNR) contra a imagem de referência convertida para o espaço de saída.

Dependências (só para validar; não fazem parte da aplicação):
  pip install pillow imagecodecs tifffile psd-tools pypdf OpenEXR numpy
"""

import io
import json
import sys
from pathlib import Path

import numpy as np

D65 = (0.3127, 0.3290)
D50 = (0.3457, 0.3585)
SPACES = {
    "srgb": ((0.64, 0.33), (0.30, 0.60), (0.15, 0.06), D65, "srgb"),
    "displayP3": ((0.680, 0.320), (0.265, 0.690), (0.150, 0.060), D65, "srgb"),
    "adobeRgb": ((0.64, 0.33), (0.21, 0.71), (0.15, 0.06), D65, 563 / 256),
    "proPhoto": ((0.7347, 0.2653), (0.1596, 0.8404), (0.0366, 0.0001), D50, 1.8),
    "rec709": ((0.64, 0.33), (0.30, 0.60), (0.15, 0.06), D65, "bt709"),
    "rec2020": ((0.708, 0.292), (0.170, 0.797), (0.131, 0.046), D65, "bt709"),
}


def xyz(c):
    return np.array([c[0] / c[1], 1.0, (1 - c[0] - c[1]) / c[1]])


def rgb_to_xyz(p):
    m = np.stack([xyz(p[0]), xyz(p[1]), xyz(p[2])], axis=1)
    s = np.linalg.solve(m, xyz(p[3]))
    return m * s


def bradford(src, dst):
    b = np.array([[0.8951, 0.2664, -0.1614], [-0.7502, 1.7135, 0.0367], [0.0389, -0.0685, 1.0296]])
    cs, cd = b @ xyz(src), b @ xyz(dst)
    return np.linalg.inv(b) @ np.diag(cd / cs) @ b


def encode(x, curve):
    x = np.clip(x, 0, 1)
    if curve == "linear":
        return x
    if curve == "srgb":
        return np.where(x <= 0.0031308, 12.92 * x, 1.055 * np.power(x, 1 / 2.4) - 0.055)
    if curve == "bt709":
        return np.where(x < 0.018, 4.5 * x, 1.099 * np.power(x, 0.45) - 0.099)
    return np.power(x, 1 / curve)


def expected(ref, space, linear=False):
    p = SPACES[space]
    to_xyz = rgb_to_xyz(SPACES["rec2020"])
    if p[3] != D65:
        to_xyz = bradford(D65, p[3]) @ to_xyz
    m = np.linalg.inv(rgb_to_xyz(p)) @ to_xyz
    lin = ref @ m.T
    if linear:
        return np.maximum(lin, 0)
    return encode(lin, p[4])


def psnr(a, b):
    mse = float(np.mean((a - b) ** 2))
    return 99.0 if mse == 0 else 10 * np.log10(1.0 / mse)


def icc_desc(icc):
    from PIL import ImageCms

    if not icc:
        return None
    return ImageCms.getProfileDescription(ImageCms.ImageCmsProfile(io.BytesIO(icc))).strip()


def pil_meta(im):
    ex = im.getexif()
    gps = ex.get_ifd(0x8825)
    return {"copyright": ex.get(0x8298), "model": ex.get(0x0110), "gps": bool(gps)}


def decode(path, fmt):
    """Devolve (pixels normalizados 0–1 HxWx3, bits, icc, meta)."""
    data = path.read_bytes()
    if fmt in ("jpeg", "png", "webp", "avif"):
        from PIL import Image

        im = Image.open(io.BytesIO(data))
        icc = im.info.get("icc_profile")
        meta = pil_meta(im)
        meta["xmp"] = b"Est\xc3\xbadio" in data
        if fmt == "png" and im.mode in ("I;16", "I;16B", "RGB;16") or (fmt == "png" and b"IHDR" in data[:40] and data[24] == 16):
            import imagecodecs

            arr = imagecodecs.png_decode(data).astype(np.float64) / 65535
            return arr, 16, icc, meta
        if fmt == "avif":
            import imagecodecs

            arr = imagecodecs.avif_decode(data)
            bits = 8 if arr.dtype == np.uint8 else int(np.ceil(np.log2(arr.max() + 1)))
            maxv = 255 if arr.dtype == np.uint8 else (1 << bits_of_avif(data)) - 1
            return arr.astype(np.float64) / maxv, bits_of_avif(data), icc, meta
        arr = np.asarray(im.convert("RGB")).astype(np.float64) / 255
        return arr, 8, icc, meta
    if fmt in ("tiff", "bigTiff"):
        import tifffile

        with tifffile.TiffFile(io.BytesIO(data)) as t:
            page = t.pages[0]
            arr = page.asarray()
            icc = page.tags.get(34675).value if 34675 in page.tags else None
            meta = {
                "copyright": page.tags.get(33432).value if 33432 in page.tags else None,
                "model": page.tags.get(272).value if 272 in page.tags else None,
                "gps": 34853 in page.tags,
                "xmp": 700 in page.tags,
                "bigtiff": t.is_bigtiff,
            }
        if arr.dtype == np.float32:
            return arr.astype(np.float64), 32, icc, meta
        maxv = np.iinfo(arr.dtype).max
        return arr.astype(np.float64) / maxv, 16 if maxv > 255 else 8, icc, meta
    if fmt == "jpegXl":
        import imagecodecs

        arr = imagecodecs.jpegxl_decode(data)
        maxv = np.iinfo(arr.dtype).max
        return arr.astype(np.float64) / maxv, 16 if maxv > 255 else 8, None, {"xmp": b"xml " in data, "exif": b"Exif" in data[:200] or b"Exif" in data}
    if fmt == "jpeg2000":
        import imagecodecs

        arr = imagecodecs.jpeg2k_decode(data)
        bits = 8 if arr.dtype == np.uint8 else (12 if arr.max() < 4096 and b"colr" in data and data.find(b"ihdr") > 0 and data[data.find(b"ihdr") + 14] == 11 else 16)
        ihdr = data.find(b"ihdr")
        bits = data[ihdr + 14] + 1
        icc = None
        c = data.find(b"colr")
        if c > 0 and data[c + 4] == 2:
            size = int.from_bytes(data[c - 4 : c], "big")
            icc = data[c + 7 : c - 4 + size]
        return arr.astype(np.float64) / ((1 << bits) - 1), bits, icc, {}
    if fmt == "exr":
        import OpenEXR

        with OpenEXR.File(str(path)) as f:
            ch = f.channels()
            hdr = f.header()
            if "RGB" in ch:
                arr = ch["RGB"].pixels
            else:
                arr = np.stack([ch[k].pixels for k in "RGB"], axis=-1)
            meta = {"chromaticities": hdr.get("chromaticities"), "owner": hdr.get("owner")}
        return arr.astype(np.float64), 16 if arr.dtype == np.float16 else 32, None, meta
    if fmt == "psd":
        from psd_tools import PSDImage

        psd = PSDImage.open(io.BytesIO(data))
        arr = psd.numpy()[..., :3].astype(np.float64)
        icc = psd.image_resources.get_data(1039)
        return arr, psd.depth, icc, {"xmp": psd.image_resources.get_data(1060) is not None}
    if fmt == "pdf":
        from pypdf import PdfReader
        from PIL import Image

        r = PdfReader(io.BytesIO(data))
        page = r.pages[0]
        img = page.images[0]
        im = Image.open(io.BytesIO(img.data))
        arr = np.asarray(im.convert("RGB")).astype(np.float64) / 255
        box = page.mediabox
        meta = {"author": r.metadata.author if r.metadata else None, "mediabox": (float(box.width), float(box.height))}
        return arr, 8, None, meta
    raise ValueError(fmt)


def bits_of_avif(data):
    j = data.find(b"pixi")
    return data[j + 9]


THRESHOLD = {"lossless": 48.0, "fast": 48.0, "best": 48.0, "none": 48.0, "lzw": 48.0, "zip": 48.0, "piz": 48.0, "rle": 48.0}
LOSSY = {"jpeg": 38.0, "webp": 34.0, "avif": 34.0, "jpeg2000": 34.0, "pdf": 38.0}


def main():
    folder = Path(sys.argv[1])
    ref = np.frombuffer((folder / "reference.f32").read_bytes(), dtype="<f4").astype(np.float64)
    manifest = json.loads((folder / "manifest.json").read_text())
    w, h = manifest[0]["width"], manifest[0]["height"]
    ref = ref.reshape(h, w, 3)
    failures = 0
    print(f"{'ficheiro':52} {'dim':>10} {'bits':>4} {'PSNR':>6}  perfil / metadados")
    for m in manifest:
        path = folder / m["file"]
        fmt = m["format"]
        problems = []
        try:
            arr, bits, icc, meta = decode(path, fmt)
        except Exception as e:  # noqa: BLE001
            print(f"{m['file']:52} ERRO ao descodificar: {e}")
            failures += 1
            continue
        if arr.shape[:2] != (m["height"], m["width"]):
            problems.append(f"dimensões {arr.shape[:2]}")
        want_bits = {"8": 8, "10": 10, "12": 12, "16": 16, "16f": 16, "32f": 32}[m["depth"]]
        if bits != want_bits:
            problems.append(f"bits {bits}≠{want_bits}")
        linear = fmt == "exr" or m["depth"] == "32f"
        exp = expected(ref, m["space"], linear)
        if linear:
            # Vírgula flutuante: erro relativo.
            err = np.abs(arr - exp) / np.maximum(exp, 0.01)
            p = 99.0 if err.max() == 0 else -20 * np.log10(max(float(err.max()), 1e-12))
            limit = 40.0 if m["depth"] == "16f" else 90.0
        else:
            p = psnr(arr, exp)
            limit = LOSSY.get(fmt, 34.0) if m["compression"] == "lossy" else THRESHOLD.get(m["compression"], 48.0)
        if p < limit:
            problems.append(f"PSNR {p:.1f} < {limit}")
        desc = icc_desc(icc) if icc else None
        if fmt not in ("jpegXl", "exr", "avif", "pdf"):
            if desc != m["profile"]:
                problems.append(f"ICC '{desc}' ≠ '{m['profile']}'")
        if fmt in ("jpeg", "png", "webp", "tiff", "bigTiff"):
            if not meta.get("copyright") and not meta.get("xmp"):
                problems.append("sem copyright")
            if meta.get("gps"):
                problems.append("GPS presente")
        if fmt == "bigTiff" and not meta.get("bigtiff"):
            problems.append("não é BigTIFF")
        if fmt == "exr" and not meta.get("chromaticities"):
            problems.append("sem cromaticidades")
        info = desc or ""
        if fmt == "pdf":
            info = f"página {meta['mediabox'][0]:.1f}×{meta['mediabox'][1]:.1f} pt, autor {meta['author']}"
        if fmt == "exr":
            info = f"cromaticidades {meta['chromaticities']}"
        status = "OK " if not problems else "FALHA"
        failures += bool(problems)
        print(f"{m['file']:52} {m['width']:>4}×{m['height']:<5} {bits:>4} {p:6.1f}  {status} {info} {'; '.join(problems)}")
    print(f"\n{len(manifest) - failures} de {len(manifest)} ficheiros válidos")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
