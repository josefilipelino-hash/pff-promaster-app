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

## Fase 3 — Seleção automática

Duplicados (hash e percetual), semelhança, rajadas e sequências, nitidez global e facial,
exposição, clipping, olhos fechados, expressões, recomendação explicada por grupo,
comparação lado a lado com zoom sincronizado, proteção de fotografias.

## Fase 4 — Edição

Motor não destrutivo (luz, cor, curvas, HSL, color grading, nitidez, denoise, correção de
lente, geometria), histórico, snapshots, cópias virtuais, copiar/colar/sincronizar,
presets (próprios, XMP, LUT), edição em lote, correção automática por fotografia.

## Fase 5 — Organização de casamentos

Momentos automáticos e personalizáveis, pessoas e noivos, consistência entre câmaras e
condições de luz, workflow de revisão.

## Fase 6 — Exportação

Receitas e lotes múltiplos, presets de exportação, gestão de cor completa (soft proofing,
gamut), marca de água, metadados e privacidade, impressão, redes sociais, arquivo
TIFF/DNG.

## Fase 7 — Galerias

Galeria estática com temas, slideshow, downloads, favoritas, palavra-passe (cifra), e
publicação opcional (S3/R2/B2/SFTP).

## Fase 8 — Álbuns

Editor, templates, montagem automática, margens, sangria, perfis, validações, exportação
PDF/JPEG/TIFF.

## Fase 9 — Otimização e distribuição

GPU (wgpu), otimização Apple Silicon, projetos grandes, recuperação, testes de carga,
assinatura de instaladores.

> **Windows** deixou de ser tarefa da Fase 9: por decisão do utilizador (D-011), Windows e
> macOS são plataformas-alvo desde a Fase 1 — CI, compilação cruzada e instalador NSIS.

## Mais tarde

Aprendizagem local de preferências (opcional, transparente, reversível); idiomas es, fr,
de; plugins de codecs dinâmicos (se necessário).
