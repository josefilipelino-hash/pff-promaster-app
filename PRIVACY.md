# Privacidade

O PFF ProMaster processa fotografias privadas de clientes, incluindo rostos. As regras
abaixo são requisitos de arquitetura, não opções.

## Regras

1. **Processamento local por omissão.** Importação, análise, IA, edição e exportação
   correm no computador. A aplicação funciona sem Internet.
2. **Nada é enviado para fora.** A aplicação 1.0.0-personal **não faz nenhuma ligação de
   rede**: os modelos de IA vêm incluídos na aplicação (preparados antes da compilação por
   `scripts/prepare-models.py`, com verificação SHA-256) e a publicação direta de galerias
   não está implementada (a galeria é uma pasta local). Não existe telemetria, análise de
   utilização nem relatórios de falhas automáticos.
3. **Dados faciais** (caixas, coeficientes de expressão, assinaturas faciais) ficam apenas
   no catálogo do projeto, no disco local, e vão com ele quando o projeto é eliminado
   (**Segurança e cópias → Privacidade → Eliminar projeto**). **Apagar dados de pessoas**
   apaga as assinaturas faciais (dados biométricos), as pessoas e os nomes; as caixas dos
   rostos ficam porque a seleção as usa (olhos fechados, expressões).
4. **Sem treino externo.** As fotografias nunca são usadas para treinar serviços ou
   modelos externos. A aprendizagem de preferências (fase futura) é local, opcional,
   exportável e eliminável.
