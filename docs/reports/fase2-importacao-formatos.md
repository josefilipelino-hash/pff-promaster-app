# Fase 2 — Importação real de todos os formatos

Gerado por `cargo test -p momento-project-management --release --test real_formats` em
2026-11-08: importação pelo mesmo caminho da aplicação (`ProjectSession`), seguida de
miniatura, preview 2560 px e resolução total de cada fotografia. Inclui ficheiros reais de
câmara, ficheiros produzidos por codificadores independentes (ImageMagick/libwebp/OpenJPEG,
libjxl, OpenEXR 3.5) e ficheiros malformados. Ambiente Linux: HEIC/AVIF aparecem como
“codec em falta” (sem ImageIO/WIC).

| Ficheiro | Formato | Resultado | Dimensões | Bits | Cor | Câmara | Miniatura | Preview 2560 | Resolução total |
|---|---|---|---|---|---|---|---|---|---|
| RAW_SIGMA_SD9_SRGB.X3F | x3f | ✅ importada | 2266×1510 | 12 | Câmara (sensor Foveon) | Sigma SD9 | ✅ | ✅ 585 ms | ✅ gerada 572 ms |
| Canon EOS 10D.crw | crw | ✅ importada | 2056×3088 | 12 | Câmara (RGBG, 3 cores) | Canon EOS 10D | ✅ | ✅ 885 ms | ✅ gerada 797 ms |
| Nikon D1X.webp | webp | ✅ importada | 600×391 | 8 | sRGB (sem perfil embutido — assumido) | NIKON CORPORATION NIKON D1X | ✅ | ✅ 12 ms | ✅ gerada 11 ms |
| Kodak DCS Pro SLRn.dcr | raw-tiff | ✅ importada | 3012×4516 | 12 | Câmara (RGBG, 3 cores) | Kodak DCS Pro SLR/n | ✅ | ✅ 1517 ms | ✅ gerada 1699 ms |
| Nikon D70.nef | raw-tiff | ✅ importada | 2014×3039 | 12 | Câmara (RGBG, 3 cores) | Nikon D70 | ✅ | ✅ 0 ms | ✅ gerada 807 ms |
| RAW_CANON_40D_SRAW_V103.CR2 | cr2 | ✅ importada | 1944×1296 | 15 | Câmara (RGBG, 3 cores) | Canon EOS 40D | ✅ | ✅ 0 ms | ✅ gerada 247 ms |
| Sony NEX-5N.arw | raw-tiff | ✅ importada | 4920×3276 | 14 | Câmara (RGBG, 3 cores) | Sony NEX-5N | ✅ | ✅ 0 ms | ✅ gerada 2000 ms |
| Canon EOS 7D.cr2 | cr2 | ✅ importada | 3464×5202 | 14 | Câmara (RGBG, 3 cores) | Canon EOS 7D | ✅ | ✅ 0 ms | ✅ gerada 2619 ms |
| iss030e122639.NEF | raw-tiff | ✅ importada | 4284×2844 | 12 | Câmara (RGBG, 3 cores) | Nikon D3S | ✅ | ✅ 0 ms | ✅ gerada 1654 ms |
| Fujifilm X-E1.raf | raf | ✅ importada | 4952×3288 | 12 | Câmara (RGBG, 3 cores) | Fujifilm X-E1 | ✅ | ✅ 0 ms | ✅ gerada 8947 ms |
| FujiFilm FinePix S5500.raf | raf | ✅ importada | 2304×1734 | 12 | Câmara (RGBG, 3 cores) | Fujifilm S5500 | ✅ | ✅ 439 ms | ✅ gerada 452 ms |
| Leica T (Typ 701).dng | raw-tiff | ✅ importada | 4944×3278 | 12 | Câmara (RGBG, 3 cores) | Leica T (Typ 701) | ✅ | ✅ 1925 ms | ✅ gerada 2023 ms |
| Samsung NX1.srw | raw-tiff | ✅ importada | 6496×4336 | 14 | Câmara (RGBG, 3 cores) | Samsung NX1 | ✅ | ✅ 0 ms | ✅ gerada 3747 ms |
| Leica D-LUX (Typ 109).rwl | rw2 | ✅ importada | 4288×2872 | 12 | Câmara (RGBG, 3 cores) | Leica D-LUX (Typ 109) | ✅ | ✅ 0 ms | ✅ gerada 1635 ms |
| Pentax QS1.dng | raw-tiff | ✅ importada | 4000×3000 | 12 | Câmara (RGBG, 3 cores) | Pentax Q-S1 | ✅ | ✅ 0 ms | ✅ gerada 1426 ms |
| Panasonic DMC-GF7.rw2 | rw2 | ✅ importada | 4608×3464 | 12 | Câmara (RGBG, 3 cores) | Panasonic DMC-GF7 | ✅ | ✅ 0 ms | ✅ gerada 2278 ms |
| Nikon D810.nef | raw-tiff | ✅ importada | 4928×7380 | 14 | Câmara (RGBG, 3 cores) | Nikon D810 | ✅ | ✅ 0 ms | ✅ gerada 5078 ms |
| Olympus E-M10.orf | orf | ✅ importada | 4640×3472 | 12 | Câmara (RGBG, 3 cores) | Olympus E-M10 | ✅ | ✅ 0 ms | ✅ gerada 2116 ms |
| DJI Phantom 4 (1).dng | raw-tiff | ✅ importada | 4000×3000 | 16 | Câmara (RGBG, 3 cores) | DJI FC330 | ✅ | ✅ 1351 ms | ✅ gerada 1340 ms |
| LG Nexus 5X.dng | raw-tiff | ✅ importada | 4032×3024 | 16 | Câmara (RGBG, 3 cores) | LG Nexus 5X | ✅ | ✅ 1349 ms | ✅ gerada 1384 ms |
| Pentax K-1 Mark II.pef | raw-tiff | ✅ importada | 7376×4932 | 14 | Câmara (RGBG, 3 cores) | Pentax K-1 Mark II | ✅ | ✅ 0 ms | ✅ gerada 5151 ms |
| Sony ILCE-7M4 (A7M4).ARW | raw-tiff | ✅ importada | 7028×4688 | 14 | Câmara (RGBG, 3 cores) | Sony ILCE-7M4 | ✅ | ✅ 0 ms | ✅ gerada 4340 ms |
| Canon EOS R3.cr3 | cr3 | ✅ importada | 6032×4032 | 14 | Câmara (RGBG, 3 cores) | Canon EOS R3 | ✅ | ✅ 0 ms | ✅ gerada 3092 ms |
| IMG_0036.CR3 | cr3 | ✅ importada | 6022×4024 | 14 | Câmara (RGBG, 3 cores) | Canon EOS R8 | ✅ | ✅ 0 ms | ✅ gerada 3452 ms |
| 10x12x16bit-CMYK.psd | psd | ⚠️ importada, suporte parcial — PSD em CMYK: conversão simplificada para RGB (sem perfil de cor); as cores podem não ser exatas. | 10×12 | 16 | CMYK |  | ✅ | ✅ 1 ms | ✅ gerada 1 ms |
| 8x4x8bit-Grayscale.psd | psd | ✅ importada | 8×4 | 8 | Escala de cinzentos |  | ✅ | ✅ 1 ms | ✅ gerada 1 ms |
| Alpha Lossy (1).webp | webp | ✅ importada | 400×301 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 7 ms | ✅ gerada 7 ms |
| Animation Lossy.webp | webp | ✅ importada | 320×240 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 5 ms | ✅ gerada 5 ms |
| BlazRobar Thinking Head Icon Set.psd | psd | ✅ importada | 2113×4925 | 8 | RGB |  | ✅ | ✅ 268 ms | ✅ gerada 306 ms |
| OM Sytem OM-1.orf | orf | ✅ importada | 5220×3912 | 12 | Câmara (RGBG, 3 cores) | OM Digital OM-1 | ✅ | ✅ 0 ms | ✅ gerada 2954 ms |
| Sigma DP2 Quattro.x3f | x3f | ✅ importada | 5446×3624 | 12 | Câmara (sensor Foveon) | Sigma dp2 Quattro | ✅ | ✅ 0 ms | ✅ gerada 2432 ms |
| balloon.jp2 | jp2 | ✅ importada | 2717×3701 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 1477 ms | ✅ gerada 963 ms |
| bigtiff.dng | bigtiff | ✅ importada | 16×12 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 1 ms | ✅ gerada 1 ms |
| img-16bit.tif | tiff | ✅ importada | 1175×1600 | 16 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 100 ms | ✅ gerada 96 ms |
| img-alpha.png | png | ✅ importada | 1175×1600 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 77 ms | ✅ original |
| img-bigtiff.tif | tiff | ✅ importada | 1175×1600 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 88 ms | ✅ gerada 94 ms |
| img-cmyk.jpg | jpeg | ⚠️ importada, suporte parcial — JPEG em CMYK: conversão simplificada para RGB (sem gestão de cor); as cores podem não ser exatas. | 1175×1600 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 78 ms | ✅ original |
| img-lossless.webp | webp | ✅ importada | 1175×1600 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 94 ms | ✅ gerada 96 ms |
| img-lossy.webp | webp | ✅ importada | 1175×1600 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 109 ms | ✅ gerada 112 ms |
| img.bmp | bmp | ✅ importada | 1175×1600 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 57 ms | ✅ gerada 60 ms |
| img.exr | exr | ⚠️ importada, suporte parcial — Imagem HDR: a pré-visualização converte para sRGB e corta os valores acima do branco (sem mapeamento de tons). | 1175×1600 | 32 | Linear (HDR) |  | ✅ | ✅ 194 ms | ✅ gerada 192 ms |
| img.gif | gif | ⚠️ importada, suporte parcial — GIF: só o primeiro fotograma é mostrado. | 1175×1600 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 80 ms | ✅ gerada 79 ms |
| img.hdr | hdr | ⚠️ importada, suporte parcial — Imagem HDR: a pré-visualização converte para sRGB e corta os valores acima do branco (sem mapeamento de tons). | 1175×1600 | 32 | Linear (HDR) |  | ✅ | ✅ 112 ms | ✅ gerada 113 ms |
| img.ico | ico | ✅ importada | 188×256 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 3 ms | ✅ gerada 3 ms |
| img.jp2 | jp2 | ✅ importada | 1175×1600 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 1057 ms | ✅ gerada 579 ms |
| img.jxl | jxl | ✅ importada | 1175×1600 | 8 | sRGB |  | ✅ | ✅ 178 ms | ✅ gerada 190 ms |
| img.pam | pam | ✅ importada | 1175×1600 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 54 ms | ✅ gerada 54 ms |
| img.ppm | pnm | ✅ importada | 1175×1600 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 52 ms | ✅ gerada 55 ms |
| img.psd | psd | ✅ importada | 1175×1600 | 8 | RGB |  | ✅ | ✅ 74 ms | ✅ gerada 73 ms |
| img.tga | tga | ✅ importada | 1175×1600 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 59 ms | ✅ gerada 59 ms |
| master.png | png | ✅ importada | 1175×1600 | 8 | sRGB (sem perfil embutido — assumido) |  | ✅ | ✅ 81 ms | ✅ original |
| AssertionError.PsdReader.extract.psd | — | 🟥 Corrupt — PSD: fim inesperado do ficheiro | — | — | — | — | — | — | — |
| Issue 419.rw2 | — | 🟥 Corrupt — ficheiro RAW incompleto ou danificado (368 bytes): abrir RAW: Unsupported file format or not RAW file (LibRaw -2) | — | — | — | — | — | — | — |
| 32Bit-72dpi-sRGB-800x600.avif | — | 🟧 MissingCodec — formato avif reconhecido, mas sem descodificador nesta plataforma | — | — | — | — | — | — | — |
| IMG_1034.heic | — | 🟧 MissingCodec — formato heic reconhecido, mas sem descodificador nesta plataforma | — | — | — | — | — | — | — |
| Issue 649.avif | — | 🟧 MissingCodec — formato avif reconhecido, mas sem descodificador nesta plataforma | — | — | — | — | — | — | — |
| NegativeArraySizeException.ItemInfoBox.init.heif | — | 🟧 MissingCodec — formato heic reconhecido, mas sem descodificador nesta plataforma | — | — | — | — | — | — | — |
| NullPointerException.HeifPictureHandler.processBox.heif | — | 🟧 MissingCodec — formato heic reconhecido, mas sem descodificador nesta plataforma | — | — | — | — | — | — | — |
| Sony ILCE-7M4 (A7M4).heif | — | 🟧 MissingCodec — formato heic reconhecido, mas sem descodificador nesta plataforma | — | — | — | — | — | — | — |
| cheers_1440x960.heic | — | 🟧 MissingCodec — formato heic reconhecido, mas sem descodificador nesta plataforma | — | — | — | — | — | — | — |
| exif-at-eof.heic | — | 🟧 MissingCodec — formato heic reconhecido, mas sem descodificador nesta plataforma | — | — | — | — | — | — | — |
| img.avif | — | 🟧 MissingCodec — formato avif reconhecido, mas sem descodificador nesta plataforma | — | — | — | — | — | — | — |
| img.exr | — | ⬜ Duplicate — conteúdo idêntico a outra fotografia | — | — | — | — | — | — | — |
| img.jxl | — | ⬜ Duplicate — conteúdo idêntico a outra fotografia | — | — | — | — | — | — | — |
| img.psd | — | ⬜ Duplicate — conteúdo idêntico a outra fotografia | — | — | — | — | — | — | — |

Importação: 1.33s · miniaturas: 3.56s · contagens: SessionCounts { total: 65, pending: 0, imported: 51, duplicate: 3, already_imported: 0, unsupported: 0, missing_codec: 9, corrupt: 2, error: 0 }
Por estado: {"Corrupt": 2, "Duplicate": 3, "MissingCodec": 9, "importada": 46, "importada (parcial)": 5}
