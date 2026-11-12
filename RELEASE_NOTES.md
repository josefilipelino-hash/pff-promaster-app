# PFF ProMaster 1.2.1-personal

**Correção:** miniaturas esmagadas em "Luz semelhante" e Pessoas. **Editor reorganizado**
(separadores, secção Básico, controlos numa linha) e **zoom rápido** (Z, duplo clique, roda
do rato, arrastar). Ver `CHANGELOG.md`.

---

# PFF ProMaster 1.2.0-personal

**Edição a sério de RAW:** copiar/colar a edição para várias fotografias; perspetiva
projetiva com **Upright** (Auto, Nível, Vertical, Completo, Guiado); **formatos de recorte**
do mercado por fotografia ou em lote; **perfis de objetiva** automáticos (Lensfun);
**AI Denoise** com o modelo de ruído do sensor. Detalhes e medições em `CHANGELOG.md`;
o que ainda não iguala o Lightroom em `KNOWN_LIMITATIONS.md` (secção 2c).

---

# PFF ProMaster 1.1.4-personal

**Correção (1.1.4):** Canon EOS C50 sem dominante magenta/oliva — a calibração retira o
nível de preto que a LibRaw não subtrai nesta câmara; se não for fiável, usa-se a preview
da própria câmara (nunca a revelação sem perfil). Ver `CHANGELOG.md` (1.1.3 e 1.1.4).

---

# PFF ProMaster 1.1.2-personal

**Correção (1.1.2):** a Canon EOS C50 já não aparece verde/oliva — a calibração passa a
corrigir também o balanço de brancos, que a câmara sem perfil não recebia.

---

# PFF ProMaster 1.1.1-personal

**Correção (1.1.1):** fotografias de câmaras mais recentes do que o motor RAW (ex.: Canon
EOS C50) já não aparecem com dominante magenta: as cores são calibradas pela preview que a
própria câmara grava no ficheiro.

---

# PFF ProMaster 1.1.0-personal

**Data:** 2026-11-11 · **Tipo:** versão para uso pessoal (assinatura local *ad-hoc*, sem certificado Apple)

## Novo na 1.1

- **Classificações para o Lightroom** — Seleção automática → *Classificações para o
  Lightroom…*: **Melhor 5★, Alternativa 3★, Rever 2★** em ficheiros `.xmp` ao lado de cada
  RAW. O Lightroom lê-os ao importar a pasta, ou em *Metadados → Ler metadados dos ficheiros*.
  Os RAW nunca são alterados. Opcional: aplicar as mesmas estrelas no PFF ProMaster.
  (Um catálogo `.lrcat` não é gerado — formato privado da Adobe; ver `KNOWN_LIMITATIONS.md`.)
- **Temperatura em Kelvin nos RAW** (2000–50 000 K, a partir do balanço da câmara).
- **Editar várias fotografias ao mesmo tempo**: *Sincronizar auto* no editor, presets
  aplicados a todas as selecionadas, seleção múltipla na tira (Cmd/Ctrl+clique, Shift+clique).
- **Novo visual** com a barra do fluxo *Importar → Selecionar → Editar → Entregar*.
- **macOS recente**: a aplicação passa a ter assinatura local; primeira abertura em
  *Definições do Sistema → Privacidade e segurança → Abrir mesmo assim*.
- **Windows**: corrigida a cópia do catálogo antes de migrar/restaurar ("Access is denied").

---

# Versão anterior: 1.0.0-personal


**Data:** 2026-11-10 · **Tipo:** versão para uso pessoal (não assinada) · **Tag:** `v1.0.0-personal`

Primeira versão instalável. Fluxo completo de um casamento, local e sem internet:
cartão → importação segura → seleção → edição → exportação → galeria do cliente → álbum
para o laboratório.

## O que faz

| Área | |
|---|---|
| Projetos | Um projeto por trabalho (pasta `.momento`), catálogo SQLite gravado a cada alteração (sem "Guardar"), versões automáticas, recuperação de sessões interrompidas |
| Importação | Cartões/pastas, cópia verificada (BLAKE3) para a pasta dos originais **e** para um disco de segurança em simultâneo; duplicados pelo conteúdo; relatório de tudo o que não entrou |
| Formatos | RAW de câmara via LibRaw (fabricantes testados com ficheiros reais em `SUPPORTED_FORMATS.md`), JPEG, TIFF, PNG, WebP, JPEG XL, JPEG 2000, PSD, EXR, HEIC/AVIF (macOS; não validado em Mac neste ciclo). Perfis ICC embutidos aplicados; ficheiros não-RAW de 16 bits/HDR são revelados a 8 bits por canal (ver `KNOWN_LIMITATIONS.md`) |
| Library / Culling | Grelha virtualizada (15 000 fotografias), estrelas, favoritas, rejeitadas, cores, comparação, menus contextuais, atalhos de teclado |
| Seleção assistida | Nitidez, olhos fechados, exposição, rajadas e duplicados — sugestões que nunca se sobrepõem às decisões do utilizador |
| Pessoas e momentos | Rostos agrupados localmente; momentos do casamento por cena e hora, várias câmaras sincronizadas |
| Develop | Revelação não destrutiva de alta precisão, curvas, HSL, máscaras (pincel, gradientes, sujeito/céu por IA), retrato, retoque, AI Denoise, histórico, instantâneos, cópias virtuais |
| Auto edit e presets | Edição automática por fotografia ou em lote, presets e LUTs, consistência entre fotografias |
| Exportação | JPEG, JPEG XL, JPEG 2000, PNG, TIFF/BigTIFF (8–32 bits), WebP, AVIF, OpenEXR, PDF, PSD (HEIF/HEIC e DNG **não** exportam — ver `EXPORT_FORMATS.md`); perfis de cor; redimensionar, nitidez de saída, marca de água, metadados (GPS removível); planos com várias receitas numa operação |
| Galeria | Site estático para o cliente, com PIN/palavra-passe (AES-256-GCM), capítulos por momento, transferências |
| Álbum | Formatos de laboratório, layouts automáticos pela narrativa do dia, verificação (DPI, rostos na dobra), PDF de impressão com sangria e JPEG/TIFF por spread |

