# Estado do projeto

**Última atualização:** 2026-11-10 · **Fase atual:** 11 — Álbuns · **Estado:** ✅ concluída
**Versão:** 0.11.0 · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

---

## Resumo

Cada projeto pode ter **álbuns fotográficos** (vista "Álbum"): formatos 20×20 a 30×40 cm e
personalizado, com largura, altura, orientação, páginas, DPI, sangria, zona segura,
dobra, ICC e laboratório. Presets de laboratório personalizados; sem preset nem
confirmação, as medidas ficam **por confirmar** (não há valores universais). Editor com
arrastar e largar, layouts de 1 a 6+ fotografias, full bleed, panorâmicas, alinhar,
distribuir, margens, espaçamento, recorte e zoom. **"Criar álbum automaticamente"** monta
uma narrativa pelos momentos. Alertas de DPI, zona segura e rostos na dobra. Exportação
pela fila, a partir dos originais: JPEG/TIFF por spread ou página, PDF de impressão e PDF
de prova.

## Funcional (verificado)

| Funcionalidade | Verificação |
|---|---|
| Formatos 20×20, 25×25, 30×30, 35×35, 20×30, 25×35, 30×40 e personalizado; orientação | Testes; interface (30×40 → 40 cm horizontal; editar → "Personalizado") |
| Páginas (pares), DPI, sangria, zona segura, dobra, cor/ICC, unidade de exportação, fundo | Testes; aplicação real (20 páginas → 10 spreads, 150 DPI) |
| Presets de laboratório personalizados; medidas "por confirmar" | Teste de fluxo (biblioteca); interface (aplicar preset → confirmado) |
| Arrastar e largar da tira para o spread ou para uma moldura | Aplicação real (xdotool) e teste de interface |
| Layouts de 1 a 6+ (incl. panorâmica, full bleed, grelhas justificadas), margens, espaçamento | Testes; aplicação real (F\|F aplicado) |
| Mover, redimensionar, recortar, zoom, alinhar, distribuir, teclado (setas, Delete) | Testes de lógica e interface; aplicação real (Shift+clique + Delete) |
| "Criar álbum automaticamente" (seleção, momentos, cronologia, classificação, orientação, pessoas, relevância) | Testes (ordem da narrativa, sem duplicados, sem rostos na dobra); aplicação real: 55 fotografias → Abertura → Cerimónia → Casal → Receção → Festa |
| Evitar baixa resolução: fotografias pequenas demais ficam de fora; as restantes nunca são ampliadas além do DPI do laboratório | Teste de regressão (erro encontrado na aplicação real e corrigido) |
| Alertas de DPI (erro < 50 %, aviso < 80 %), zona segura, rostos na dobra, bordas perto do corte, vazias, repetidas | Testes; aplicação real (46 DPI → erro e exportação bloqueada) |
| PDF de impressão | Aplicação real + `pypdf`: 10 páginas, MediaBox 606×306 mm, TrimBox 600×300 mm, 3579×1807 px (150 DPI exatos), ICCBased |
| PDF de prova | Aplicação real: 120 DPI, guias de corte/zona segura/dobra, marca "PROVA" |
| JPEG por spread / TIFF | Teste de fluxo (largura exata com sangria) e testes do crate |
| Persistência | Álbum reaberto igual depois de reiniciar a aplicação |

## Desempenho medido (Linux x86_64, 4 núcleos, release)

| Operação | Resultado |
|---|---|
| Automático com 55 fotografias | instantâneo (< 1 s na aplicação real) |
| PDF de impressão 30×30, 10 spreads, 150 DPI (fotografias ~0,8 MP) | 5,4 MB; concluído em segundos (não cronometrado com precisão) |

A 300 DPI e com fotografias de 24 MP a revelação dos originais domina (como na Fase 9).
Mac/Windows: **não medido**.

## Não funcional / limitações

- **Texto e grafismos** nas páginas, **capa** e lombada: não implementados.
- **CMYK / PDF/X:** não; o PDF é RGB com o perfil ICC escolhido (D-076).
- **Desfazer** no editor: não (a montagem automática pede confirmação antes de substituir).
- O automático usa a análise (relevância, grupos, rostos) quando existe; sem análise usa
  as estrelas e só deteta rajadas pelo tempo (< 3 s) — imagens repetidas com horas
  diferentes não são detetadas.
- Encomendas diretas a laboratórios: não (o utilizador envia os ficheiros).
- Mac/Windows não executados.

## Arquitetura

Decisões D-071 a D-077. Crate `momento-album` (`spec`, `model`, `layouts`, `auto`,
`preflight`, `compose`, `pdf`, `export`); `project-management::album` (catálogo, dados das
fotografias, exportação pela fila); tabela `album_lab` (biblioteca, migração 4); editor
`AlbumView.tsx` + `logic/album.ts`.

## Dependências novas

Nenhuma (o PDF é escrito diretamente; JPEG/TIFF/ICC pelo motor de exportação existente).

## Comandos exatos

```bash
pnpm dev
```

```bash
./scripts/check.sh
```

## Testes realizados (2026-11-10)

| Conjunto | Resultado |
|---|---|
| `cargo test --release --workspace` (com amostras RAW, modelos e rostos) | ✅ 248 passaram, 0 falharam, 1 ignorado (débito 24 MP, manual) |
| `pnpm -r test` (Vitest) | ✅ 72 (desktop) + 6 (localização) |
| `tsc --noEmit`, `cargo fmt --check`, `cargo clippy -D warnings` | ✅ limpos |
| Aplicação real (build release, Linux/Xvfb) | ✅ criar álbum, especificação, automático (2×), arrastar e largar, layout, Delete, alertas, PDF de impressão e de prova validados por `pypdf` |

Novos testes: `album` (8), `tests/album_flow.rs`, interface (+3), lógica (+4).

## Problemas encontrados no teste real (corrigidos)

1. O automático colocava uma fotografia pequena numa página inteira (84 DPI → erro): agora
   reduz a moldura ao tamanho que a fotografia aguenta; o filtro usa o DPI do laboratório.
2. Os atalhos globais (estrelas, rejeitar, setas) atuavam na seleção invisível da grelha
   enquanto se editava o álbum: desligados na vista Álbum (exceto ajuda e Revelar).
3. A fila mostrava "Abrir galeria" num álbum (botão que falharia): agora "Abrir PDF".

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Laboratório exige CMYK ou PDF/X | Médio | Exportar JPEG/TIFF por página com o ICC do laboratório; CMYK numa fase futura |
| Medidas erradas do laboratório | Alto | Medidas "por confirmar" até haver preset; PDF de prova com guias antes de encomendar |
| Álbuns muito grandes (80+ páginas a 300 DPI) | Médio | Exportação pela fila com pausa/cancelamento e cache de originais |

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
| 10 — Galerias | ✅ | 2026-11-10 |
| 11 — Álbuns | ✅ | 2026-11-10 |
| 12 — Otimização e distribuição | ⏸ próxima | — |

## Próximo passo

**Recomendado:** montar um álbum de um casamento teu com o preset do teu laboratório
(medidas reais de sangria, zona segura e dobra) e enviar o PDF de prova ao laboratório.

**Fase 12 — Otimização e distribuição:** GPU (wgpu), Apple Silicon, projetos grandes,
recuperação, testes de carga, instaladores assinados.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); fotografias de teste;
medidas do teu laboratório; nome definitivo.
