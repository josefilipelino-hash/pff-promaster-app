# Estado do projeto

**Última atualização:** 2026-11-10 · **Fase atual:** 15 — Estabilização · **Estado:** ✅ concluída
**Versão:** 0.15.0 · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

---

## Resumo

Fase sem funcionalidades novas, dedicada a tornar a aplicação utilizável no próximo
trabalho. Um casamento completo corre de ponta a ponta como teste automático (cartão →
álbum, com reinício a meio); as condições adversas foram testadas com discos reais; os 18
módulos foram auditados. Foram encontrados e corrigidos **2 erros críticos** (fotografias
de um segundo cartão ignoradas; "Substituir" sobre os originais no Windows/macOS) e **13
altos** (edições de uma fotografia gravadas noutra, máscaras copiadas na sincronização,
nomes de pessoas apagados, catálogo trocado por uma cópia antiga, molduras vazias no
álbum, exportações e importações presas). Detalhe: **`docs/reports/fase15-auditoria.md`**.
Antes de cada versão: **`RELEASE_CHECKLIST.md`**.

## Funcional (verificado)

| Pedido | Verificação |
|---|---|
| Auditoria módulo a módulo (18) | 4 revisões independentes + confirmação no código; todos os achados críticos/altos corrigidos com teste |
| Testes end-to-end | `wedding_e2e`: casamento completo (51 ficheiros, 6 RAW reais de marcas diferentes) em ≈ 3,5 min |
| Workflow cartão → … → álbum | Idem, com cartão intacto byte a byte, cópias de trabalho e segurança verificadas, exportação de 44 favoritas × 5 receitas, galeria, PDF de impressão |
| Projeto grande | 15 000 fotografias (Fases 12 e 14): abrir 247–291 ms, scroll 59–97 FPS |
| Memory leaks | 25 ciclos abrir/fechar com previews, renders e exportações: threads, descritores e memória estáveis |
| Restart | Classificações, edições, momentos e presets idênticos depois de reabrir; catálogo válido |
| Recovery | Falhas forçadas (Fase 13) + catálogo de versão mais recente recusado sem troca |
| Ficheiros corrompidos | RAW com lixo e JPEG truncado: recusado/marcado com motivo, sem pânico, fora da análise |
| Caminhos Unicode | Projeto, pastas, cartão, ficheiros e exportação com acentos, "&", "—", emoji |
| Discos externos | Disco de segurança, originais noutro disco, disco desligado/religado (Fase 13), cartão só de leitura |
| Nomes longos | Ficheiro de 232 bytes importado, copiado e exportado |
| Falta de espaço | Exportação, importação, disco de segurança e catálogo — sem ficheiros parciais nem corrupção |
| Permissões | Como utilizador sem privilégios: ficheiro/pasta ilegíveis no relatório; destino sem escrita falha com o motivo |

## Não funcional / limitações

- macOS e Windows **não executados** (as correções de caminhos e maiúsculas foram testadas
  com os equivalentes em Linux).
- Discos de rede não testados; cartões com falhas físicas não testados.
- Limitações menores documentadas na auditoria (ex.: nomes > 255 bytes falham ficheiro a
  ficheiro com o erro do sistema; verificação de cópia pode ler da cache do sistema).

## Arquitetura

Sem mudanças de arquitetura. Decisões D-099 a D-103 (duplicados por conteúdo, recuperação
só perante dano real, destino testado antes da exportação, proteção do editor contra
respostas de outra fotografia, testes com discos reais).

## Dependências novas

Nenhuma.

## Comandos exatos

```bash
pnpm dev
```

```bash
cargo test --release -p momento-project-management --test wedding_e2e -- --nocapture
```

Condições adversas e lista completa: `RELEASE_CHECKLIST.md`.

## Testes realizados (2026-11-10)

| Conjunto | Resultado |
|---|---|
| `cargo test --release --workspace` (RAW reais, modelos, rostos, discos cheio/só de leitura) | ✅ 285 passaram, 0 falharam, 1 ignorado (débito 24 MP, manual) |
| Permissões como utilizador sem privilégios (`setpriv`) | ✅ |
| `pnpm -r test` (Vitest) | ✅ 93 (desktop) + 6 (localização) |
| `tsc --noEmit`, `cargo fmt --check`, `cargo clippy` | ✅ limpos |
| Aplicação real (release, Linux/Xvfb) | ✅ importação de pasta Unicode com emoji pelo diálogo do sistema, duplicado reconhecido; cartão reutilizado → erros explicados e "6 em falta" (encontrou I11/X11, corrigidos) |

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Comportamento diferente em macOS/Windows (caminhos, maiúsculas, permissões) | Alto | Correções feitas para esses sistemas; `RELEASE_CHECKLIST.md` secção 2 num computador de cada sistema antes do trabalho |
| Reimportar um cartão já importado lê todos os ficheiros (decisão por conteúdo) | Baixo | Mais lento, nunca perde fotografias |

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
| 12 — Desempenho | ✅ | 2026-11-10 |
| 13 — Fiabilidade | ✅ | 2026-11-10 |
| 14 — UX/UI profissional | ✅ | 2026-11-10 |
| 15 — Estabilização | ✅ | 2026-11-10 |
| 16 — Distribuição | ⏸ próxima | — |

## Próximo passo

**Antes do próximo trabalho:** executar a secção 2 de `RELEASE_CHECKLIST.md` num Mac (ou
PC) com um cartão real de teste — especialmente importar dois cartões com os mesmos nomes,
exportar com "Substituir" para a pasta dos originais e desligar o disco a meio.

**Fase 16 — Distribuição:** CI em macOS/Windows, instaladores assinados, menu nativo,
GPU, Apple Silicon.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); fotografias de teste;
medidas do laboratório; nome definitivo.
