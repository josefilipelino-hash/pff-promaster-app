# Limitações conhecidas — PFF ProMaster 1.0.0-personal

**Data:** 2026-11-10 · Resultado da auditoria final (`FINAL_AUDIT.md`). Esta lista não
esconde nada: o que aqui está **não funciona, funciona só em parte ou não foi verificado**.
Os requisitos não implementados estão todos na matriz de `FINAL_AUDIT.md`; aqui ficam os
que têm impacto no uso real, por gravidade.

## 1. Plataforma e distribuição

| Limitação | Impacto | O que fazer |
|---|---|---|
| **O build macOS (.app/.dmg) não foi gerado nem testado neste ciclo.** Todo o desenvolvimento e o smoke test foram feitos em Linux (mesmo código, mesmas versões). | Ícone, Info.plist, pedidos de permissão (TCC), HEIC/AVIF pelo ImageIO, fontes e Retina nunca foram vistos num Mac. | Antes de um trabalho real: `./scripts/build-release.sh` no Mac e a secção 2 de `RELEASE_CHECKLIST.md`. |
| **Windows nunca foi testado** (a CI compilava; não há execução real). | Compatibilidade com Windows não confirmada. | Testar num PC antes de usar. |
| Não assinada nem notarizada. | O macOS pede confirmação na primeira abertura (clique direito → Abrir). | `docs/SIGNING.md`. |
| Sem atualização automática. | Atualizar = instalar a nova versão por cima (o catálogo é copiado antes de migrar). | `INSTALLATION.md`. |
| **Instaladores gerados pela CI do GitHub (macOS .dmg, Windows .exe/.msi) usam modelos de IA convertidos na própria CI**, das mesmas fontes oficiais (verificadas por SHA-256) e com o mesmo script, mas que **não ficam byte a byte iguais** aos validados (`models/models.lock.json`) e não foram comparados com as respostas de referência. Decisão do utilizador. Os SHA-256 usados ficam em `BUILD_INFO.txt`. | Resultados da IA (rostos, cena, máscaras, denoise) podem diferir ligeiramente dos testados em Linux. | Para usar os validados: gerar no Mac com `scripts/build-release.sh` e a pasta `models/` validada. |
| Leitores de ecrã (VoiceOver) não testados. | Acessibilidade só verificada com axe-core nos testes da interface. | — |

## 2. Fiabilidade e cor

| Limitação | Impacto |
|---|---|
| **LibRaw corre no mesmo processo** (o processo auxiliar planeado não existe). Pânicos em Rust são apanhados por ficheiro, mas uma falha nativa grave num RAW malformado fecha a aplicação. | O catálogo fica íntegro (WAL, `synchronous=FULL`) e a sessão é recuperada ao reabrir; perde-se só o trabalho em memória (ex.: exportação a meio, retomável). |
| **Ficheiros não-RAW de 16 bits ou HDR (TIFF 16, PNG 16, PSD 16, EXR) são revelados a 8 bits por canal.** | Edição forte de um TIFF 16 bits pode mostrar bandas; EXR/HDR perdem os valores acima do branco. Os RAW usam 16 bits lineares. |
| Pré-visualização do editor em sRGB, sem perfil do monitor nem Display P3. | Cores muito saturadas (fora de sRGB) aparecem cortadas no ecrã, mas são exportadas corretamente em Adobe RGB/ProPhoto/P3. |
| Sem soft proofing nem aviso de gamut. | Não há prova de impressão no ecrã; o fora-de-gama é cortado sem aviso. |
| JPEG CMYK convertido sem o perfil CMYK. | Cores aproximadas (marcado "Parcial" na importação). |
| DNG gravado em BigTIFF é tratado como imagem (não como RAW). | Raro; esse DNG perde a revelação RAW. |
| HEIC/AVIF no Windows: o WIC não converte a cor (um HEIC P3 é lido como sRGB). | Windows não testado de qualquer forma. |
| Catálogo em disco de rede não suportado (SQLite WAL). | Usar disco local ou externo. |
| Nomes de ficheiro com mais de 255 bytes falham individualmente. | Erro do sistema mostrado por ficheiro. |

