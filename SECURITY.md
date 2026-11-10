# Segurança

## Modelo de ameaças (uso pessoal, aplicação local)

| Ameaça | Mitigação |
|---|---|
| Ficheiro de imagem malicioso/corrompido explora um descodificador | Descodificadores em Rust sempre que possível; código C/C++ (LibRaw, LittleCMS) corre **no mesmo processo** (o processo auxiliar previsto não foi implementado — ver `KNOWN_LIMITATIONS.md`); pânicos em Rust são apanhados por ficheiro (`catch_unwind`), mas uma falha nativa grave num ficheiro RAW malformado pode fechar a aplicação (o catálogo fica íntegro e a sessão é recuperada ao reabrir); limites de dimensões e memória antes de descodificar |
| Conteúdo web (galerias, SVG) executa código na aplicação | CSP restritiva; SVG renderizado pelo `resvg` no núcleo (sem scripts), nunca injetado como HTML |
| Frontend comprometido acede ao sistema | Tauri capabilities mínimas (`core:default`); sem plugin genérico de sistema de ficheiros; caminhos resolvidos e validados no núcleo; comandos com argumentos tipados |
| Escrita acidental sobre originais | Módulos de escrita só escrevem dentro das pastas do projeto/exportação; teste automático de integridade (Fase 1) |
| Dependência maliciosa (cadeia de fornecimento) | Lockfiles versionados; pnpm `minimumReleaseAge` 7 dias; só `esbuild` autorizado a executar scripts de instalação; dependências revistas antes de adicionar |
| Fuga de credenciais | Keychain do sistema; `.env` ignorado; nenhuma credencial no código ou no Git |
| Perda de dados por falha | SQLite WAL, transações, escrita atómica, backups do catálogo |

## Configuração atual (Fase 1)

- CSP: `default-src 'self'; img-src 'self' momento: http://momento.localhost asset: data: blob:; style-src 'self' 'unsafe-inline'; connect-src 'self' ipc: http://ipc.localhost`
  (`'unsafe-inline'` em estilos é necessário para estilos dinâmicos do React; scripts
  inline continuam proibidos).
- Capabilities (`apps/desktop/src-tauri/capabilities/default.json`): `core:default`,
  `core:window:allow-set-fullscreen`, `core:window:allow-is-fullscreen` (ecrã completo) e
  `dialog:allow-open` (seletores de ficheiros/pastas nativos). Não há acesso genérico ao
  sistema de ficheiros a partir da interface.
- **Protocolo de imagens `momento://`**: serve apenas ficheiros identificados por
  `token do projeto + nível + ID numérico`; nunca aceita caminhos (teste
  `parses_only_valid_paths` rejeita `../`). O token muda a cada abertura de projeto.
- Comandos expostos: ver `apps/desktop/src-tauri/src/commands.rs`. Caminhos recebidos
  (importação, abrir projeto) são validados no núcleo; os originais são sempre abertos só
  para leitura.
- Limite de 200 megapíxeis por imagem antes de alocar memória; pânicos em descodificadores
  são isolados e o ficheiro é marcado como falhado.
- **Código C++ (LibRaw):** acedido só através da camada `shim/` própria, que apanha todas as
  exceções C++ e expõe estruturas POD; limite de memória da LibRaw de 4 GB por ficheiro.
  Os pânicos de qualquer adaptador Rust são convertidos em erro “corrompido”. Testado com
  ficheiros reais malformados (que fizeram falhar outras bibliotecas) e com dados aleatórios
  com cabeçalhos RAW de 7 fabricantes. Risco residual: um segfault dentro da LibRaw
  terminaria a aplicação (ver ARCHITECTURE.md D-017).
- O JPEG truncado e os ficheiros falsos (texto com extensão `.jpg`) dos testes não afetam a
  aplicação.

## Comunicar problemas

Projeto pessoal: registar como issue privada no repositório.
