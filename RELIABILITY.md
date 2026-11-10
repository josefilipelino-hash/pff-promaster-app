# Fiabilidade — o que acontece quando algo corre mal

**Fase 13 (2026-11-10).** Objetivo: nunca perder horas de trabalho e nunca apagar um
original sem o utilizador o pedir explicitamente. Cada comportamento abaixo está coberto
por um teste automático (`packages/project-management/tests/resilience.rs` e testes
unitários) — os testes de encerramento matam mesmo o processo a meio (SIGKILL).

## Princípios

1. **Gravação contínua.** Não há "Guardar": cada classificação, edição, momento ou álbum é
   uma transação SQLite confirmada no momento, com `synchronous = FULL` — está no disco
   quando a interface mostra a alteração. Um encerramento (falha, energia, processo
   terminado) perde no máximo a operação que estava a meio.
2. **Nada é apagado automaticamente.** Nem originais, nem referências, nem o catálogo
   danificado. A remoção é sempre um pedido explícito, confirmado duas vezes para o lixo.
3. **Escritas atómicas.** Miniaturas, previews, exportações e cópias da importação são
   escritas num temporário (`.<nome>.<pid>-<n>.tmp`), sincronizadas e renomeadas: nunca
   existe um ficheiro final meio escrito.
4. **Cada restauro guarda primeiro o estado atual.**

## Catálogo

| Mecanismo | Como funciona |
|---|---|
| Journaling | WAL do SQLite + `synchronous = FULL` + histórico de edições por fotografia |
| Checkpoints | A cada 5 minutos e no fecho (`wal_checkpoint(TRUNCATE)`): tudo passa para o ficheiro principal |
| Cópias de segurança (versões) | `backups/` do projeto: ao abrir (se a última tiver > 1 h), de forma programada enquanto está aberto (15/30/60/240 min ou desligada; por omissão 60) e no fecho (se a última tiver > 15 min). Cada cópia é **validada** (`integrity_check`) antes de ficar com o nome final |
| Retenção | Automáticas: as 10 mais recentes + uma por dia nos últimos 14 dias. Manuais, "antes do restauro" e "danificado" **nunca** são apagadas automaticamente |
| Validação | Em cada abertura: `quick_check` (15 000 fotografias: 68 ms). Depois de uma sessão interrompida: `integrity_check` completo + chaves estrangeiras (89 ms). Também a pedido |
| Recuperação | Catálogo danificado → é **guardado** em `backups/catalog-danificado-*.db` e substituído pela versão válida mais recente; a interface explica o que aconteceu e desde quando |
| Exportar / importar catálogo | Instantâneo consistente e validado para outro ficheiro/disco; importar valida (é um catálogo do Momento, íntegro, versão suportada) e guarda o atual antes de o substituir |
| Uso exclusivo | `session.lock` com bloqueio do sistema: um segundo processo/janela não abre o mesmo projeto ("já está aberto noutra janela"); o bloqueio é libertado pelo sistema mesmo que a aplicação morra |
| Deteção de sessão interrompida | O ficheiro de bloqueio diz "running" até ao fecho limpo; se a abertura o encontrar sem dono, a sessão anterior foi interrompida → validação completa, limpeza de temporários, registo e aviso |

## Originais

| Situação | Comportamento |
|---|---|
| Disco externo desligado | Fotografias marcadas **offline** (verificação a cada 30 s e a pedido). A referência fica intacta. Miniaturas e previews já feitas continuam a ser vistas; editar a 100 %/exportar dá "original indisponível" por fotografia, a fila continua. Miniaturas por fazer ficam pendentes (não "falhadas") |
| Disco ligado outra vez | **Reconexão automática** em ≤ 30 s: estado "ok", miniaturas pendentes geradas, aviso na interface |
| Ficheiro desaparecido (disco presente) | "**Em falta**". Relink: "Localizar original…" (um ficheiro; procura depois os outros em falta na mesma pasta) ou "Procurar numa pasta…" (nome + tamanho + conteúdo) |
| Relink | Só aceita um ficheiro com **o mesmo conteúdo** (BLAKE3); outro ficheiro com o mesmo nome é recusado |
| Remover do catálogo | Retira as fotografias (classificações, edições, análise) **sem tocar nos ficheiros** |
| Mover para o lixo | Lixo do sistema (recuperável): escolher a opção + confirmar "Compreendo…"; o núcleo recusa sem confirmação. Se o lixo recusar um ficheiro, essa fotografia fica no catálogo. A cópia de segurança da importação nunca é tocada |
| Localizar original | Mostra o ficheiro no Finder/Explorador |

