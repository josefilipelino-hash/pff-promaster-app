# Estado do projeto

**Última atualização:** 2026-11-09 · **Fase atual:** 6 — Edição automática · **Estado:** ✅ concluída
**Versão:** 0.6.0 · **Branch:** `main` · **Plataformas-alvo:** macOS (Apple Silicon e Intel) e Windows 10/11

---

## Resumo

O motor de edição ganhou **edição automática por fotografia**: cada imagem é medida
(histograma, rostos, balanço de cinzentos, saturação, ruído com o ISO e o sensor da câmara,
horizonte, objetiva) e recebe uma receita própria — editável e desfazível como qualquer
ajuste. Há 6 intensidades, **AI Denoise** local opcional, olhos vermelhos, manchas do
sensor, recorte automático opcional, antes/depois dividido e **"Editar automaticamente
seleção"** para centenas de fotografias em segundo plano (progresso, pausa, retoma,
cancelar). Tudo continua **não destrutivo** (originais verificados por hash nos testes).

## Funcional (verificado)

| Funcionalidade | Verificação |
|---|---|
| Análise por fotografia → exposição (com rostos), balanço, realces, sombras, brancos, pretos, contraste, curva, vibração/saturação, nitidez, horizonte, aberração cromática, vinheta (RAW) | Testes (escura clareia, dominante azul aquece, quente arrefece menos, escura e normal convergem); 8 fotografias numa fila → ≥ 6 combinações de luz diferentes; 10 fotografias reais avaliadas antes/depois |
| Intensidades: Muito suave, Suave, Natural, Equilibrada, Intensa, Personalizada (força + grupos) | Testes (exposição e vibração ordenadas; personalizada só mexe nos grupos pedidos) + interface |
| Ruído: medição (Immerkær nas zonas lisas), ISO × fator do sensor, sem redução forte em fotografias limpas | Testes (limpa → 0; σ ≈ 9/255 → alto; ISO 100 limita a 12) |
| Redução rápida com **preservação de detalhe** e de cor | Testes do motor (a omissão = comportamento da Fase 5) |
| **AI Denoise** (FFDNet, local) com o nível medido; no editor em segundo plano | Teste: PSNR 20,2 → 39,8 dB; render não bloqueia e o editor é avisado quando acaba |
| Antes/depois: alternar (\\) e **dividido** com divisória (Y) | Interface + aplicação real |
| Endireitar automático integrado (só inclinações de 0,25°–10°) | Teste (3° corrigido, 0° intacto) |
| Recorte automático opcional (só com pessoas, mantém proporção, nunca corta o rosto) | Teste |
| Olhos vermelhos (íris da deteção de rostos local) | Teste sintético + fotografias reais: **0 falsos em 33 rostos**, 2/2 simulados corrigidos |
| Manchas do sensor (céu/paredes) | Teste: 3/3 manchas num degradê, removidas; 0 numa imagem sem pó |
| Editar automaticamente seleção: fila com progresso, pausa, retoma, cancelar, acrescentar | Teste de integração + interface + aplicação real |
| Não destrutivo: entrada "Edição automática" no histórico, desfazer, originais intactos | Teste de integração (hash dos originais) |

## Desempenho medido (Linux x86_64, 4 núcleos, release)

| Operação | Resultado |
|---|---|
| Edição automática de um JPEG (análise + rostos + gravação) | ≈ 1,6–3,4 s (mais rostos = mais tempo) |
| Edição automática de um RAW (Canon 7D / Nikon D810 / Fuji X-E1) | ≈ 3–11 s (inclui revelação linear) |
| AI Denoise | ≈ 2,8–3,6 s por megapíxel (fonte de 6 MP ≈ 17–20 s) |

Num Mac com 8–10 núcleos espera-se 2–3× mais rápido, **não medido**.

## Não funcional / limitações

- **AI Denoise:** FFDNet foi treinado com ruído gaussiano em sRGB, não com RAW reais; é
  melhor do que a redução rápida mas não é uma reconstrução a partir do sensor. Lento em
  CPU; o resultado fica só em memória (é recalculado ao reabrir; as miniaturas editadas já
  o incluem). Sem o modelo (`prepare-models.py`), usa-se só a redução rápida.
- **Pausa/cancelar** atuam entre fotografias: a que está em curso termina.
- **Correção de objetiva:** sem perfis (Lensfun): a aberração cromática é medida; a vinheta
  só nos RAW e só quando os 4 cantos são uniformemente escuros; a distorção não é corrigida
  automaticamente.
