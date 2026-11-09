# Dependências e licenças

Projeto de uso pessoal, sem distribuição comercial. Mesmo assim, a política é preferir
licenças permissivas (MIT, Apache-2.0, BSD, Zlib, ISC, MPL-2.0) e isolar copyleft forte,
para não fechar opções futuras.

## Política

| Categoria | Regra |
|---|---|
| MIT, Apache-2.0, BSD-2/3, ISC, Zlib, Unicode, CC0 | Permitidas |
| MPL-2.0 | Permitida (copyleft ao nível do ficheiro) |
| LGPL / CDDL | Permitidas com isolamento: LibRaw sob **CDDL-1.0**; libheif/libde265 só em builds Windows/Linux, ligadas dinamicamente |
| GPL / AGPL | **Não ligadas ao binário.** Só como ferramentas externas opcionais (ex.: ExifTool, FFmpeg com build GPL) instaladas pelo utilizador |
| Pesos de modelos “não comerciais” | Só como opção, nunca predefinidos, assinalados na interface |
| Sem licença clara | Proibido |

Verificação automática das licenças transitivas com `cargo-deny` e `pnpm licenses list`:
ainda **não** implementada (adiada para a Fase 2; as licenças diretas foram verificadas manualmente no crates.io e no npm).

## Dependências em uso (Fase 3)

### Rust (diretas)

| Crate | Versão | Licença | Uso |
|---|---|---|---|
| tauri | 2.12.5 | MIT OR Apache-2.0 | shell desktop |
| tauri-build | 2.7 | MIT OR Apache-2.0 | build |
| rusqlite (feature `bundled`) | 0.40.2 | MIT | SQLite |
| SQLite (via libsqlite3-sys 0.38) | 3.x | Domínio público | base de dados |
| serde, serde_json | 1.0 | MIT OR Apache-2.0 | serialização |
| thiserror | 2.0 | MIT OR Apache-2.0 | erros |
| tracing | 0.1 | MIT | registos |
| tempfile (testes) | 3 | MIT OR Apache-2.0 | testes |
| tauri-plugin-dialog | 2.8.4 | Apache-2.0 OR MIT | seletores de ficheiros/pastas nativos |
| image (features jpeg, png, tiff) | 0.25.10 | MIT OR Apache-2.0 | descodificação PNG/TIFF, codificação JPEG |
| jpeg-decoder | 0.3.2 | MIT OR Apache-2.0 | JPEG com redução DCT (miniaturas rápidas) |
| kamadak-exif | 0.6 | BSD-2-Clause | leitura de EXIF |
| blake3 | 1.8 | CC0-1.0 OR Apache-2.0 | hash de conteúdo (duplicados, verificação de cópias) |
| walkdir | 2.5 | Unlicense OR MIT | varrimento de pastas |
| rayon | 1.12 | MIT OR Apache-2.0 | processamento paralelo |
| tracing-subscriber / tracing-appender | 0.3 / 0.2 | MIT | registos em ficheiro |
| **LibRaw** (submódulo, compilada do código-fonte) | 0.22.3 | **CDDL-1.0** (escolhida; dupla com LGPL-2.1) | RAW de câmara |
| zlib (via libz-sys, estática) | libz-sys 1.1.31 | Zlib (biblioteca) · MIT OR Apache-2.0 (crate) | DNG com compressão deflate |
| cc (compilação) | 1.10 | MIT OR Apache-2.0 | compila a LibRaw |
| jxl-oxide | 0.12.6 | MIT OR Apache-2.0 | JPEG XL |
| jpeg2k + openjp2 (Rust) | 0.10.1 / 0.6.1 | MIT OR Apache-2.0 / BSD-2-Clause | JPEG 2000 |
| image-webp (via image) | 0.2.4 | MIT OR Apache-2.0 | WebP |
| objc2-image-io, objc2-core-graphics, objc2-core-foundation (só macOS) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | HEIC/HEIF/AVIF via ImageIO |
| windows (só Windows) | 0.62.2 | MIT OR Apache-2.0 | HEIC/HEIF/AVIF via WIC |
| **tract-onnx** (+ tract-core, tract-hir, tract-linalg…) | 0.23.9 | MIT OR Apache-2.0 | inferência dos modelos de IA (Fase 3) |
| anyhow | 1 | MIT OR Apache-2.0 | erros internos da inferência |

