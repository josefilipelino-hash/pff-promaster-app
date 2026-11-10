# Fase 15 — Auditoria de estabilização (módulo a módulo)

**Data:** 2026-11-10 · **Versão:** 0.15.0

Objetivo: tornar a aplicação utilizável num trabalho real. Critério de fecho: nenhum erro
conhecido que possa **perder fotografias, perder edições, corromper o catálogo, produzir
uma exportação errada ou bloquear a aplicação com frequência**.

## Método

1. **Casamento completo** como teste de ponta a ponta (`tests/wedding_e2e.rs`): cartão com
   duas câmaras, 6 RAW reais de marcas diferentes, rajada, cópia repetida, ficheiros
   corrompidos, nomes Unicode e de 232 bytes → importação com cópia verificada + disco de
   segurança → miniaturas → análise → seleção → momentos → edição/desfazer/refazer → auto
   edit → preset → **reinício** → exportação (plano de 5 receitas) → galeria → álbum PDF.
2. **Condições adversas** com sistemas de ficheiros reais (`tests/adverse.rs`): disco cheio
   (exportação, importação, disco de segurança, catálogo), volumes só de leitura,
   permissões negadas (como utilizador sem privilégios), fugas de recursos.
3. **Revisão de código** por módulo (4 revisões independentes, só leitura), com cada
   achado **confirmado no código** antes de ser corrigido e, sempre que possível, um teste
   que **falha sem a correção**.
4. Testes já existentes: falhas deliberadas da Fase 13 (`resilience.rs`), 15 000
   fotografias (Fase 12), formatos reais.

## Resultado por módulo

Legenda: ✅ corrigido e testado · ☑️ corrigido (sem teste automático) · 📝 conhecido, não
crítico, documentado.

### 1. Projeto
| # | Gravidade | Problema | Estado |
|---|---|---|---|
| P1 | Média | Fechar o projeto durante uma importação deixava a análise automática a arrancar depois do fecho e a escrever no catálogo | ✅ sinal "a fechar"; análise parada depois da importação |
| P2 | Baixa | Clique em "Analisar" e retoma automática podiam arrancar duas análises | ☑️ início serializado |

### 2. Importação
| # | Gravidade | Problema | Estado |
|---|---|---|---|
| I1 | **Crítica** | "Já importada" decidida pelo **caminho**: um 2.º cartão (ou o mesmo formatado) montado no mesmo sítio, com IMG_0001… repetidos, era ignorado — fotografias perdidas depois de formatar | ✅ decide-se pelo conteúdo (BLAKE3); teste `second_card_at_same_path…` |
| I2 | Alta | Erro ao gravar no catálogo (disco cheio, ocupado) deixava os produtores bloqueados — importação e **fecho do projeto** presos | ✅ produtores param, canal fecha |
| I3 | Alta | Ficheiros lidos inteiros antes de detetar o formato: vídeos de vários GB no cartão esgotavam a memória | ✅ cabeçalho primeiro; limite de 2 GB por fotografia |
| I4 | Média | Pastas ilegíveis e caminhos inexistentes ignorados em silêncio | ✅ aparecem no relatório com o erro; testes (inclusive sem privilégios) |
| I5 | Média | Nomes que só diferem em maiúsculas podiam substituir uma cópia (macOS/Windows) | ✅ reserva sem distinção de maiúsculas + nunca substituir no `rename` |
| I6 | Média | Ao detetar conteúdo duplicado, a importação apagava "a sua cópia" — que podia ser o ficheiro da fotografia já catalogada | ✅ nunca apaga o ficheiro de outra fotografia |
| I7 | Baixa | Cópia falhada bloqueava um ficheiro idêntico noutra pasta do cartão | ✅ hash libertado |
| I8 | Baixa | Modelo de câmara ".." (EXIF corrompido) tirava a cópia da pasta dos originais | ✅ teste |
| I9 | Baixa | Disco de segurança igual à pasta dos originais | ✅ recusado |
| I11 | **Alta** | (Encontrado na **aplicação real**.) No modo "manter no local", um cartão reutilizado com os mesmos caminhos e OUTRAS fotografias aparecia como "Já existiam no projeto" — as novas ficavam de fora sem aviso | ✅ erro explicado por fotografia; teste |
| I10 | 📝 | A verificação relê a cópia logo a seguir (pode vir da cache do sistema) — deteta erros de escrita da aplicação, não do suporte | Documentado |

### 3. RAW
Sem defeitos críticos. Orientação (LibRaw sem rotação + EXIF uma vez), pré-visualização
embutida já rodada e dimensões 5–8 verificadas pela revisão; 6 RAW reais no casamento.
| R1 | 📝 | RAW sem dados do sensor usa a pré-visualização embutida (marcado "parcial" na grelha) | Documentado |

