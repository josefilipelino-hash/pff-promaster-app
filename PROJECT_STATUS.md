# Estado do projeto

**Última atualização:** 2026-11-09 · **Fase atual:** 9 — Exportação · **Estado:** ✅ concluída
**Versão:** 0.9.0 · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

---

## Resumo

Motor de exportação profissional: cada fotografia é revelada **a partir do original** (RAW
em resolução total do sensor) com a receita de edição, uma única vez, e dela saem todas as
receitas pedidas (multi-exportação). 11 formatos com ficheiro real (JPEG, JPEG XL, JPEG
2000, PNG, TIFF, BigTIFF, WebP, AVIF, OpenEXR, PSD, PDF), 8–16 bits e vírgula flutuante,
6 espaços de cor + ICC personalizado/laboratório, dimensões, PPI, nitidez de saída,
metadados com remoção de GPS, copyright, marca de água, nomes e subpastas, 10 receitas
incluídas e o plano "Exportar casamento", e uma fila com progresso, tamanho, espaço e tempo
restante. HEIF/HEIC e DNG não estão disponíveis (explicado na interface).

## Funcional (verificado)

| Funcionalidade | Verificação |
|---|---|
| Exportação a partir do original (RAW em resolução total, não preview) com a edição | Teste com RAW Canon 7D: TIFF 16 bits 5184×3456 com > 2000 níveis distintos; edição +1 EV presente no ficheiro; render de exportação = ecrã (±1) |
| JPEG, PNG 8/16, TIFF e BigTIFF 8/16/32f, WebP, AVIF 8/10/12, JPEG XL 8/16 sem perdas, JPEG 2000, OpenEXR 16f/32f, PSD 8/16, PDF | Testes com descodificadores independentes + `scripts/validate-exports.py`: **25/25 amostras válidas** em libjxl, libavif, OpenJPEG (C), libwebp, Pillow, tifffile, psd-tools, pypdf e OpenEXR |
| sRGB, Display P3, Adobe RGB, ProPhoto, Rec. 709, Rec. 2020, ICC personalizado/laboratório | Conversão = LittleCMS com o perfil embutido; P3 lido de volta pelo perfil dá as cores originais (fora de sRGB); ICC certo em cada ficheiro (verificado com Pillow/ImageCms) |
| Dimensões (lado longo/curto, largura, altura, caber, preencher, megapíxeis; px/cm/in), PPI, não ampliar | Testes; aplicação real: Galeria 2560 px, Instagram 1080×1350 exatos, PPI 300/72 gravados |
| Nitidez de saída (ecrã, mate, brilhante) e marca de água (texto/imagem, 9 posições) | Testes; aplicação real: "© Estúdio Lino" no canto inferior direito, acentos corretos |
| Metadados (tudo/copyright/nada), remover GPS, copyright, autor, contacto | EXIF lido por kamadak-exif e Pillow; GPS ausente; XMP com o texto Unicode completo |
| Nome, prefixo, sufixo, sequência, subpastas, conflitos (nunca sobrescreve originais) | Testes (inclui nomes reservados do Windows); segunda exportação → `-2` |
| 10 receitas incluídas, plano "Exportar casamento", receitas/planos do utilizador | Testes; aplicação real: 15 ficheiros em 5 pastas numa operação |
| Fila: progresso, restantes, tamanho estimado, espaço livre, tempo restante, erros, pausa/retoma/cancelar | Testes (pausada não avança; cancelar; erro claro) + aplicação real |
| Originais intactos | Hash igual antes/depois em todos os testes de fluxo |

## Desempenho medido (Linux x86_64, 4 núcleos, release)

| Operação | Resultado |
|---|---|
| RAW 18 MP (Canon 7D) → TIFF 16 bits + JPEG q95 em resolução total | ≈ 9 s (descodificar o RAW + revelar + 2 ficheiros) |
| Plano "Exportar casamento" (3 fotografias, 15 ficheiros, 116 MB) na aplicação real | ≈ 15 s |
| Reamostragem 45 MP → 2048 px (Lanczos 3, luz linear) | 0,13 s |
| Por ficheiro de 1,8 MP: JPEG 0,06 s · PNG 16 0,9 s (máxima) · TIFF 16 ZIP 0,4 s · WebP 0,2–0,9 s · JPEG XL 0,07 s · JP2 0,3–1 s · EXR 0,09 s · PSD 0,06 s | medido em `examples/export_samples` |
| **AVIF** | ≈ 2,2 s/MP (rav1e sem assembly): ≈ 50 s por fotografia de 24 MP — lento |
| Estimativa de tamanho antes de exportar | RAW real: 83 MB estimados vs. 96 MB reais; a fila corrige com os ficheiros reais |

Mac/Windows: **não medido**.

## Não funcional / limitações

- **HEIF/HEIC:** indisponível (codificador HEVC: x265 GPL e patentes). No macOS o ImageIO
  codifica HEIC — fica para quando houver testes num Mac. Use AVIF.
- **DNG:** indisponível (um DNG útil exige os dados do sensor por revelar e as matrizes de
  cor da câmara). A receita "Arquivo DNG" aparece desativada com a explicação.
