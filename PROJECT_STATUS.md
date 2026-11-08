# Estado do projeto

**Última atualização:** 2026-11-08 · **Fase atual:** 2 — Formatos e RAW · **Estado:** ✅ concluída
**Versão:** 0.2.0 · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

---

## Resumo

O motor de importação passou a ser **multiformato**, com uma arquitetura de 7 adaptadores
de codecs extensível e RAW através da **LibRaw 0.22.3** compilada do código-fonte. Cada
ficheiro recebe um estado claro: importado, **suporte parcial** (com explicação),
**corrompido**, **codec em falta** ou não suportado. Validado com **26 RAW reais de 15
fabricantes** e **65 ficheiros de 25 formatos** pelo caminho completo da aplicação, e na
aplicação real (release).

A Fase 1 (projetos, importação, grelha, visualizador, classificação, persistência)
continua funcional e testada.

## Funcional (verificado)

| Funcionalidade | Verificação |
|---|---|
| Arquitetura de codecs: deteção pelo conteúdo, registo com prioridades e recurso, metadados e nível de suporte por adaptador, proteção contra pânicos | Testes unitários (22 em `image-codecs`) |
| RAW (LibRaw 0.22.3): CR2, CR3, CRW, NEF, ARW, RAF (Bayer e X-Trans), RW2, RWL, ORF, PEF, SRW, X3F (incl. Quattro), DCR, DNG (DJI, Leica, Pentax, smartphone) | 25/26 ficheiros reais (o 26.º é truncado → “corrompido”) |
| RAW: preview embutida primeiro (10–140 ms); preview de ecrã na mesma leitura que a miniatura; revelação total só a 100 % (0,3–9 s) | Relatórios em `docs/reports/` + aplicação real (NEF de 36 MP a 100 %) |
| RAW: câmara, objetiva, ISO, velocidade, abertura, focal, data, orientação, bits, cor do sensor, preview embutida, descodificador | Painel de informação na aplicação real |
| Previews já rodadas pela câmara (Nikon D70, Kodak DCS) normalizadas | Verificação visual + teste unitário |
| RAW com compressão não suportada pela LibRaw (DNG JPEG XL, NEF HE, GoPro) → suporte parcial com preview embutida | Lógica testada; sem amostra real destes casos |
| WebP, JPEG XL, JPEG 2000, PSD (composto, leitor próprio), BigTIFF, BMP, GIF, OpenEXR, HDR, ICO, PNM/PAM, TGA, JPEG CMYK | Ficheiros reais e de codificadores independentes |
| Estados: parcial (selo na grelha + explicação), corrompido, codec em falta, não suportado (relatório com legenda) | Testes + aplicação real |
| Perfil de cor: descrição ICC (v2/v4), espaço linear (HDR), cor do sensor (RAW) | Testes + painel |
| Cache: limites LRU (previews 10 GB, resolução total 5 GB), miniaturas permanentes | Teste unitário de LRU |
| Catálogo v3 com migração automática a partir da v2 | Teste de migração |
| HEIC/HEIF/AVIF no macOS (ImageIO) e Windows (WIC, com indicação das extensões da Store) | **Compilado** para cada plataforma; **não executado** |
| LibRaw compila para Windows (MinGW) e o executável liga | Compilação cruzada |

## Não funcional / não validado

- **HEIC/HEIF/AVIF**: adaptadores ImageIO e WIC não foram executados (sem macOS/Windows nesta
  sessão). No Linux aparecem corretamente como “codec em falta”.
- RAW sem amostra real: NRW, SRF/SR2, 3FR/FFF, IIQ, KDC, MRW, MEF/MOS, ERF, Apple ProRAW,
  lossy DNG, NEF High Efficiency. A LibRaw suporta a maioria, mas não foram testados.
- Gestão de cor no núcleo (LittleCMS): ainda não — previews RAW em sRGB pela LibRaw, perfis
  ICC dos outros formatos preservados para o WebView.
- IPTC/XMP; relocalização de originais; sincronização de hora entre câmaras (adiadas para a
  Fase 5 — momentos/câmaras).
- Isolamento da LibRaw num processo auxiliar (decisão D-017: não nesta fase).
- Mapeamento de tons para EXR/HDR; animação de GIF/WebP; PSD com compressão ZIP; PSD Lab.

## Arquitetura

Sem alterações à arquitetura principal. Decisões novas D-016 a D-023 em `ARCHITECTURE.md`:
camada C própria sobre a LibRaw; LibRaw no mesmo processo; contrato de codecs com
metadados/suporte; HEIC/AVIF via codecs do sistema; leitor PSD próprio; preview de ecrã a
partir da preview embutida; limites LRU; política de 7 dias também para crates Rust.

