# Limitações conhecidas — PFF ProMaster 1.2.0-personal

**Data:** 2026-11-10 · Resultado da auditoria final (`FINAL_AUDIT.md`). Esta lista não
esconde nada: o que aqui está **não funciona, funciona só em parte ou não foi verificado**.
Os requisitos não implementados estão todos na matriz de `FINAL_AUDIT.md`; aqui ficam os
que têm impacto no uso real, por gravidade.

## 1. Plataforma e distribuição

| Limitação | Impacto | O que fazer |
|---|---|---|
| **A aplicação instalada nunca foi aberta num Mac nem num PC Windows por uma pessoa.** Os instaladores (`.dmg` universal, `.exe`) são gerados pela CI do GitHub (macOS 15, Windows Server 2022) e **todos os testes automáticos passam nos dois sistemas** (Windows: 288 passaram, 0 falharam). Não foram verificados: ícone, pedidos de permissão (TCC), HEIC/AVIF pelo ImageIO/WIC, fontes e ecrãs Retina/HiDPI, instalação e primeira abertura. | Problemas de interface ou de empacotamento específicos de cada sistema podem existir. | Antes de um trabalho real: instalar e correr a secção 2 de `RELEASE_CHECKLIST.md`. |
| A CI em Windows encontrou e permitiu corrigir um bug real (cópia do catálogo antes de migrar/restaurar recusada com "Access is denied"). | Corrigido em `fa6d07a`; os instaladores entregues incluem a correção. | — |
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

## 2a. Câmaras mais recentes do que o motor RAW (1.1.1)

- A LibRaw 0.22.3 não tem o perfil de cor de modelos lançados depois dela (ex.: **Canon EOS
  C50**). Nesses ficheiros as cores são **calibradas automaticamente** pela preview JPEG que a
  câmara grava no RAW: o resultado segue as cores (e o estilo) da própria câmara, não um
  perfil colorimétrico neutro. A temperatura aparece na escala relativa, sem Kelvin.
- Sem esse perfil a LibRaw também não aplica o balanço de brancos nem subtrai o nível de
  preto: a calibração (1.1.4) estima os dois pela preview. Em RAW reais com estes defeitos
  simulados recupera as cores; **não foi testada com um ficheiro C50 real** (não temos um).
- Se a calibração não for fiável, a edição usa a própria preview da câmara: cores certas,
  mas a resolução dessa preview (na C50, 810×540 — exportações ficam pequenas). O registo
  indica qual dos dois caminhos foi usado. A solução definitiva é uma LibRaw mais recente
  com o perfil desse modelo.

## 2c. Edição face ao Lightroom / Camera Raw (1.2)

Não foi feita nenhuma comparação lado a lado com o Lightroom ou o Camera Raw (não estão
disponíveis no ambiente de desenvolvimento): **não se afirma que sejam superados**. O que foi
medido está no `CHANGELOG.md`. Diferenças conhecidas:

- **Perfis de câmara:** a Adobe tem perfis de cor medidos por modelo de câmara; aqui as cores
  vêm das matrizes da LibRaw (e da calibração pela preview nas câmaras que ela não conhece).
- **Redução de ruído:** o "Denoise" da Adobe atua nos dados do sensor antes da desmatrização
  e foi treinado com RAW reais; o AI Denoise daqui (FFDNet, treinado com ruído gaussiano)
  atua depois da desmatrização, agora com o modelo de ruído do sensor e tratamento do ruído
  correlacionado. Em ISO muito alto pode ficar ruído de cor de baixa frequência.
- **Desmatrização e realces:** algoritmo da LibRaw (AHD) e mistura simples nos realces
  cortados; sem escolha de algoritmo nem reconstrução avançada de realces.
- **Perfis de objetiva:** só as objetivas da base Lensfun (1577); objetivas recentes podem
  faltar. Os NEF da Nikon muitas vezes não indicam a objetiva de forma legível para a LibRaw
  — nesses ficheiros o perfil não é encontrado. Sem perfil para objetivas olho de peixe.
- **Geometria:** o recorte é sempre restringido (sem zonas vazias, como "Restringir recorte"
  ligado); não há preenchimento de bordas por IA. O Upright guiado mostra a correção só ao
  concluir (não em tempo real enquanto se desenha).
- **Formatos em lote:** o recorte segue os rostos da análise; sem análise feita, usa o
  centro da composição atual (sem deteção de outros sujeitos).

## 2b. Lightroom e temperatura (1.1)

- **Não é gerado um catálogo `.lrcat`**: é uma base de dados privada da Adobe, sem
  documentação e diferente entre versões; um catálogo gerado poderia não abrir ou ficar
  danificado. As classificações vão em `.xmp` ao lado dos RAW (o caminho suportado pela Adobe).
- **DNG, JPEG, TIFF, HEIC, PSD não recebem classificação para o Lightroom**: o Lightroom só a
  lê de dentro destes ficheiros, que a aplicação nunca altera. Ficam listados no relatório.
- **A temperatura em Kelvin é uma estimativa própria** a partir dos coeficientes da câmara
  (LibRaw) e do locus do corpo negro: valores plausíveis em 20 RAW reais de 12 marcas
  (3100–6500 K), mas pode diferir algumas centenas de K do valor que o Lightroom mostra (usa
  os perfis de câmara da Adobe). Não foi comparada com o valor gravado pela câmara.
- A sincronização automática não leva recorte/rotação, máscaras, retoque nem retrato (são
  próprios de cada fotografia, como no Lightroom).

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
- **Edição:** microcontraste/textura automáticos, calibração e perfis de câmara, editar os
  valores de um preset, sidecars XMP (a receita vive no catálogo e num sidecar JSON).
  (Perspetiva automática — Upright — e perfis de objetiva existem desde a 1.2.)
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