## 3. Funcionalidades da especificação não implementadas (principais)

- **Importação:** deteção automática de cartões/DCIM e dispositivos USB (PTP/MTP),
  renomear na importação, modelos de estrutura de pastas, leitura de IPTC/XMP dos
  ficheiros, PCX/SVG/PDF como entrada, pacotes de codecs instaláveis.
- **Projeto:** estados do projeto (só criado/a importar/importado, sem interface), muitos
  campos da especificação (fotógrafos, contrato, entrega…), editar o projeto depois de
  criado, definir a pasta dos originais depois da criação, arquivar, capa/estado/progresso
  no ecrã inicial, exportar o projeto completo (só o catálogo).
- **Seleção/IA:** balanço de brancos, direção do olhar, rostos ocultos, obstruções,
  qualidade estética, relevância narrativa, pessoas importantes fora de foco, classe
  "problema técnico", bloquear fotografias (existe só a indicação "Protegida" para
  favoritas/estrelas), RAW+JPEG em pastas diferentes, upscaling, aprendizagem de
  preferências entre projetos.
- **Pessoas e momentos:** mover um rosto para outra pessoa, pessoas principais por
  momento, câmara/objetiva nos momentos, renomear/apagar momentos na interface.
- **Edição:** microcontraste/textura/perspetiva/distorção automáticos, perfis de objetiva,
  calibração e perfis de câmara, editar os valores de um preset, sidecars XMP (a receita
  vive no catálogo e num sidecar JSON).
- **Exportação:** HEIF/HEIC (sem codificador HEVC livre), DNG, GIF/MP4, ACEScg, limite de
  tamanho de ficheiro, receitas para redes sociais genéricas/Facebook, registo das
  exportações feitas.
- **Galerias:** publicação direta (S3, R2, B2, SFTP) — a galeria é uma pasta para copiar
  para o alojamento; data de expiração; cores personalizadas; a capa só gera `og:image`
  relativo (inútil para pré-visualizações em redes sociais).
- **Álbuns:** tipo de papel, grelha, troca de fotografias entre slots, exportar um spread
  isolado.
- **Interface:** idiomas es/fr/de e seletor de idioma (existem pt-PT e en; o `en` nunca é
  escolhido; as mensagens de erro do núcleo são sempre em português), modo de seleção
  rápida, dois monitores, "Fotografias editadas" na barra lateral.
- **Desempenho:** GPU, Neural Engine/Core ML (a inferência é tract-onnx em CPU), limitação
  de recursos configurável.

## 4. Privacidade — o que fica depois de apagar

- **Apagar dados de pessoas** apaga as assinaturas faciais (biométricas), pessoas e nomes.
  Ficam as caixas dos rostos e os coeficientes de expressão (usados na seleção) e o
  embedding da fotografia inteira (duplicados). As versões antigas do catálogo em
  `backups/` mantêm os dados anteriores até a rotação as substituir. Para apagar tudo:
  **Eliminar projeto** (vai para o Lixo; esvaziar o Lixo é definitivo).
- O catálogo não é cifrado: depende da encriptação do disco (FileVault recomendado).
- Galerias protegidas: as imagens têm nomes aleatórios mas **não** são cifradas.

## 5. Verificação

- Testes automáticos em Linux: ver `PROJECT_STATUS.md` (Rust e Vitest).
- `organize_flow`, testes de modelos de referência, RAW reais e discos cheios/só de
  leitura só correm com as variáveis de ambiente das amostras (`TESTING.md`); sem elas são
  saltados.
- 165 dos 479 requisitos da matriz não têm teste automático (`FINAL_AUDIT.md`).