## Desempenho medido (Linux x86_64, 8 núcleos, ficheiros reais)

| Operação | Resultado |
|---|---|
| Importação de 65 ficheiros (≈ 700 MB, 26 RAW) | 1,3 s |
| Miniaturas (+ previews de ecrã dos RAW) de 51 ficheiros | 3,6 s |
| Abrir preview de um RAW já importado | 0–1 ms (já em cache) |
| Revelação total a 100 % | 0,2 s (6 MP) a 9 s (X-Trans 16 MP); 36–45 MP em 4–5 s |

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| HEIC/AVIF não validados em macOS/Windows | Alto (iPhone = HEIC) | Primeira verificação manual no Mac/PC; adaptadores já compilados |
| Segfault dentro da LibRaw com ficheiro extremamente malformado | Médio | Exceções isoladas; LibRaw testada por OSS-Fuzz; processo auxiliar se surgir caso real |
| Build Windows MSVC depende do runtime Visual C++ | Baixo | Avaliar `crt-static` ou incluir o redistribuível no instalador (Fase 9) |
| Formatos RAW sem amostra | Médio | Pedir amostras; teste `real_formats` pronto |
| Disco na compilação (LibRaw + 3 alvos ≈ 20 GB de `target/`) | Baixo | Limpar `target/` entre alvos |

## Dependências novas

LibRaw 0.22.3 (CDDL-1.0, submódulo), zlib (libz-sys 1.1.31), cc 1.10, jxl-oxide 0.12.6,
jpeg2k 0.10.1 + openjp2 0.6.1, image (features webp/bmp/gif/exr/hdr/ico/pnm/tga), objc2-*
0.3.2 (macOS), windows 0.62.2 (Windows). Ver `LICENSES.md`.

## Comandos exatos

```bash
git submodule update --init --recursive
```

```bash
pnpm install
```

```bash
pnpm dev
```

```bash
./scripts/check.sh
```

Validar com as tuas fotografias (nada é copiado para o repositório):

```bash
MOMENTO_SAMPLES_DIR=/caminho/para/fotos cargo test -p momento-project-management --release --test real_formats -- --nocapture
```

## Testes realizados (2026-11-08)

| Verificação | Resultado |
|---|---|
| `cargo test --workspace` | ✅ 86/86 (80 unitários + 6 de integração, 2 deles com ficheiros reais quando `MOMENTO_SAMPLES_DIR` está definido) |
| `pnpm -r test` | ✅ 36/36 |
| `pnpm -r typecheck`, `cargo fmt --check`, `cargo clippy -D warnings` | ✅ |
| 26 RAW reais (`real_samples`) | ✅ 25 completos + 1 corrompido identificado |
| 65 ficheiros reais (`real_formats`) | ✅ 46 completos, 5 parciais, 3 duplicados, 9 codec em falta, 2 corrompidos, 0 falhas |
| Aplicação real (release, Linux) | ✅ importação, relatório, grelha RAW, painel, 100 % |
| `cargo check --workspace --target x86_64-pc-windows-gnu` + executável | ✅ (19,9 MB, com LibRaw) |
| `cargo check -p momento-image-codecs --target aarch64-apple-darwin` | ✅ (ImageIO) |
| macOS / Windows reais | ⏳ não executado |

## Histórico de fases

| Fase | Estado | Data |
|---|---|---|
| 0 — Fundação técnica | ✅ | 2026-11-08 |
| 1 — Aplicação utilizável | ✅ | 2026-11-08 |
| 2 — Formatos e RAW | ✅ | 2026-11-08 |
| 3 — Seleção automática | ⏸ próxima | — |

## Próximo passo

**Recomendado antes da Fase 3:** uma verificação curta no teu Mac e/ou PC — `pnpm dev`,
importar uma pasta com RAW das tuas câmaras e fotografias HEIC do iPhone, e confirmar que
os HEIC aparecem (ImageIO/WIC). É o único ponto relevante desta fase sem validação real.

**Fase 3 — Seleção automática:** duplicados por semelhança (hash percetual), rajadas e
sequências, nitidez global, exposição/clipping, comparação lado a lado com zoom
sincronizado e recomendação explicada por grupo (métricas clássicas primeiro; modelos de
IA a seguir).

**Critério de conclusão da Fase 3:** num conjunto real de ≥ 500 fotografias, grupos de
rajadas/semelhantes formados e revistos visualmente, recomendação por grupo com explicação,
comparação lado a lado funcional, sem eliminar nada automaticamente.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); amostras RAW das tuas
câmaras e HEIC do iPhone; nome definitivo da aplicação.
