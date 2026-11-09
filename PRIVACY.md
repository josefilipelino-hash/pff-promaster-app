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
