# Estado do projeto

**Última atualização:** 2026-11-10 · **Fase atual:** 13 — Fiabilidade · **Estado:** ✅ concluída
**Versão:** 0.13.0 · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

---

## Resumo

A aplicação passa a ser segura para trabalho real: não há "Guardar" — cada alteração é
gravada no disco quando acontece; o catálogo tem versões verificadas (automáticas e
manuais), é validado em cada abertura e recuperado sozinho se estiver danificado; uma
sessão interrompida é detetada e explicada; nada é apagado sem pedido explícito e
confirmado; discos desligados ficam offline e voltam sozinhos. Comportamento detalhado e
testes: **`RELIABILITY.md`**.

## Funcional (verificado)

| Pedido | Como | Verificação |
|---|---|---|
| Auto save / journaling | Transação por alteração, WAL + `synchronous = FULL`, histórico de edições | `kill -9` na aplicação real: alteração anterior mantida |
| Checkpoints | WAL a cada 5 min e no fecho | Teste de fecho limpo |
| Backups do catálogo, versões, programado | `backups/`: ao abrir, a cada 15/30/60/240 min (60 por omissão), no fecho, manual; validados; retenção 10 + 1/dia (14 dias) | Testes; aplicação real (versão manual criada) |
| Validação | `quick_check` em cada abertura (68 ms com 15 000), `integrity_check` depois de interrupção (89 ms) | Testes; aplicação real |
| Recuperação | Catálogo danificado guardado + versão válida restaurada; restauro manual com confirmação (atual guardado antes) | Teste de catálogo danificado; interface |
| Recuperação de sessão | Bloqueio "running" → validação completa, temporários limpos, importação retomada, exportações retomáveis, aviso, crash log | SIGKILL em importação e exportação (testes); `kill -9` na aplicação real |
| Exportar / importar catálogo | Instantâneo validado; importação valida e guarda o atual | Interface |
| Nunca eliminar automaticamente | Nenhum caminho de código apaga originais sem pedido | Revisão + testes |
| Delete destrutivo com confirmação | Escolha obrigatória; lixo com segunda confirmação; núcleo recusa sem `confirmed` | Interface (Enter não apaga) + teste do núcleo |
| Remover só do catálogo / mover para o lixo / localizar / relink | Lixo do sistema (`trash`); relink por ficheiro ou pasta confirmado pelo conteúdo | Testes; aplicação real |
| Disco indisponível → offline, sem perder a referência; reconexão automática | Verificação a cada 30 s | Aplicação real: 300 offline em 30 s, reconectadas em 22 s |
| Importação: checksum opcional, ORIGINAL → Trabalho + Backup, integridade | Cópias atómicas e verificadas (BLAKE3), caminho do backup registado | SIGKILL a meio: 400/400 em ambos os destinos, hashes certos |
| Crash logs sem dados sensíveis | `logs/crash-*.txt`, caminhos e emails removidos | Teste + aplicação real |
| Testes deliberados | Importação, exportação, disco removido, ficheiro desaparecido, BD bloqueada, RAW corrompido (+ catálogo danificado, 2.ª instância) | `tests/resilience.rs` (6), estáveis em 3 execuções seguidas |

## Problemas encontrados pelos testes (corrigidos)

1. **Temporário órfão no disco de backup** depois de um SIGKILL a meio de uma cópia: a
   limpeza só cobria a cache e a pasta dos originais → a retoma da importação limpa também
   o disco de backup.
2. Mensagem "projeto em uso" repetida (detalhe = mensagem) → corrigida.
3. Contagem "Analisar N fotografias" desatualizada depois de remover do catálogo → recarregada.
4. Teste da galeria protegida instável (~1 %): o base64 do conteúdo cifrado podia conter
   "Ana" por acaso → a procura de fugas ignora os blocos cifrados (15/15 execuções).

## Não funcional / limitações

- macOS e Windows **não executados** (APIs portáveis: bloqueio de ficheiros da biblioteca
  padrão, lixo do sistema via `trash`).
- Catálogo em disco de rede (SMB/NFS) não é detetado nem impedido (o WAL não é seguro aí).
- Exportação retomada: a fotografia a ser gravada no instante exato da falha pode sair
  duas vezes (`_1`).
- Falhas em código nativo (LibRaw, etc.) não geram relatório no momento; são detetadas e
  registadas na abertura seguinte.

## Arquitetura

Decisões D-086 a D-092. Novos: `database::safety` (validação, checkpoint, disponibilidade,
remover, relink, registo), migração v9 (`backup_path`, `safety_log`), `project-management::
{safety, availability, originals}`, registo de exportações, `core-types::redact`,
`src-tauri/crash.rs`, `SafetyDialogs.tsx`.

## Dependências novas

`trash` 5.3 (MIT; lixo do sistema em macOS, Windows e Linux). Ver `LICENSES.md`.

## Comandos exatos

```bash
pnpm dev
```

```bash
./scripts/check.sh
```

Testes de falhas (matam processos a meio):

```bash
cargo test --release -p momento-project-management --test resilience
```

## Testes realizados (2026-11-10)

| Conjunto | Resultado |
|---|---|
| `cargo test --release --workspace` (com amostras RAW, modelos e rostos) | ✅ 267 passaram, 0 falharam, 1 ignorado (débito 24 MP, manual) |
| `pnpm -r test` (Vitest) | ✅ 78 (desktop) + 6 (localização) |
| `tsc --noEmit`, `cargo fmt --check`, `cargo clippy -D warnings` | ✅ limpos (lint novo da MSRV 1.89 aplicado) |
| Aplicação real (release, Linux/Xvfb) | ✅ disco desligado/ligado, Delete, versões, `kill -9` + reabertura, crash log, 2.ª instância |

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Lixo indisponível em alguns volumes externos | Baixo | A fotografia fica no catálogo com o motivo; remover só do catálogo continua possível |
| Catálogo num disco de rede | Alto | Documentado; aviso na interface numa fase futura |
| Disco de backup cheio durante a importação | Médio | Esperado (não testado): a cópia falha antes do `rename`, a fotografia fica com erro no relatório e não entra pela metade |

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
| 14 — Distribuição | ⏸ próxima | — |

## Próximo passo

**Recomendado:** num Mac, importar um cartão com "Copiar também para um disco de
segurança", desligar e ligar o disco externo com o projeto aberto, e experimentar
"Segurança e cópias → Versões".

**Fase 14 — Distribuição:** GPU (wgpu), Apple Silicon, instaladores assinados.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); fotografias de teste;
medidas do laboratório; nome definitivo.