## Auditoria final (antes de fechar a 1.0)

Comparação de toda a implementação com a especificação: **`FINAL_AUDIT.md`** (matriz de
479 requisitos e três listas). Corrigidos dois problemas críticos (realces/sombras locais
sem efeito; perfil ICC embutido ignorado na edição/exportação) e os altos/médios
encontrados; as ações de privacidade prometidas passaram a existir (Segurança e cópias →
Privacidade). A aplicação chama-se agora **PFF ProMaster**. Tudo o que não funciona ou
não foi verificado: **`KNOWN_LIMITATIONS.md`**.

## Novo nesta versão (Fase 16)

- **Atualização segura:** antes de mudar o formato de um catálogo existente, a aplicação
  guarda uma cópia completa em `backups/catalog-antes-da-atualizacao-v<N>-<data>.db`; sem
  essa cópia não altera nada. Uma versão antiga recusa abrir um catálogo mais recente.
- Pedidos de acesso do macOS com explicações (cartões e discos externos, Documentos,
  Secretária, Transferências, rede).
- Build de produção reproduzível (`scripts/build-release.sh`): ferramentas fixadas,
  dependências pelos locks, modelos de IA verificados por SHA-256, `BUILD_INFO.txt` e
  `SHA256SUMS.txt`.
- Mais pequeno: sem source maps (−2,3 MB), sem informação de depuração.
- Registos limitados aos últimos 14 dias.

Correções e estabilização das fases anteriores: ver `CHANGELOG.md` (0.13–0.15) e
`docs/reports/fase15-auditoria.md`.

## Requisitos

macOS 12+ (Apple Silicon recomendado; Intel suportado pelo build universal), 8 GB de
memória (16 GB recomendado), ~200 MB para a aplicação. Projetos em disco local. Compilar:
ver `INSTALLATION.md`.

## Instalar, atualizar, backup

Ver `INSTALLATION.md` → "Versão 1.0.0-personal no Mac": instalar (primeira abertura com
clique direito → Abrir), permissões, atualizar sem perder catálogos, backup, desinstalar.

## Limitações conhecidas

Lista completa em **`KNOWN_LIMITATIONS.md`**. As principais:

- Ficheiros não-RAW de 16 bits/HDR revelados a 8 bits; sem soft proofing nem perfil do
  monitor; LibRaw no mesmo processo; sem rede (publicação direta de galerias não existe).

- **Não assinada nem notarizada** (uso pessoal): o macOS pede confirmação na primeira
  abertura. Passos para assinar: `docs/SIGNING.md`.
- **O build macOS não foi executado neste ciclo de desenvolvimento** (ambiente Linux): o
  `.app`/`.dmg` é gerado no Mac com `scripts/build-release.sh` ou pela CI (macos-15). O
  mesmo código foi compilado e testado em Linux (smoke test do pacote de produção,
  ver abaixo). Antes de um trabalho real, correr a secção 2 de `RELEASE_CHECKLIST.md` no Mac.
- Fontes do sistema, Retina e controlos nativos verificados só por equivalência em Linux
  (escala 2×).
- Leitores de ecrã (VoiceOver) não testados.
- Catálogo em disco de rede não suportado (SQLite WAL).
- Nomes de ficheiro com mais de 255 bytes falham individualmente com o erro do sistema.
- Reimportar um cartão já importado lê todos os ficheiros (decide pelo conteúdo — mais
  lento, nunca perde fotografias).
- Tamanho: ~102 MB são modelos de IA (9 modelos, todos usados; mantidos em precisão total
  para manter os resultados validados).

## Verificação desta versão

- Testes automáticos: núcleo Rust (com RAW reais, modelos e rostos, discos cheios/só de
  leitura) e interface — ver `PROJECT_STATUS.md`.
- Casamento completo de ponta a ponta com reinício (`wedding_e2e`).
- Smoke test do pacote de produção (Linux): lançamento, criação de projeto, importação,
  edição, reinício, exportação, recuperação depois de terminar o processo à força.
- Checksums dos pacotes: `SHA256SUMS.txt` gerado pelo build.