- **JPEG XL:** só sem perdas. **JPEG 2000:** sem perdas só a 8 bits (a 12/16 bits o
  OpenJPEG de referência não lia exatamente o que o porte em Rust escreve — encontrado na
  validação externa); com perdas a 12/16 bits funciona. Sem EXIF/XMP no JP2.
- **AVIF:** lento e só sRGB, Display P3, Rec. 709 e Rec. 2020 (sem ICC no contentor).
- **Fora da gama:** corte por componente (sem mapeamento de gama perceptual); sem soft
  proofing nem simulação de papel.
- **TIFF/EXIF:** campos ASCII sem acentos ("(c) 2026 Estudio"); o texto completo está no XMP.
- **PSD:** só a imagem achatada (sem camadas). **PDF:** uma fotografia por página.
- **Mostrar pasta** usa o gestor de ficheiros do sistema (`open`/`explorer`/`xdg-open`).
- Exportação só em CPU (sem GPU). macOS/Windows não executados (Windows: compilação cruzada).

## Arquitetura

Decisões D-057 a D-063. Crates: `momento-color-management` (LittleCMS) e
`momento-export-engine` (receitas, cor, dimensões, nitidez, marca de água, codificadores,
metadados, nomes, presets); `develop::render_linear`; `project-management::{export,
export_store}` (fila e biblioteca); biblioteca global v3 (`export_preset`).

## Dependências novas

lcms2 (MIT), jpeg-encoder (MIT/Apache + IJG), png, tiff, exr (diretas), webp + libwebp
(BSD-3), rav1e (BSD-2), avif-serialize (BSD-3), zune-jpegxl (MIT/Apache/Zlib), openjp2
(BSD-2), fs4, ab_glyph; fonte Inter (OFL-1.1). Ver `LICENSES.md`.

## Comandos exatos

```bash
pnpm dev
```

```bash
./scripts/check.sh
```

Validar os formatos com descodificadores independentes (numa fotografia tua):

```bash
cargo run --release -p momento-export-engine --example export_samples -- foto.jpg amostras/
```

```bash
python3 scripts/validate-exports.py amostras/
```

## Testes realizados (2026-11-09)

| Conjunto | Resultado |
|---|---|
| `cargo test --release --workspace` (com amostras RAW, modelos e rostos) | ✅ 232 passaram, 0 falharam, 1 ignorado (débito 24 MP, manual) |
| `pnpm -r test` (Vitest) | ✅ 63 (desktop) + 6 (localização) |
| `tsc --noEmit`, `cargo fmt --check`, `cargo clippy -D warnings` | ✅ limpos |
| `cargo check --target x86_64-pc-windows-gnu -p momento-desktop` | ✅ (inclui LittleCMS e libwebp em C) |
| `scripts/validate-exports.py` (descodificadores independentes) | ✅ 25/25 amostras válidas |
| Aplicação real (build release, Linux/Xvfb) | ✅ diálogo, plano "Exportar casamento" (15 ficheiros em 5 pastas, RAW + JPEG), marca de água, copyright, seletor de pasta nativo, fila até "Concluída" |

Novos testes: `color-management` (4), `export-engine` (22, dos quais 13 de qualidade),
`develop::tests` (+1), `export_store`, `database` (+1), `tests/export_flow.rs` (2), interface (+3).

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Codificadores sem testes em macOS/Windows | Médio | Bibliotecas multiplataforma; compilação cruzada para Windows; CI a configurar |
| Exportações grandes (milhares de RAW) lentas em CPU | Médio | Uma revelação por fotografia para todas as receitas; GPU na fase de otimização |
| Laboratórios com perfis ICC exóticos (LUT, CMYK) | Baixo | Só RGB aceite, com mensagem clara; LittleCMS para LUT |

## Histórico de fases

| Fase | Estado | Data |
|---|---|---|
| 0 — Fundação técnica | ✅ | 2026-11-08 |
| 1 — Aplicação utilizável | ✅ | 2026-11-08 |
| 2 — Formatos e RAW | ✅ | 2026-11-08 |
| 3 — Seleção automática | ✅ | 2026-11-09 |
| 4 — Organização do casamento | ✅ | 2026-11-09 |
| 5 — Motor de edição | ✅ | 2026-11-09 |
| 6 — Edição automática | ✅ | 2026-11-09 |
| 7 — Presets e consistência visual | ✅ | 2026-11-09 |
| 8 — Edição localizada, retoque e retrato | ✅ | 2026-11-09 |
| 9 — Exportação | ✅ | 2026-11-09 |
| 10 — Galerias | ⏸ próxima | — |

## Próximo passo

**Recomendado:** num casamento teu, exportar com "Exportar casamento" para um disco externo
e abrir os ficheiros no visualizador habitual e no laboratório; enviar-me o perfil ICC do
laboratório se quiseres verificar a conversão. Num Mac, confirmar se queres HEIC via
ImageIO (posso implementá-lo, mas só fica validado depois de testado no Mac).

**Fase 10 — Galerias:** galeria estática com temas, slideshow, downloads, favoritas do
cliente, palavra-passe e publicação opcional.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); fotografias de teste;
nome definitivo.
