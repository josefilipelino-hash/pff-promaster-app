# Privacidade

O Momento Studio processa fotografias privadas de clientes, incluindo rostos. As regras
abaixo são requisitos de arquitetura, não opções.

## Regras

1. **Processamento local por omissão.** Importação, análise, IA, edição e exportação
   correm no computador. A aplicação funciona sem Internet.
2. **Nada é enviado para fora sem autorização explícita.** As únicas ligações de rede
   previstas são:
   - download de modelos de IA, iniciado pelo utilizador, a partir das URLs do
     `models/manifest.json`, com verificação SHA-256 (sem envio de dados);
   - publicação de galerias (Fase 7), iniciada pelo utilizador, para o destino que ele
     configurar.
   Não existe telemetria, análise de utilização nem relatórios de falhas automáticos.
3. **Dados faciais** (caixas, landmarks, embeddings) ficam apenas no catálogo do projeto,
   no disco local, e são eliminados quando o projeto é eliminado. Existe opção para
   apagar só os dados faciais de um projeto.
4. **Sem treino externo.** As fotografias nunca são usadas para treinar serviços ou
   modelos externos. A aprendizagem de preferências (fase futura) é local, opcional,
   exportável e eliminável.
5. **Registos (logs)** nunca contêm píxeis, thumbnails, dados faciais nem metadados GPS.
   Caminhos de ficheiros podem aparecer em registos de erro locais. Ficam apenas no
   computador (macOS: `~/Library/Logs/pt.momentostudio.desktop/`; Windows:
   `%LOCALAPPDATA%\pt.momentostudio.desktop\logs\`).
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
   utilizador. “Gerir pessoas” permite apagar todos os dados de pessoas do projeto
   (`clear_people`: identidades, grupos e nomes). Os registos nunca contêm vetores faciais.
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
6. **Exportação:** opção para remover GPS e metadados privados; preset “Entrega sem
   metadados privados”.
7. **Credenciais** de publicação ficam no porta-chaves do sistema (Keychain no macOS),
   nunca no catálogo, em ficheiros de texto ou no código.
8. **Repositório:** fotografias, modelos e credenciais nunca são versionados (garantido
   pelo `.gitignore`).

## Eliminação

- **Eliminar projeto:** apaga catálogo, cache e dados faciais. Os originais **só** são
  apagados se o utilizador o pedir explicitamente, com confirmação separada.
- **Limpar cache:** apaga previews (regeneráveis).
- **Limpar dados faciais:** apaga rostos, embeddings e pessoas do projeto.

## Encriptação

Na v1 o catálogo não é cifrado (depende da encriptação do disco — FileVault no macOS,
recomendado). Avaliar SQLCipher numa fase posterior se houver necessidade.
