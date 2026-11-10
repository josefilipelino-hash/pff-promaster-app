# Momento Studio *(nome provisório)*

Aplicação desktop local para gestão, seleção, edição, exportação e entrega de fotografia
de casamentos e outros eventos. Pensada para trabalhos de 3 000 a 15 000 fotografias, com
automatização em cada passo e o controlo final sempre do fotógrafo.

> **Estado atual: v1.0.0-personal — primeira versão instalável (uso pessoal).** Fluxo
> completo de um casamento: cartão → importação com cópia verificada e disco de segurança →
> RAW → miniaturas → análise e seleção assistida → momentos e pessoas → edição (global,
> local, retrato, automática, presets) → exportação em vários formatos → galeria do cliente →
> álbum para o laboratório. Testado de ponta a ponta, com 15 000 fotografias e em condições
> adversas (disco cheio, só de leitura, permissões, falhas forçadas). Instalação no Mac:
> [`INSTALLATION.md`](INSTALLATION.md) · notas: [`RELEASE_NOTES.md`](RELEASE_NOTES.md).
> Ainda **não** verificado em macOS/Windows reais (o `.dmg` gera-se no Mac com
> `scripts/build-release.sh`).

![Grelha](docs/screenshots/fase1-grelha.png)

Plataformas-alvo: **macOS** (Apple Silicon e Intel) e **Windows 10/11**.

## Princípios

- Funciona localmente, sem Internet obrigatória.
- Os originais nunca são alterados nem apagados automaticamente.
- Edição não destrutiva (receitas de parâmetros num catálogo SQLite).
- Processamento local de rostos e IA; nada é enviado para fora sem autorização.
- Compatibilidade abrangente com formatos profissionais, RAW e formatos modernos, com
  sistema extensível de codecs.

## Stack

Tauri 2 · React 19 · TypeScript · Rust · SQLite · LibRaw · LittleCMS 2 · ONNX Runtime /
Core ML. Justificação completa em [`ARCHITECTURE.md`](ARCHITECTURE.md).

## Início rápido

```bash
git submodule update --init --recursive
```

```bash
pnpm install
```

```bash
pnpm dev
```

Requisitos e instruções completas: [`INSTALLATION.md`](INSTALLATION.md) e
[`DEVELOPMENT.md`](DEVELOPMENT.md).

## Documentação

| Documento | Conteúdo |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Arquitetura, decisões e incompatibilidades |
| [ROADMAP.md](ROADMAP.md) | Fases 0–17 e critérios |
| [RELEASE_NOTES.md](RELEASE_NOTES.md) | Notas da versão 1.0.0-personal |
| [docs/SIGNING.md](docs/SIGNING.md) | Assinatura e notarização (macOS) |
| [PROJECT_STATUS.md](PROJECT_STATUS.md) | O que funciona realmente hoje |
| [INSTALLATION.md](INSTALLATION.md) | Instalação e execução |
| [DEVELOPMENT.md](DEVELOPMENT.md) | Convenções de desenvolvimento |
| [TESTING.md](TESTING.md) | Estratégia e comandos de testes |
| [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) | Verificação antes de cada versão e de um trabalho real |
| [RELIABILITY.md](RELIABILITY.md) | Comportamento em falhas e condições adversas |
| [PERFORMANCE.md](PERFORMANCE.md) | Medições com 15 000 fotografias |
| [DESIGN.md](DESIGN.md) | Sistema de design, teclado, acessibilidade |
| [docs/reports/fase15-auditoria.md](docs/reports/fase15-auditoria.md) | Auditoria módulo a módulo |
| [SUPPORTED_FORMATS.md](SUPPORTED_FORMATS.md) | Formatos de leitura e estado de teste |
| [EXPORT_FORMATS.md](EXPORT_FORMATS.md) | Formatos de exportação |
| [PRIVACY.md](PRIVACY.md) | Privacidade |
| [SECURITY.md](SECURITY.md) | Segurança |
| [LICENSES.md](LICENSES.md) | Dependências e licenças |
| [CHANGELOG.md](CHANGELOG.md) | Histórico de versões |

## Estrutura

```
apps/desktop/          Aplicação (React em src/, Tauri/Rust em src-tauri/)
packages/              Módulos: crates Rust do núcleo + ui e localization (TypeScript)
models/                Manifesto de modelos de IA (pesos fora do Git)
tests/                 Fixtures sintéticas e documentação de testes
scripts/               Scripts de verificação e utilitários
docs/                  Documentação adicional e capturas de ecrã
```

Uso pessoal. Sem licença de distribuição definida.