### 4. Library · 5. Culling
| # | Gravidade | Problema | Estado |
|---|---|---|---|
| C1 | Média | No editor, as teclas classificavam a fotografia **selecionada** mesmo quando o editor mostrava outra (seleção escondida pelo filtro) | ✅ teste |
| C2 | Média | Em Pessoas/Sequências/Consistência, P/X/estrelas/Delete agiam sobre a seleção invisível da grelha | ✅ teste |
| C3 | Média-baixa | Clique direito/arrastar numa seleção múltipla encolhia-a para uma (o menu "Rejeitar" só rejeitava uma) | ✅ teste |
| C4 | 📝 | Falha ao gravar uma classificação durante cliques muito rápidos pode repor o valor anterior no ecrã | Documentado (raro; o catálogo fica certo) |
| C5 | 📝 | Comparação com uma fotografia removida a meio: painel destacado ≠ classificado | Documentado |

### 6. Duplicados / seleção
| # | Gravidade | Problema | Estado |
|---|---|---|---|
| D1 | Média | "Aceitar sugestões" rejeitava fotografias com estrelas/cor (ex.: a 1.ª dança escura com 4★), que depois iam para "Remover…" | ✅ só rejeita o que não foi avaliado; teste |
| D2 | Média | Fotografia ilegível ficava "por analisar" para sempre ("Analisar 1") | ✅ conta como falhada; casamento E2E |
| D3 | 📝 | "Aceitar grupo" com o identificador de um grupo recalculado entre cliques | Documentado (janela de milissegundos) |

### 7. Pessoas
| # | Gravidade | Problema | Estado |
|---|---|---|---|
| F1 | Alta | Reanalisar sem os modelos (ou com o rosto noutra posição) **apagava** nomes confirmados e "não é esta pessoa" | ✅ decisões nunca apagadas; teste |
| F2 | Média | Ocultar uma pessoa sem nome perdia-se ao reagrupar | ☑️ ocultar confirma os rostos |
| F3 | Baixa-média | Escolhas em Pessoas ficavam em identificadores antigos depois de um reagrupamento — "Juntar" juntava pessoas erradas | ☑️ escolhas limpas ao recarregar |

### 8. Momentos
Sem defeitos. Atribuições do utilizador sobrevivem à classificação automática; desvios de
relógio guardados à parte, sem tocar no EXIF. Testado no casamento (25 momentos, atribuição
e contagens depois do reinício).

### 9. Develop · 13. Máscaras
| # | Gravidade | Problema | Estado |
|---|---|---|---|
| E1 | **Alta** | Máscara IA, retrato, conta-gotas, desfazer… que terminassem depois de mudar de fotografia escreviam os ajustes de uma fotografia **noutra** | ✅ respostas de outra fotografia ignoradas; teste que falha no código antigo |
| E2 | **Alta** | Auto no editor + mudar de fotografia: o próximo ajuste gravava **toda** a edição da anterior na atual | ✅ idem; teste |
| E3 | Média-alta | O último ajuste podia não ser gravado (rato largado fora do controlo) | ✅ grava ao perder a captura e ao sair da fotografia; teste |
| E4 | Baixa-média | Gravações enviadas em paralelo podiam chegar pela ordem inversa | ☑️ em série |
| E5 | 📝 | Um bloco de ajustes que não se consiga ler (ficheiro editado à mão) volta aos valores de origem | Documentado |

### 10. Auto Edit
| A1 | Média | A edição automática em lote revertia em silêncio edições feitas pelo utilizador nessa fotografia durante a análise | ✅ não grava se a receita mudou (conta como falhada, com motivo) |

### 11. Denoise
| N1 | Média | Um pânico no AI Denoise deixava a fotografia marcada "em curso" para sempre — preview e exportação dessa fotografia presas | ☑️ marca limpa sempre; espera com limite |

### 12. Presets · Sincronização
| # | Gravidade | Problema | Estado |
|---|---|---|---|
| S1 | **Alta** | Sincronizar o aspeto copiava as **máscaras e regiões de retrato** da referência para todas as fotografias (e apagava as delas) | ✅ teste |
| S2 | Média | Preset a 50 %/150 % interpolava a matiz da tonalização (laranja → verde/magenta) | ✅ teste |

