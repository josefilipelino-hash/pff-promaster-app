# Estado do projeto

**Última atualização:** 2026-11-10 · **Fase atual:** 10 — Galerias · **Estado:** ✅ concluída
**Versão:** 0.10.0 · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

---

## Resumo

Cada projeto gera a sua **galeria do cliente**: uma pasta HTML estática, pronta a publicar,
que também abre diretamente do disco. Nomes, data, local, texto, hero, capa e logótipo;
capítulos automáticos pelos momentos; temas Light, Dark, Minimal e Editorial; mosaico,
grelha, visualizador, apresentação e ecrã inteiro em telemóvel, tablet e desktop;
favoritas, downloads individuais e ZIP, marca de água, PIN/palavra-passe e fotografias
ocultas. As imagens saem da fila de exportação a partir dos originais. A edição não depende
da Internet e nenhum serviço externo é contactado; a publicação direta (S3, R2, B2, SFTP)
tem só a arquitetura.

## Funcional (verificado)

| Funcionalidade | Verificação |
|---|---|
| Nomes, data, local, capa, hero, logótipo, texto introdutório | Testes; aplicação real (valores iniciais do projeto, hero da fotografia em foco) |
| Capítulos automáticos: Preparação, Cerimónia, Sessão, Família e amigos, Cocktail, Receção, Festa, personalizados | Testes; aplicação real: 10 momentos → 7 capítulos |
| Temas Light, Dark, Minimal, Editorial | Capturas no Chromium (desktop e telemóvel) |
| Mosaico, grelha, visualizador, ecrã inteiro, apresentação | Playwright: sem sobreposições, colunas 4/3/2, setas, Esc, endereço direto, apresentação avança/pausa |
| Telemóvel, tablet, desktop | Playwright (iPhone 13, iPad, 1440 px): colunas certas, sem deslocamento horizontal |
| Favoritas (coração, filtro, lista, email, ZIP no navegador por HTTP) e importação na aplicação | Playwright + teste de fluxo (etiqueta roxa) |
| Download individual e completo (ZIP, também por capítulo) | Testes (conteúdo do ZIP) + Playwright (ficheiros existem) |
| Marca de água, copyright, sem GPS | Teste de fluxo (EXIF lido de volta) |
| PIN/palavra-passe | Nada visível sem o código (testes); PIN errado/certo no Chromium |
| Ocultar fotografias | Nunca escritas na galeria (teste de fluxo) |
| Substituição segura | Só pastas com o marcador do Momento, depois de confirmar; outras nunca tocadas |
| Arquitetura de publicação | Plano de envio, incremental, configurações sem segredos (testes) |

## Desempenho medido (Linux x86_64, 4 núcleos, release)

| Operação | Resultado |
|---|---|
| Galeria de 55 fotografias (~0,8 MP; miniatura + ecrã + download + ZIP), aplicação real | 11,7 s, 175 ficheiros, 39 MB (estimativa prévia: 58 MB) |
| Código da galeria (HTML+CSS+JS) | ≈ 48 KB, sem bibliotecas; fontes 72 KB |

Fotografias de 24 MP demoram o mesmo que a exportação (Fase 9): a revelação do original
domina. Mac/Windows: **não medido**.

## Não funcional / limitações

- **PIN/palavra-passe:** cifra o índice (nomes, textos, endereços), não as imagens: quem
  tiver o endereço direto de uma imagem consegue abri-la; um PIN curto pode ser adivinhado
  por tentativa offline. Explicado na interface.
- **Favoritas:** ficam no navegador do cliente; voltam à aplicação por lista (copiar/email).
  O ZIP das favoritas só funciona com a galeria publicada (não aberta do disco).
- **Sem servidor:** sem expiração, estatísticas, comentários nem avisos de novas favoritas.
- **Publicação direta (S3, R2, B2, SFTP):** só arquitetura; publicar = copiar a pasta.
- "Abrir galeria" usa o navegador predefinido do sistema (`open`/`xdg-open`/`start`).
- Mac/Windows não executados.

## Arquitetura

Decisões D-064 a D-070. Crate `momento-gallery` (`settings`, `chapters`, `crypto`, `site`,
`publish`, assets HTML/CSS/JS e fontes); `project-management::gallery` (definições no
catálogo, geração pela fila de exportação, favoritas do cliente).

## Dependências novas

aes-gcm, pbkdf2, sha2, base64, getrandom (RustCrypto e afins, MIT/Apache), zip (MIT);
fontes Instrument Serif/Sans (OFL-1.1). Ver `LICENSES.md`.

## Comandos exatos

```bash
pnpm dev
```

```bash
./scripts/check.sh
```

Galeria de demonstração a partir de uma pasta de JPEG e teste num navegador real:

```bash
cargo run --release -p momento-gallery --example gallery_demo -- fotos/ galeria/ editorial
```

```bash
node scripts/test-gallery.cjs galeria/
```

## Testes realizados (2026-11-10)

| Conjunto | Resultado |
|---|---|
| `cargo test --release --workspace` (com amostras RAW, modelos e rostos) | ✅ 239 passaram, 0 falharam, 1 ignorado (débito 24 MP, manual) |
| `pnpm -r test` (Vitest) | ✅ 65 (desktop) + 6 (localização) |
| `tsc --noEmit`, `cargo fmt --check`, `cargo clippy -D warnings` | ✅ limpos |
| `scripts/test-gallery.cjs` (Chromium, `file://` e HTTP, telemóvel/tablet/desktop) | ✅ galerias de demonstração (aberta e com PIN) e a gerada pela aplicação real |
| Aplicação real (build release, Linux/Xvfb) | ✅ diálogo (4 separadores), 7 capítulos pelos momentos, tema editorial, hero, pasta nativa, geração na fila, "Abrir galeria" |

Novos testes: `gallery` (7), `tests/gallery_flow.rs`, interface (+2), `scripts/test-gallery.cjs`.

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Clientes esperam proteção total com PIN | Médio | Limitação explicada; palavra-passe recomendada; nomes aleatórios; `noindex` |
| Galerias muito grandes (3000+ fotografias) no telemóvel | Médio | Miniaturas 900 px com `loading=lazy`; mosaico sem bibliotecas; testar com um casamento real |
| Alojamentos com listagem de pastas ativa | Baixo | Aviso na interface |

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
| 11 — Álbuns | ⏸ próxima | — |

## Próximo passo

**Recomendado:** gerar a galeria de um casamento teu (centenas de fotografias), abri-la no
telemóvel e publicá-la num alojamento estático de teste; dizer-me o que achas dos temas.

**Fase 11 — Álbuns:** editor de álbuns com templates, montagem automática, margens,
sangria, perfis de laboratório e exportação.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); fotografias de teste;
nome definitivo.
