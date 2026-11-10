# Fase 16 — Smoke test do build final (1.0.0-personal)

**Data:** 2026-11-10 · **Pacote testado:** `Momento Studio_1.0.0_amd64.deb` gerado por
`scripts/build-release.sh`, instalado com `dpkg -i` (como um utilizador o instalaria) e
executado de `/usr/bin/momento-desktop`, com uma pasta de dados vazia (primeira execução).
Também lançado: `Momento Studio_1.0.0_amd64.AppImage`.

**Ambiente:** Linux x86_64 (contentor, Xvfb). **O `.app`/`.dmg` de macOS não pode ser
gerado neste ambiente** (exige o SDK e as ferramentas da Apple): é gerado no Mac com o mesmo
script, a partir do mesmo código e das mesmas versões fixadas. O teste confirma o código, os
recursos empacotados (modelos) e o comportamento de produção; não confirma o empacotamento
macOS em si.

## Resultados

| Passo | O que foi feito | Resultado |
|---|---|---|
| Lançamento | Primeira execução, dados vazios | ✅ Início em ~1 s; "Momento Studio 1.0.0-personal · núcleo 1.0.0-personal · esquema 9" |
| Criação de projeto | "Novo projeto": *Casamento Inês e João*, *Inês & João*, 20/06/2026, pasta dos originais e de trabalho | ✅ Pasta `2026-06-20_Inês_e_João.momento` |
| Importação | Cartão `Cartão/DCIM/100MSDCF` com 3 RAW reais (Canon R3 CR3, Nikon D810 NEF, Sony A7 IV ARW) + 3 JPEG; "Copiar para a pasta dos originais" + "disco de segurança" + verificação, pelo diálogo do sistema | ✅ 6 importadas; cópias por câmara nos originais **e** no disco de segurança; cartão intacto (SHA-256 6/6) |
| Modelos de IA empacotados | Auto no NEF | ✅ "1 rosto considerado" — modelos carregados dos recursos do pacote |
| Edição | Develop do NEF em resolução total, ✦ Auto | ✅ Histórico "Edição automática" |
| Restart | "Fechar projeto", sair, relançar, abrir pelos recentes | ✅ Edição presente; versão do catálogo criada no fecho |
| Export | Plano "Exportar casamento" (5 receitas) das 6 fotografias | ✅ 30 ficheiros; NEF 4928×7380 (orientação aplicada), Instagram 1080×1350, TIFF 16 bits Adobe RGB, perfis ICC corretos |
| Recovery | 5★ numa fotografia e `kill -9` logo a seguir; relançar e abrir | ✅ "a sessão anterior não fechou o projeto; catálogo validado por completo"; 5★ e edição preservadas |
| Atualização de catálogo | Abrir projetos reais de versões anteriores: esquema **v7** (Fases 10–11) e **v3** (Fase 2) | ✅ Cópia `catalog-antes-da-atualizacao-v7/v3-*.db` criada antes da migração; dados idênticos antes/depois (55 fotografias, 25 momentos, 6 estrelas, 1 favorita); a cópia mantém a versão antiga |
| Originais em falta | Projeto v3 com 4 PSD que já não existem no disco | ✅ Marcados "em falta" (não apagados), restantes 35 normais |
| AppImage | Lançamento | ✅ 1.0.0-personal |

## Problemas encontrados e corrigidos

1. **Miniatura "pendente" para sempre** quando terminava enquanto a lista era recarregada
   (corrida na interface; o catálogo estava certo) → estados que chegam durante o
   recarregar são aplicados à lista nova; teste automático.
2. **"Analisar 0 fotografias"** depois da importação (contagem não recarregada) → recarregada
   no fim da importação.
3. Dica do botão Exportar com o atalho repetido → corrigida.

Os três estão na versão final (o build foi repetido depois das correções).
