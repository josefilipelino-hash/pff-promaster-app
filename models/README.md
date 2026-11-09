# Modelos de IA

Os ficheiros de modelos (`.onnx`) **não são versionados** no Git.

- `manifest.json` — cada modelo: origem (URL oficial do MediaPipe), licença e tarefa.
- `models.lock.json` — SHA-256 da origem e de cada ONNX gerado.

Preparar (uma vez, em desenvolvimento; o CI faz o mesmo):

```bash
python3 -m venv .venv-models
.venv-models/bin/pip install tf2onnx==1.17.0 tensorflow-cpu==2.21.0 onnx==1.23.2
.venv-models/bin/python scripts/prepare-models.py
```

Sem estes ficheiros a aplicação funciona, mas a análise fica limitada às métricas técnicas
(sem rostos, olhos nem semelhança visual) e a interface avisa. Na aplicação instalada, os
modelos são incluídos como recursos (`tauri.conf.json` → `bundle.resources`).
Licenças: ver `LICENSES.md`. Decisões: `ARCHITECTURE.md` D-024 e D-025.
