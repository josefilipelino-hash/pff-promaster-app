# Roadmap

Cada fase entrega código funcional, testado e documentado. Uma fase só termina quando
todos os critérios estão verificados e registados em `PROJECT_STATUS.md`.

## Fase 0 — Fundação técnica ✅ *(concluída 2026-11-08)*

- Análise e escolha da arquitetura (`ARCHITECTURE.md`).
- Estrutura do repositório (workspace Cargo + pnpm), Git, CI.
- Esqueleto Tauri funcional com comando de diagnóstico.
- Base de: catálogo SQLite com migrações, deteção de formatos por assinatura, contrato de
  codecs, modelo da fila de tarefas, envelope da receita de edição, i18n.

## Fase 1 — Aplicação utilizável ✅ *(concluída 2026-11-08, ver PROJECT_STATUS.md)*

Criar projeto · importar JPEG/PNG/TIFF (ficheiros, pastas recursivas, arrastar e largar) ·
deteção de duplicados exatos (BLAKE3) · thumbnails em segundo plano · grelha virtualizada ·
visualização individual com zoom · estrelas, escolhida/rejeitada, etiquetas de cor, atalhos
· filtros · guardar e reabrir · retoma de importação interrompida · backup do catálogo.

**Critérios de conclusão**

1. `pnpm dev` abre a aplicação em pt-PT; todos os textos vêm dos ficheiros de tradução.
2. Criar projeto (nome, noivos/cliente, tipo de evento, data, local) cria
   `<nome>.momento/` com `catalog.db`.
3. Importação de ficheiros, pastas recursivas e arrastar e largar para JPEG/PNG/TIFF, com
   formato detetado pelo conteúdo, duplicados exatos detetados e ficheiros corrompidos
   registados sem interromper.
4. Thumbnails geradas em segundo plano com progresso, sem bloquear a interface
   (teste com 2 000 JPEG sintéticos).
5. Grelha com 10 000 itens com deslocamento fluido; visualização individual com zoom.
6. Estrelas 0–5, escolhida/rejeitada, cores; atalhos 0–5, P, X, U, 6–9; filtros.
7. Fechar e reabrir preserva tudo; terminar o processo durante a importação e reabrir
   retoma-a.
8. Originais intactos: hash antes = hash depois (teste automático).
9. `./scripts/check.sh` passa; CI configurado para macOS, Windows e Linux.
10. `PROJECT_STATUS.md` e `INSTALLATION.md` atualizados.

## Fase 2 — Formatos e RAW ✅ *(concluída 2026-11-08, ver PROJECT_STATUS.md)*

LibRaw (DNG, CR2, CR3, NEF, ARW, RAF, ORF, RW2 e restantes), previews embutidas, HEIC/HEIF
(ImageIO), WebP, AVIF, JPEG XL, PSD composto, metadados EXIF/IPTC/XMP, perfis ICC
embutidos, processo auxiliar de descodificação, registo de erros por ficheiro,
sincronização de hora entre câmaras. **Critério-chave:** cada formato marcado como
“testado” em `SUPPORTED_FORMATS.md` tem um ficheiro real verificado.

## Fase 3 — Seleção automática ✅ *(concluída 2026-11-09, ver PROJECT_STATUS.md)*

Duplicados (hash e percetual), semelhança, rajadas e sequências, nitidez global e facial,
exposição, clipping, olhos fechados, expressões, recomendação explicada por grupo,
comparação lado a lado com zoom sincronizado, proteção de fotografias.

## Fase 4 — Organização de casamentos ✅ *(concluída 2026-11-09, ver PROJECT_STATUS.md)*

> Por pedido do utilizador, a organização do casamento (planeada como Fase 5) foi feita
> antes da edição. A numeração seguinte desloca-se uma fase.

Pessoas (deteção e agrupamento local de rostos, noiva/noivo/pais/irmãos/padrinhos/
madrinhas, pesquisas “noiva + mãe”, “noivos juntos”), 25 momentos automáticos com
confiança e “Por classificar”, momentos personalizados, arrastar entre momentos,
sincronização de hora entre câmaras com sugestão automática e cronologia reconstruída.

## Fase 5 — Edição ✅ *(concluída 2026-11-09, ver PROJECT_STATUS.md)*

Motor não destrutivo (luz, balanço de brancos, cor, HSL, curvas, detalhe, efeitos,
geometria, objetiva), histórico, instantâneos, cópias virtuais, copiar/colar/sincronizar,
antes/depois, histograma, qualidade dinâmica. **Por fazer (próximas fases):** color grading, presets (próprios, XMP, LUT), perfis de objetiva
automáticos (a correção automática por fotografia foi feita na Fase 6).

Também: consistência de cor entre câmaras e condições de luz; workflow de revisão.



## Fase 6 — Edição automática ✅ *(concluída 2026-11-09, ver PROJECT_STATUS.md)*

Edição automática por fotografia (exposição, balanço, realces/sombras, brancos/pretos,
contraste, curva, cor, ruído com ISO e câmara, nitidez, horizonte, objetiva), 6
intensidades, AI Denoise local opcional (FFDNet), redução rápida com preservação de
detalhe, recorte automático opcional, olhos vermelhos, manchas do sensor, antes/depois
dividido, "Editar automaticamente seleção" em segundo plano com pausa/retoma/cancelar.

## Fase 7 — Presets e consistência visual ✅ *(concluída 2026-11-09, ver PROJECT_STATUS.md)*

Presets (criar, guardar, renomear, organizar, duplicar, eliminar, importar, exportar,
pré-visualizar, aplicar a fotografia/seleção/momento/casamento), Amount, componentes, 13
presets internos, XMP aproximado com relatório, LUT .cube; tonalização e P&B; consistência:
agrupamento por iluminação/câmara/cenário/horário/balanço/flash e sincronização inteligente
com uma referência.

## Fase 8 — Edição localizada, retoque e retrato ✅ *(concluída 2026-11-09, ver PROJECT_STATUS.md)*

Máscaras (pincel, gradientes, luminância, cor, sujeito, pessoa, rosto, pele, cabelo, roupa,
céu, fundo, objeto) com somar/subtrair/intersetar, suavidade, fluxo, densidade e inverter; 13
ajustes locais; corrigir, clonar, manchas, pó do sensor, remoção simples de objetos; retrato
subtil (pele, olheiras, brilho, dentes, olhos).

## Fase 9 — Exportação

Receitas e lotes múltiplos, presets de exportação, gestão de cor completa (soft proofing,
gamut), marca de água, metadados e privacidade, impressão, redes sociais, arquivo
TIFF/DNG.

## Fase 10 — Galerias

Galeria estática com temas, slideshow, downloads, favoritas, palavra-passe (cifra), e
publicação opcional (S3/R2/B2/SFTP).

## Fase 11 — Álbuns

Editor, templates, montagem automática, margens, sangria, perfis, validações, exportação
PDF/JPEG/TIFF.

## Fase 12 — Otimização e distribuição

GPU (wgpu), otimização Apple Silicon, projetos grandes, recuperação, testes de carga,
assinatura de instaladores.

> **Windows** deixou de ser tarefa da fase de otimização: por decisão do utilizador (D-011), Windows e
> macOS são plataformas-alvo desde a Fase 1 — CI, compilação cruzada e instalador NSIS.

## Mais tarde

Aprendizagem local de preferências (opcional, transparente, reversível); idiomas es, fr,
de; plugins de codecs dinâmicos (se necessário).
