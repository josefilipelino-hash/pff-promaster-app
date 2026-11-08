# Modelos de IA

Os ficheiros de modelos (`.onnx`, `.mlpackage`, …) **não são versionados** no Git.

`manifest.json` lista, para cada modelo: identificador, versão, URL de origem, SHA-256,
tamanho, licença e tarefa. O script de download (Fase 3) descarrega para a pasta de suporte
da aplicação e recusa ficheiros cujo hash não coincida.

Nenhum modelo é adicionado sem a verificação descrita em `LICENSES.md` e
`ARCHITECTURE.md §11` (licença, tamanho, velocidade, compatibilidade, precisão,
privacidade, requisitos de hardware).