### 14. Export
| # | Gravidade | Problema | Estado |
|---|---|---|---|
| X1 | **Crítica** | "Substituir" podia escrever por cima dos **originais** no Windows (`\\?\`) e por ligações simbólicas no macOS (`/var`) | ✅ comparação na mesma forma canónica; teste com ligação simbólica |
| X2 | **Alta** | Destino só de leitura / sem permissões parecia "sem espaço": exportação em pausa **para sempre** | ✅ falha já com o motivo; testes reais |
| X3 | Alta | Retomar uma exportação recomeçava {seq} em 0001 (com "Substituir", ficheiros entregues eram sobrescritos) | ☑️ numeração continua |
| X4 | Alta | Fechar o projeto a meio de uma fotografia perdia as receitas que faltavam dessa fotografia na retoma | ☑️ registo só avança com a fotografia completa |
| X5 | Média | Edição com LUT/máscara em falta (projeto copiado sem a pasta) saía com outras cores, sem aviso | ✅ fotografia falha com o motivo; teste |
| X6 | Média | Marca de água maior do que a fotografia (retratos) era omitida em silêncio | ✅ reduzida para caber; teste |
| X7 | Média-baixa | Trabalho acrescentado no instante em que a fila terminava ficava "Na fila" para sempre | ☑️ decisão sob o mesmo cadeado |
| X8 | Baixa-média | Temporário parcial deixado na pasta de entrega com o disco cheio | ☑️ removido em qualquer erro |
| X9 | Baixa | Exportação sem nenhum ficheiro escrito dizia "Concluída" | ✅ "Falhou" |
| X11 | **Alta** | (Consequência de I11.) Original substituído por outra fotografia no mesmo caminho: a exportação aplicava as edições da antiga à nova | ✅ verificação marca "em falta" (tamanho) e a exportação confirma o conteúdo (BLAKE3) antes de exportar; teste |
| X12 | Alta | **Regressão introduzida por X7** e apanhada pelo teste de fugas (25 ciclos): fechar o projeto esperava pelo trabalhador da fila com o cadeado que este precisava para terminar — bloqueio no fecho | ✅ cadeado largado antes de esperar; teste repetido 8× sem falhas |
| X10 | 📝 | Nomes > 255 bytes / caminhos > 260 carateres (Windows) falham ficheiro a ficheiro com o erro do sistema | Documentado |

### 15. Gallery
| G1 | Média | Fotografia de destaque/capa **oculta ou rejeitada** era publicada (com download em alta resolução) | ☑️ bloqueia a geração com aviso |

### 16. Album
| # | Gravidade | Problema | Estado |
|---|---|---|---|
| B1 | **Alta** | Fotografia indisponível (disco retirado, removida) → **moldura vazia** enviada ao laboratório e trabalho "Concluído" | ☑️ exportação recusada com erros da verificação; falha com a lista das fotografias |
| B2 | Média | Retratos em molduras horizontais revelados com ~70 % dos píxeis necessários (ampliados na impressão) | ☑️ resolução pelo pior caso |
| B3 | Média | Reexportar para a mesma pasta misturava páginas antigas com as novas | ☑️ recusa e pede pasta limpa |

### 17. Backup · 18. Recovery
| # | Gravidade | Problema | Estado |
|---|---|---|---|
| K1 | **Alta** | Qualquer erro ao abrir (versão mais recente da aplicação, disco cheio, permissões) era tratado como dano: catálogo atual posto de lado e **substituído por uma cópia antiga** | ✅ só dano real; teste "catálogo mais recente" |
| K2 | Baixa | Restauro copiava sem sincronizar (corte de energia a meio deixava o projeto sem catálogo) | ☑️ cópia sincronizada + `rename` |
| K3 | Baixa | Importar um catálogo com alterações só no `-wal` perdia-as | ☑️ recusado com explicação |

## Condições adversas (resultados medidos)

| Condição | Resultado |
|---|---|
| Disco cheio — exportação | Aviso na estimativa; pausa "sem espaço" sem ficheiros a meio; cancelar liberta a fila; outra exportação funciona |
| Disco cheio — importação (originais) | As que não cabem ficam com erro; nenhuma cópia parcial; depois de libertar espaço, a mesma importação traz as restantes sem duplicados |
| Disco cheio — disco de segurança | Fotografias sem segunda cópia não entram e são listadas ("No space left on device"); repetir completa tudo sem duplicados |
| Disco cheio — catálogo | 59/60 gravações recusadas com erro; nada corrompido; com espaço, grava; tudo intacto depois de reabrir |
| Destino só de leitura | Falha imediata com "Read-only file system" |
| Cartão só de leitura | Importa, mostra, edita, exporta; "mover para o lixo" recusado e a fotografia fica |
| Permissões negadas (utilizador sem privilégios) | Ficheiro e pasta ilegíveis no relatório; destino sem escrita falha com "Permission denied" |
| Unicode e nomes de 232 bytes | Importados, copiados, exportados (`Inês & João — brinde 💍 ação.jpg`) |
| Ficheiros corrompidos | RAW com lixo recusado com motivo; JPEG truncado catalogado com miniatura "falhada", fora da análise |
| Reinício | Classificações, edições, momentos, presets idênticos; catálogo válido |
| Fugas (25 ciclos abrir/fechar com previews, renders, exportações) | Threads 6–8, descritores 4, RSS estável (221–278 MB, sem tendência) |
| Projeto grande (15 000) | Ver `PERFORMANCE.md` (Fases 12 e 14) |

## Aplicação real (Linux, Xvfb, compilação de lançamento)

- Importação pelo diálogo do sistema de uma pasta `Cartão SD` com um ficheiro
  `Inês & João — brinde 💍.JPG`: 6 importadas, a cópia reconhecida como duplicada, nomes
  com acentos e emoji corretos no relatório.
- Mesmo cartão com outras fotografias nos mesmos caminhos (modo "manter no local"):
  revelou I11/X11 — corrigidos e testados.

## Não verificado

- macOS e Windows (as correções X1 e I5 visam precisamente estes sistemas; o teste usa a
  equivalente em Linux — ligação simbólica e nomes em minúsculas).
- Discos de rede (SMB/NFS) e cartões a falhar fisicamente.
