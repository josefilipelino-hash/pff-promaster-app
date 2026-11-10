# Estado do projeto

**Última atualização:** 2026-11-10 · **Etapa atual:** Auditoria final da versão 1.0 pessoal · **Estado:** ✅ concluída
**Produto:** PFF ProMaster · **Versão:** 1.0.0-personal · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11 — **nenhuma das duas testada ainda em hardware real** (ver `KNOWN_LIMITATIONS.md`)

---

## Resumo

Auditoria completa da implementação contra toda a especificação mestre e as Fases 0–16
(**`FINAL_AUDIT.md`**: matriz de 479 requisitos, três listas). Encontrados e corrigidos dois
problemas críticos (realces/sombras locais simulados; perfil ICC embutido ignorado na
edição e exportação), oito altos/médios (cache depois de restaurar um catálogo, ações de
privacidade prometidas e inexistentes, relatórios de falha, caminhos nos registos,
favoritas da galeria perdidas, galeria apagada antes da nova, rosto cortado no álbum,
erros da interface sem captura) e cinco menores. Documentação alinhada com o
comportamento real; limitações em **`KNOWN_LIMITATIONS.md`**. A aplicação passou a chamar-se
**PFF ProMaster**, com ícone próprio.

**Resultado da matriz:** COMPLETA 309 · PARCIAL 91 · NÃO IMPLEMENTADA 66 · MOCK 0 ·
outros/decisões 13 · bugs conhecidos por corrigir 10 (menores, documentados) · com teste
automático 314 / sem teste 165.

## Funcional (verificado nesta etapa)

| Correção | Verificação |
|---|---|
| Realces/Sombras locais agem no render | `local_adjustments_stay_inside_the_mask` |
| Perfil ICC embutido (Adobe RGB, P3, ProPhoto) na edição e exportação | `embedded_icc_profile_is_honoured`, `embedded_input_profiles_round_trip` |
| Privacidade: Limpar cache, Apagar dados de pessoas, Eliminar projeto (para o Lixo) | `privacy_and_cleanup_actions`, `clear_all_only_touches_the_cache` |
| Cache invalidada ao restaurar/importar um catálogo | `privacy_and_cleanup_actions` |
| Registos sem caminhos nem emails; relatórios de falha com o registo certo | `log_lines_never_carry_paths_or_emails`, `reports_are_written_without_personal_paths_and_rotated` |
| Galeria: favoritas do cliente mantidas ao regenerar; anterior só substituída no fim | `wedding_gallery_end_to_end` |
| Álbum: aviso de rosto cortado pelo enquadramento | `preflight_finds_the_problems` |
| "Protegida" para favoritas; pesquisas por papel com 2+ pessoas | Vitest |
| Erros da interface: mensagem em vez de janela em branco + registo | Vitest |
| Cópias virtuais: nomes únicos; no Lixo nunca levam o original | `history…virtual_copies`, `privacy_and_cleanup_actions` |
| Versão do motor na chave das edições em cache | código (`PIPELINE_VERSION`) |
| Nome e ícone PFF ProMaster | pacote gerado |

## Não funcional / limitações

Ver **`KNOWN_LIMITATIONS.md`** (completo) e as listas "Parcialmente funcional" e "Ainda não
implementado" de `FINAL_AUDIT.md`. As mais importantes:

- **macOS e Windows nunca executados** neste ciclo (tudo testado em Linux).
- LibRaw no mesmo processo (uma falha nativa fecha a aplicação; o catálogo sobrevive).
- Não-RAW de 16 bits/HDR revelados a 8 bits; sem soft proofing nem perfil do monitor.
- Sem rede: publicação direta de galerias não existe; nem idiomas além de pt-PT (en sem seletor).

## Arquitetura

Decisão D-109 (nome PFF ProMaster, identificador `pt.pffpromaster.desktop`, nomes internos
`momento-*` mantidos). Novos: `InputColor` (color-management), `cache::clear_all`,
`safety::delete_project`, `ProjectSession::clear_cache`, comandos `clear_cache`,
`delete_project`, `log_ui_error`, `ErrorBoundary`, separador Privacidade.

## Dependências novas

Nenhuma.

## Comandos exatos

```bash
MOMENTO_SAMPLES_DIR=<amostras RAW> MOMENTO_REF_DIR=<referências> MOMENTO_FACES_DIR=<rostos> cargo test --release --workspace
```

```bash
pnpm -r test
```

```bash
./scripts/build-release.sh
```

## Testes realizados (2026-11-10, auditoria final)

| Conjunto | Resultado |
|---|---|
| `cargo test --release --workspace` (RAW reais, modelos de referência, rostos, discos cheio/só de leitura) | ✅ **291 passaram, 0 falharam, 1 ignorado** (débito 24 MP, manual). Na primeira passagem `no_leaks_across_open_close_cycles` falhou por medir uma amostra isolada de RSS (oscila ~50 MB; patamar estável): o teste passou a comparar patamares e o ficheiro `adverse` voltou a passar 9/9 |
| Permissões (utilizador sem privilégios, `setpriv`) | ✅ |
| `pnpm -r test` (Vitest) | ✅ 97 (desktop) + 6 (localização) |
| `tsc --noEmit`, `cargo fmt --check`, `cargo clippy -D warnings` | ✅ limpos |
| Pacote de produção (Linux) reconstruído com o nome e ícone novos, instalado e executado | ✅ lançamento, registo anonimizado, abrir projeto real, Limpar cache, Eliminar projeto (Lixo) — `FINAL_AUDIT.md` §5b |

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Primeiro build/uso em macOS revelar problemas não vistos em Linux | Médio | Secção 2 de `RELEASE_CHECKLIST.md` antes de um trabalho real; CI macos-14 |
| Falha nativa da LibRaw num RAW malformado | Baixo | Catálogo íntegro e sessão recuperada; processo auxiliar fica para a Fase 17 |

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
| 16 — Versão instalável (1.0.0-personal) | ✅ | 2026-11-10 |
| Auditoria final 1.0 (PFF ProMaster) | ✅ | 2026-11-10 |
| 17 — Distribuição | ⏸ próxima | — |

## Próximo passo

**No Mac:** `./scripts/build-release.sh`, instalar o `.dmg` e correr a secção 2 de
`RELEASE_CHECKLIST.md` com um cartão real de teste antes do próximo trabalho.

**Fase 17 — Distribuição:** assinatura/notarização (`docs/SIGNING.md`), menu nativo,
GPU, atualizações automáticas.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); conta Apple Developer
(opcional); fotografias de teste; medidas do laboratório.