5. **Registos (logs)** nunca contêm píxeis, thumbnails, dados faciais nem metadados GPS.
   Desde a auditoria final, **cada linha** passa por um filtro antes de chegar ao disco que
   substitui caminhos (que têm nomes de clientes) por `<caminho>` e emails por `<email>`;
   os erros da interface também ficam registados assim. Ficam 14 dias, apenas no
   computador (macOS: `~/Library/Logs/pt.pffpromaster.desktop/`; Windows:
   `%LOCALAPPDATA%\pt.pffpromaster.desktop\logs\`).
5f. **Fase 8 — máscaras de IA e retrato:** a segmentação (pessoa, cabelo, pele, rosto,
   roupa, objeto) e a deteção dos pontos faciais correm neste computador. As máscaras ficam
   como imagens em `<projeto>/masks/` e os contornos faciais do retrato na receita da
   fotografia (catálogo e sidecar do projeto); nada é enviado para fora, e removê-los da
   receita é sempre possível. Não são guardados na base de dados de pessoas.
5e. **Fase 7 — presets e consistência:** os presets do utilizador ficam na biblioteca local
   da aplicação e as LUT importadas na pasta de dados da aplicação (copiadas para o projeto
   quando usadas). As características de luz (EV, dominante, flash) ficam no catálogo do
   projeto. A exportação de presets só acontece para um ficheiro escolhido pelo utilizador
   (permissão `dialog:allow-save`); nada é enviado para fora.
5d. **Fase 6 — edição automática e AI Denoise:** a análise de cada fotografia, a deteção
   de rostos/íris (para a exposição dos rostos, olhos vermelhos e recorte) e o AI Denoise
   (FFDNet) correm neste computador. Os pontos das íris **não são guardados**: só os
   círculos das correções de olhos vermelhos ficam na receita (catálogo e sidecar JSON do
   projeto). O resultado do AI Denoise fica só em memória (e nas miniaturas/previews
   editadas em cache do projeto); nada é enviado para fora nem escrito junto dos originais.
5c. **Fase 4 — pessoas (dados biométricos):** a identidade facial (vetor de 128 valores
   por rosto, SFace) é calculada e guardada **só** no `catalog.db` do projeto; nunca é
   enviada para fora, nem usada entre projetos. Nomes e papéis são escritos pelo
   utilizador. **Segurança e cópias → Privacidade → Apagar dados de pessoas** apaga todos os dados de
   pessoas do projeto (`clear_people`: assinaturas faciais, grupos e nomes). Os registos nunca contêm vetores faciais.
5b. **Fase 3 — análise e IA:** toda a análise (métricas, rostos, olhos, expressões,
   embeddings) corre neste computador com modelos incluídos na aplicação (`tract-onnx`,
   sem serviços externos). Os modelos são preparados **antes** da compilação; a aplicação
   não descarrega nada. Os dados faciais (caixas, 52 coeficientes de expressão por rosto,
   embedding de 1024 valores por fotografia) ficam só em `catalog.db` do projeto;
   “Reanalisar tudo” apaga-os e volta a calculá-los. Os registos indicam só o
   identificador numérico da fotografia e o erro, nunca dados faciais.
5a. **Fase 1:** a aplicação não faz nenhuma ligação de rede. As fotografias são lidas
   apenas para gerar miniaturas na pasta do projeto; os originais são abertos só para
   leitura.
6. **Exportação (Fase 9):** os metadados dos ficheiros exportados são **escritos de novo**
   a partir do catálogo (nunca copiados do original), por isso o que se escolhe é o que
   fica: "tudo" (câmara, data, autor, copyright), "só copyright e contacto" ou "nada".
   **Remover localização (GPS)** está ligado por omissão em todas as receitas, exceto
   "Arquivo TIFF" (cópia pessoal). Os dados faciais, pessoas, momentos e classificações
   da IA **nunca** são escritos nos ficheiros (só a classificação por estrelas, no modo
   "tudo"). A exportação é local: grava apenas na pasta escolhida, sem ligações de rede.
6b. **Galerias (Fase 10):** a galeria é uma pasta local; a aplicação não a envia para lado
   nenhum. A galeria não faz pedidos externos (sem fontes, estatísticas ou bibliotecas de
   terceiros) e só contém as fotografias escolhidas (as ocultas e as rejeitadas nunca são
   escritas). Metadados: só copyright/autor, **sem GPS**. Com PIN/palavra-passe, os nomes,
   textos e endereços das imagens ficam cifrados (AES-GCM) e a página pede aos motores de
   pesquisa para não indexar; as imagens têm nomes aleatórios mas **não** estão cifradas —
   quem tiver o endereço direto de uma imagem consegue abri-la. As favoritas do cliente
   ficam só no navegador dele até as enviar. O PIN fica no catálogo local do projeto.
7. **Credenciais:** a versão 1.0.0-personal não usa credenciais (sem publicação direta).
   Se a publicação vier a existir, as credenciais ficam no porta-chaves do sistema,
   nunca no catálogo, em ficheiros de texto ou no código.
8. **Repositório:** fotografias, modelos e credenciais nunca são versionados (garantido
   pelo `.gitignore`).
6c. **Álbuns (Fase 11):** os álbuns e os presets de laboratório ficam no catálogo local.
   A exportação (JPEG, TIFF, PDF) grava apenas na pasta escolhida, sem metadados (nem
   EXIF nem GPS) e sem ligações de rede; nada é enviado a laboratórios — o utilizador
   envia os ficheiros como quiser. As caixas dos rostos servem só para o recorte e as
   verificações, nunca são escritas nos ficheiros.
6d. **Diagnóstico de desempenho (Fase 12):** só com `MOMENTO_PERF=1`. Regista no log local
   tempos, contagens e memória (ex.: "grelha pronta em 242 ms, 15 000 fotografias") —
   nunca nomes de ficheiros, caminhos, imagens ou metadados. Nada é enviado.
6e. **Fiabilidade (Fase 13):** versões do catálogo ficam em `backups/` dentro do projeto;
   a cópia de segurança da importação vai para o disco que o utilizador escolher. Os crash
   logs (`logs/crash-*.txt`) ficam só no computador e **não** contêm fotografias, metadados,
   caminhos (substituídos por `<caminho>`) nem emails; nada é enviado.

## Eliminação (Segurança e cópias → Privacidade)

- **Eliminar projeto…:** com confirmação, fecha o projeto e move a pasta `.momento`
  inteira (catálogo, versões, cache, máscaras, dados faciais) para o **Lixo** do sistema,
  de onde pode ser recuperada até o Lixo ser esvaziado. Os originais e as exportações
  **nunca** são tocados; se algum original estiver dentro da pasta do projeto, a
  eliminação é recusada.
- **Limpar cache:** apaga miniaturas, previews e edições renderizadas (regeneráveis; as
  miniaturas voltam a ser geradas logo).
- **Apagar dados de pessoas:** apaga assinaturas faciais, pessoas e nomes. As versões
  antigas do catálogo em `backups/` mantêm-nos até serem substituídas pela rotação; para
  apagar tudo, eliminar o projeto.
- Remover fotografias: **Remover do catálogo** (ficheiros intactos) ou **Mover para o
  Lixo** (o original vai para o Lixo do sistema, com confirmação).

## Encriptação

Na v1 o catálogo não é cifrado (depende da encriptação do disco — FileVault no macOS,
recomendado). Avaliar SQLCipher numa fase posterior se houver necessidade.
