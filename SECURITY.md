# Segurança

## Modelo de ameaças (uso pessoal, aplicação local)

| Ameaça | Mitigação |
|---|---|
| Ficheiro de imagem malicioso/corrompido explora um descodificador | Descodificadores em Rust sempre que possível; código C/C++ (LibRaw) em processo auxiliar para leituras em massa (Fase 2); limites de dimensões e memória antes de descodificar |
| Conteúdo web (galerias, SVG) executa código na aplicação | CSP restritiva; SVG renderizado pelo `resvg` no núcleo (sem scripts), nunca injetado como HTML |
| Frontend comprometido acede ao sistema | Tauri capabilities mínimas (`core:default`); sem plugin genérico de sistema de ficheiros; caminhos resolvidos e validados no núcleo; comandos com argumentos tipados |
| Escrita acidental sobre originais | Módulos de escrita só escrevem dentro das pastas do projeto/exportação; teste automático de integridade (Fase 1) |
| Dependência maliciosa (cadeia de fornecimento) | Lockfiles versionados; pnpm `minimumReleaseAge` 7 dias; só `esbuild` autorizado a executar scripts de instalação; dependências revistas antes de adicionar |
| Fuga de credenciais | Keychain do sistema; `.env` ignorado; nenhuma credencial no código ou no Git |
| Perda de dados por falha | SQLite WAL, transações, escrita atómica, backups do catálogo |

## Configuração atual (Fase 0)

- CSP: `default-src 'self'; img-src 'self' asset: data: blob:; style-src 'self' 'unsafe-inline'; connect-src 'self' ipc: http://ipc.localhost`
  (`'unsafe-inline'` em estilos é necessário para estilos dinâmicos do React; scripts
  inline continuam proibidos). O protocolo `momento:` será acrescentado em `img-src` na
  Fase 1.
- Capabilities: `apps/desktop/src-tauri/capabilities/default.json` — apenas `core:default`.
- Comandos expostos: `app_info` (só leitura, sem argumentos).

## Comunicar problemas

Projeto pessoal: registar como issue privada no repositório.