| **lcms2** + lcms2-sys (LittleCMS 2.17, compilada do código-fonte) | 6.2.0 / 4.0.7 | MIT | gestão de cor da exportação (Fase 9) |
| jpeg-encoder | 0.7.1 | (MIT OR Apache-2.0) AND IJG | JPEG (exportação) |
| png / tiff / exr (diretas) | 0.18.1 / 0.11.3 / 1.74.2 | MIT OR Apache-2.0 / MIT / BSD-3-Clause | PNG, TIFF/BigTIFF, OpenEXR (exportação) |
| webp + libwebp-sys (libwebp compilada do código-fonte) | 0.3.1 / 0.9.6 | MIT OR Apache-2.0 / MIT (libwebp: BSD-3-Clause) | WebP (exportação) |
| rav1e | 0.8.1 | BSD-2-Clause | codificador AV1 (AVIF) |
| avif-serialize | 0.8.9 | BSD-3-Clause | contentor AVIF |
| zune-jpegxl + zune-core | 0.5.2 / 0.5.3 | MIT OR Apache-2.0 OR Zlib | JPEG XL sem perdas |
| openjp2 (codificação) | 0.6.1 | BSD-2-Clause | JPEG 2000 |
| fs4 | 1.1.0 | MIT OR Apache-2.0 | espaço livre no destino |
| ab_glyph | 0.2 | Apache-2.0 | texto da marca de água |
| **Fonte Inter** (`packages/export-engine/assets/fonts`) | 4.0 | **OFL-1.1** (licença ao lado da fonte) | marca de água de texto |

**LibRaw sob CDDL-1.0:** o copyleft da CDDL aplica-se apenas aos ficheiros da própria
LibRaw (que não modificamos e cujo código-fonte está disponível no submódulo). Permite a
ligação estática ao resto da aplicação. Os ficheiros `LICENSE.CDDL` e `COPYRIGHT` da LibRaw
devem acompanhar qualquer distribuição binária (a incluir no instalador na Fase 9).

**Política de idade das versões (7 dias):** o pnpm aplica-a automaticamente; para o Cargo,
`scripts/pin-mature-crates.py` fixa no `Cargo.lock` a versão mais recente com pelo menos
7 dias (aplicado na Fase 2 a 20 crates transitivas).

### JavaScript/TypeScript (diretas)

| Pacote | Versão | Licença |
|---|---|---|
| react, react-dom | 19.3.1 | MIT |
| @tauri-apps/api, @tauri-apps/cli | 2.12.4 | MIT OR Apache-2.0 |
| i18next | 26.5.10 | MIT |
| react-i18next | 17.1.8 | MIT |
| vite | 7.3.7 | MIT |
| @vitejs/plugin-react | 5.2.0 | MIT |
| typescript | 5.9.3 | Apache-2.0 |
| vitest | 4.1.12 | MIT |
| @testing-library/react / dom | 16.3.3 / 10.4.2 | MIT |
| jsdom | 27.4.0 | MIT |
| @types/react, @types/react-dom | 19.3.0 | MIT |
| @tauri-apps/plugin-dialog | 2.8.4 | MIT OR Apache-2.0 |
| zustand | 5.0.16 | MIT |

Todas as dependências da Fase 1 são Rust puro ou JavaScript: **nenhuma biblioteca C/C++
precisa de ser compilada** (exceto o SQLite incluído no `rusqlite`), o que simplifica os
builds para macOS e Windows.

## Dependências planeadas (verificadas na Fase 0, adicionadas na fase indicada)

| Componente | Crate/biblioteca | Versão verificada | Licença | Fase |
|---|---|---|---|---|
| Concorrência assíncrona | tokio | 1.54 | MIT | quando necessário |
| Tipos TS gerados | ts-rs | 12 | MIT | 1 |
| XMP | xmp_toolkit (Adobe XMP Toolkit) | 1.12 | MIT OR Apache-2.0 (toolkit: BSD-3) | 2 |
| macOS ImageIO / Vision / Core ML / AppKit | objc2-image-io, objc2-vision, objc2-core-ml, objc2-app-kit | 0.3 | Zlib OR Apache-2.0 OR MIT (bindings); frameworks do sistema | 2–5 |
| Gestão de cor | lcms2 (LittleCMS 2) | 6.2 | MIT | 2 |
| IA (acelerada, se necessário) | ort (ONNX Runtime 1.31) | 2.0.0-rc.15 | MIT OR Apache-2.0 (ORT: MIT) | adiada (D-024) |
| Correção de lente | lensfun (porta Rust) / base de dados Lensfun | 0.7 | **LGPL-3.0-or-later** (crate em Rust, ligação estática → reavaliar ou usar só os dados); dados CC-BY-SA-3.0 | 4 |
| GPU | wgpu | 30 | MIT OR Apache-2.0 | 9 |
| JPEG rápido | turbojpeg (libjpeg-turbo) | 1.5 | Unlicense OR MIT (crate); IJG/BSD-3/Zlib | 1–2 se necessário |
| AVIF (escrita) | ravif | 0.13 | BSD-3-Clause | 6 |
| SVG | resvg | 0.49 | Apache-2.0 OR MIT | 6 |
| PDF | pdf-writer / krilla | 0.15 / 0.8 | MIT OR Apache-2.0 | 8 |
| ZIP | zip | 9 | MIT | 6 |
| Templates de galeria | minijinja | 3.1 | Apache-2.0 | 7 |
| HEIC/AVIF fora do macOS | libheif + libde265 + dav1d | — | LGPL-3 / LGPL-3 / BSD-2 | 9 |
| Metadados exóticos (opcional, externo) | ExifTool | — | Perl Artistic / GPL | 2 (opcional) |