- **Balanço automático:** cenas dominadas por uma cor (relva, pôr do sol) podem enganar a
  estimativa; por isso a correção é parcial e limitada.
- **Recorte automático:** só com pessoas; sem pessoas não recorta.
- **Olhos vermelhos:** precisam dos modelos de rostos; olhos muito pequenos (íris < 1,5 px) são ignorados.
- **Manchas:** só manchas pequenas e redondas em zonas lisas; não usa a repetição da mesma
  mancha em várias fotografias.
- Ainda da Fase 5: saída sRGB (sem perfil do monitor), sem máscaras/ajustes locais.
- macOS/Windows: compilação verificada, **não executada** nesses sistemas.

## Arquitetura

Decisões D-040 a D-045 em `ARCHITECTURE.md`. Novos módulos: `develop::auto` (análise e
receita), `face-analysis::denoise` (FFDNet por mosaicos), `project-management::auto_edit`
(fila, AI Denoise em segundo plano). Novo bloco de receita `retouch`; novos campos
`detail.noiseDetail` e `detail.aiDenoise` (receitas antigas dão o mesmo resultado).

## Dependências novas

Nenhuma biblioteca nova na aplicação. Modelo novo `denoise_ffdnet.onnx` (FFDNet, KAIR,
MIT, 3,3 MB), preparado por `scripts/prepare-models.py --only denoise` (SHA-256 da origem e
do resultado fixados; lido sem PyTorch e sem executar pickle).

## Comandos exatos

```bash
git submodule update --init --recursive
```

```bash
pnpm install
```

```bash
.venv-models/bin/python scripts/prepare-models.py --only denoise
```

```bash
pnpm dev
```

```bash
./scripts/check.sh
```

Avaliar a edição automática com fotografias tuas (nada é escrito nos originais):

```bash
cargo run --release -p momento-project-management --example auto_eval -- /pasta/fotos /pasta/saida natural
```

## Testes realizados (2026-11-09)

| Verificação | Resultado |
|---|---|
| `cargo test --workspace --release` (com RAW reais e referências) | ✅ 174 passados, 1 ignorado (débito de análise 24 MP) |
| `pnpm -r test` | ✅ 59 (53 da aplicação, 6 de traduções) |
| `pnpm -r typecheck`, `cargo fmt --check`, `cargo clippy -D warnings` | ✅ |
| Fotografias reais (`auto_eval`, 10 ficheiros JPEG/CR2/RAF/NEF) | ✅ resultados plausíveis; **0 olhos vermelhos falsos em 33 rostos**; 2/2 simulados corrigidos; 2 defeitos encontrados e corrigidos (falsos olhos vermelhos em luz linear; linha colorida na borda com aberração cromática) |
| Aplicação real (release, Linux, Xvfb): "Auto (8)" → diálogo → fila; pausa a 5/8 sem avançar; retomar → 8/8; miniaturas editadas na grelha; editor com relatório, vista dividida, olhos vermelhos corrigidos; AI Denoise de um RAW D810 com progresso e novo render no fim | ✅ (1 defeito encontrado e corrigido: o AI Denoise era calculado duas vezes em paralelo — editor e miniatura) |
| `cargo check --workspace --target x86_64-pc-windows-gnu` | ✅ |
| `cargo check` edição + automático + codecs `--target aarch64-apple-darwin` | ✅ (o `tract-linalg` dos modelos só compila num Mac/CI, como desde a Fase 3) |
| macOS / Windows reais | ⏳ não executado |

## Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Gosto da edição automática diferente do do fotógrafo | Médio | Intensidades, personalizada, tudo editável/desfazível; próximo passo: aprender com as correções |
| AI Denoise lento em portáteis | Médio | Opcional, em segundo plano; GPU na fase de otimização |
| Balanço automático em cenas de cor única | Baixo | Correção parcial e limitada; conta-gotas |

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
| 7 — Exportação | ⏸ próxima | — |

## Próximo passo

**Recomendado:** correr `auto_eval` (ou "Editar automaticamente seleção") num casamento teu
com várias câmaras e ISO altos, e dizer-me onde o resultado não te agrada (exposição,
balanço, ruído) — afino as regras com exemplos reais.

**Fase 7 — Exportação:** receitas e lotes, gestão de cor completa (perfis, soft proofing),
marca de água, metadados e privacidade.

**Pendente do utilizador:** repositório GitHub (CI em macOS/Windows); fotografias de teste;
nome definitivo.