## Importação

- **Checksum opcional** ("Verificar cada cópia"): relê cada ficheiro copiado e compara o
  BLAKE3 com o original. Ligado por omissão. (O hash do original é sempre calculado: serve
  para detetar duplicados.)
- **ORIGINAL → Trabalho + ORIGINAL → Backup** em simultâneo, ambas atómicas e verificadas;
  o caminho da cópia de segurança fica registado em cada fotografia.
- **Encerramento a meio:** os ficheiros por processar continuam pendentes e a importação é
  **retomada** na abertura seguinte com as mesmas opções. Uma cópia já completa com o mesmo
  conteúdo é reutilizada (sem duplicados `_1`); temporários de cópias interrompidas são
  removidos no trabalho e no disco de backup.

## Exportação

- Ficheiros escritos de forma atómica: depois de um encerramento só há ficheiros completos.
- A exportação fica registada no catálogo (pedido + próxima fotografia). Depois de um
  encerramento (ou fecho do projeto a meio), "Exportações interrompidas" permite **Retomar**
  (só as que faltavam) ou **Descartar**. Cancelar de propósito não deixa registo.
- Limite conhecido: se o encerramento acontecer exatamente entre gravar um ficheiro e
  registar o progresso (milissegundos), essa fotografia sai duas vezes (`_1`) na retoma.

## Crash logs

- Falhas internas (pânico) → `logs/crash-<data>.txt` com versão, sistema, mensagem e pilha
  de chamadas. Falhas em código nativo, energia ou processo terminado → relatório na
  abertura seguinte, com o fim do log anterior.
- **Sem fotografias nem dados sensíveis:** caminhos (`/Users/…`, `C:\…`, `~/…`, `\\servidor\…`)
  e emails são substituídos por `<caminho>` / `<email>` (testado). Nada é enviado; os 20
  mais recentes ficam no computador.

## Testes deliberados

| Teste | Como | Resultado verificado |
|---|---|---|
| Encerramento durante a importação | Processo filho importa 400 fotografias (cópia + backup + verificação) e é morto com SIGKILL após ≥ 10 | Sessão interrompida detetada, validação completa, importação retomada: 400 no catálogo, **exatamente 400** ficheiros no trabalho e no backup (sem `_1` nem temporários), todos com o hash certo |
| Encerramento durante a exportação | Processo filho exporta 160 fotografias e é morto após ≥ 5 ficheiros | Todos os ficheiros existentes abrem (nenhum meio escrito); exportação interrompida listada com o progresso; retomada exporta só as restantes; temporários limpos |
| Disco removido | Pasta do "disco" renomeada | 6 offline, referências intactas, miniatura servida, preview → "original indisponível", exportação falha por ficheiro sem parar; disco de volta → 6 reconectadas |
| Ficheiro desaparecido | Ficheiro movido para outra pasta | "Em falta"; relink pela pasta (confirmado pelo conteúdo); ficheiro com outro conteúdo recusado |
| Base de dados bloqueada | Outro processo segura um `BEGIN EXCLUSIVE` 1,5 s | A classificação espera (busy timeout 10 s) e é gravada; sem erro nem perda. Mesmo projeto aberto duas vezes → recusado |
| RAW corrompido | Cabeçalho CR2 válido + lixo | Fica de fora da importação com o motivo; as outras entram; sem pânico |
| Catálogo danificado | 64 KB de páginas sobrescritas | Detetado na abertura, danificado guardado, versão válida restaurada, catálogo íntegro |

Na aplicação real (Linux/Xvfb): ver `PROJECT_STATUS.md` (fase 13).

## Não coberto / limitações

- Discos de rede (SMB/NFS): o WAL do SQLite não é seguro neles; o catálogo tem de estar
  num disco local (interno ou externo). A aplicação não o impede ainda.
- O lixo do sistema em discos externos depende do sistema operativo (no Linux, alguns
  volumes não têm lixo → a fotografia fica no catálogo com o motivo).
- macOS e Windows: comportamentos testados em Linux; o código usa APIs portáveis
  (bloqueio de ficheiros da biblioteca padrão, lixo do sistema via `trash`), mas **não
  foram executados nesses sistemas**.
