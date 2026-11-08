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
prevista para a Fase 1.

## Dependências em uso (Fase 0)

### Rust (diretas)

| Crate | Versão | Licença | Uso |
|---|---|---|---|
| tauri | 2.12.5 | MIT OR Apache-2.0 | shell desktop |
| tauri-build | 2.7 | MIT OR Apache-2.0 | build |
| rusqlite (feature `bundled`) | 0.40.2 | MIT | SQLite |
| SQLite (via libsqlite3-sys 0.38) | 3.x | Domínio público | base de dados |
| serde, serde_json | 1.0 | MIT OR Apache-2.0 | serialização |
| thiserror | 2.0 | MIT OR Apache-2.0 | erros |
| tracing | 0.1 | MIT | registos (declarada, uso na Fase 1) |
| tempfile (testes) | 3 | MIT OR Apache-2.0 | testes |

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

## Dependências planeadas (verificadas na Fase 0, adicionadas na fase indicada)

| Componente | Crate/biblioteca | Versão verificada | Licença | Fase |
|---|---|---|---|---|
| Descodificação raster | image | 0.25 | MIT OR Apache-2.0 | 1 |
| Hash | blake3 | 1.8 | CC0-1.0 OR Apache-2.0 | 1 |
| Concorrência | tokio / rayon | 1.54 / 1.12 | MIT | 1 |
| Tipos TS gerados | ts-rs | 12 | MIT | 1 |
| EXIF | kamadak-exif | 0.6 | BSD-2-Clause | 1 |
| XMP | xmp_toolkit (Adobe XMP Toolkit) | 1.12 | MIT OR Apache-2.0 (toolkit: BSD-3) | 2 |
| RAW | LibRaw (código-fonte, FFI própria) | ≥ 0.21 | LGPL-2.1 **ou CDDL-1.0 (escolhida)** | 2 |
| JPEG XL (leitura) | jxl-oxide | 0.12 | MIT OR Apache-2.0 | 2 |
| macOS ImageIO / Vision / Core ML / AppKit | objc2-image-io, objc2-vision, objc2-core-ml, objc2-app-kit | 0.3 | Zlib OR Apache-2.0 OR MIT (bindings); frameworks do sistema | 2–5 |
| Gestão de cor | lcms2 (LittleCMS 2) | 6.2 | MIT | 2 |
| IA | ort (ONNX Runtime 1.31) | 2.0.0-rc.15 | MIT OR Apache-2.0 (ORT: MIT) | 3 |
| Correção de lente | lensfun (porta Rust) / base de dados Lensfun | 0.7 | **LGPL-3.0-or-later** (crate em Rust, ligação estática → reavaliar ou usar só os dados); dados CC-BY-SA-3.0 | 4 |
| GPU | wgpu | 30 | MIT OR Apache-2.0 | 9 |
| JPEG rápido | turbojpeg (libjpeg-turbo) | 1.5 | Unlicense OR MIT (crate); IJG/BSD-3/Zlib | 1–2 se necessário |
| AVIF (escrita) | ravif | 0.13 | BSD-3-Clause | 6 |
| TIFF / EXR | tiff / exr | 0.11 / 1.74 | MIT / BSD-3-Clause | 1 / 2 |
| JPEG 2000 | jpeg2k (OpenJPEG) | 0.10 | MIT OR Apache-2.0 (OpenJPEG: BSD-2) | 2 |
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

## Modelos de IA (avaliação inicial, Fase 3)

Ver `ARCHITECTURE.md §11`. Cada modelo só entra em `models/manifest.json` depois de
confirmar a licença **dos pesos** (que pode diferir da licença do código).