Licenças das crates confirmadas no registo crates.io em 2026-11-08.

Nota: a escolha final de cada crate é revista no momento da adição (versão, manutenção,
licença exata do pacote publicado). Esta tabela confirma apenas que as bibliotecas
existem e são adequadas à data de 2026-11-08.

## Modelos de IA (Fase 3)

Ver `ARCHITECTURE.md §11` e D-024/D-025. Os pesos não são versionados: são descarregados
por `scripts/prepare-models.py` (URLs e SHA-256 fixados) e incluídos no instalador.

| Modelo | Origem | Licença dos pesos | Tamanho (ONNX) | Uso |
|---|---|---|---|---|
| BlazeFace short-range (`face_detector`) | Google MediaPipe, `face_landmarker.task` | Apache-2.0 | 0,4 MB | deteção de rostos |
| Face landmarks 478 (`face_landmarks_detector`) | idem | Apache-2.0 | 4,9 MB | pontos faciais, presença |
| Face blendshapes (`face_blendshapes`) | idem | Apache-2.0 | 1,9 MB | olhos fechados, sorriso, boca |
| MobileNetV3-small image embedder (`image_embedder`) | Google MediaPipe | Apache-2.0 | 4,2 MB | semelhança visual |
| EfficientNet-Lite2 image classifier (`scene_classifier`) | Google MediaPipe | Apache-2.0 | 23,8 MB | cena/objetos (momentos) |
| SFace (`face_identity`) | OpenCV Zoo (`face_recognition_sface_2021dec`) | Apache-2.0 | 37,8 MB | identidade facial (pessoas) |
| Selfie multiclass 256 (`segment_multiclass`) | Google MediaPipe Image Segmenter | Apache-2.0 | 16,5 MB | máscaras de pessoa, cabelo, pele, rosto, roupa, fundo (Fase 8) |
| Magic Touch (`segment_object`) | Google MediaPipe Interactive Segmenter | Apache-2.0 | 12,4 MB | máscara de objeto por clique (Fase 8) |
| FFDNet a cores (`denoise_ffdnet`) | KAIR, Kai Zhang (`ffdnet_color_clip.pth`, release v1.0) | MIT | 3,3 MB | AI Denoise (Fase 6), opcional |

O SFace é descarregado do objeto Git LFS oficial; se o LFS não estiver acessível, usa-se a
cópia incluída no pacote npm `maystar-facecompare@1.0.2` **apenas** porque os bytes são
idênticos ao oficial (SHA-256 `0ba9fbfa…4e79`, verificado pelo script). As etiquetas
ImageNet (`imagenet_labels.txt`) vêm do próprio modelo do MediaPipe.

O FFDNet (Zhang, Zuo & Zhang, *IEEE TIP* 2018) vem das *releases* oficiais do repositório
KAIR (`github.com/cszn/KAIR`, licença MIT, © 2019 Kai Zhang). O ficheiro `.pth` é verificado
por SHA-256 (`99a5081e…f5ce`) e lido **sem PyTorch e sem executar pickle**
(`scripts/torch_legacy.py` só interpreta os opcodes); a rede é reconstruída em ONNX com o
pacote `onnx` (Apache-2.0). O resultado é determinístico (SHA-256 em `models.lock.json`).

Licença confirmada no *model card* do MediaPipe Face Landmarker e do Image Embedder
(Apache-2.0). A conversão (tf2onnx 1.17, só no momento da preparação, não incluído na
aplicação) é Apache-2.0.
