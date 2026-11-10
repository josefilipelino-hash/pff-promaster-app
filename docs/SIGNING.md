# Assinatura e notarização (macOS) — passos separados

A versão **1.0.0-personal** é usada **sem assinatura Apple**: funciona no Mac de quem a
compila (ver `INSTALLATION.md`). A assinatura e a notarização só são precisas para
distribuir a outras pessoas sem avisos do Gatekeeper. Nada disto bloqueia o
desenvolvimento nem o uso pessoal.

## O que é preciso (fornecido pelo utilizador)

| Item | Onde se obtém | Custo |
|---|---|---|
| Apple Developer Program | developer.apple.com/programs | 99 USD/ano |
| Certificado **Developer ID Application** | Xcode → Settings → Accounts → Manage Certificates, ou developer.apple.com → Certificates | incluído |
| App-specific password **ou** chave da App Store Connect API | appleid.apple.com (palavra-passe de aplicação) / App Store Connect → Users and Access → Keys | incluído |
| Team ID | developer.apple.com → Membership | — |

**Nunca** guardar estes valores no repositório: só em variáveis de ambiente locais ou nos
segredos da CI.

## 1. Assinar (Developer ID + hardened runtime)

Com o certificado instalado no Keychain do Mac:

```bash
security find-identity -v -p codesigning
```

Exportar a identidade e compilar (o Tauri assina a `.app` e o `.dmg`):

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: Nome (TEAMID)"
```

```bash
./scripts/build-release.sh
```

O Tauri ativa o *hardened runtime* ao assinar. A aplicação não usa sandbox (acesso a
cartões, discos externos e pastas escolhidas — ver `INSTALLATION.md`) nem JIT próprio, por
isso não precisa de entitlements adicionais.

## 2. Notarizar

Com chave da App Store Connect API (recomendado):

```bash
export APPLE_API_ISSUER="<issuer-id>"
```

```bash
export APPLE_API_KEY="<key-id>"
```

```bash
export APPLE_API_KEY_PATH="$HOME/private_keys/AuthKey_<key-id>.p8"
```

Ou com Apple ID:

```bash
export APPLE_ID="email@exemplo.pt"
```

```bash
export APPLE_PASSWORD="<app-specific-password>"
```

```bash
export APPLE_TEAM_ID="<TEAMID>"
```

Com estas variáveis definidas, `./scripts/build-release.sh` (que chama `tauri build`) envia
a aplicação para notarização e agrafa o resultado (*staple*) automaticamente.

## 3. Verificar

```bash
codesign --verify --deep --strict --verbose=2 "target/release/bundle/macos/PFF ProMaster.app"
```

```bash
spctl --assess --type execute --verbose "target/release/bundle/macos/PFF ProMaster.app"
```

```bash
xcrun stapler validate "target/release/bundle/dmg/PFF ProMaster_1.0.0_aarch64.dmg"
```

Resultado esperado de `spctl`: `accepted` e `source=Notarized Developer ID`.

## Na CI (GitHub Actions)

Os mesmos valores como *secrets* do repositório (`APPLE_CERTIFICATE` em base64 com
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_API_*`). O workflow só os
usa se existirem; sem eles produz o `.dmg` não assinado, como localmente.
