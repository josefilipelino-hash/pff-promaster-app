"""Leitor estático de checkpoints PyTorch antigos (formato pré-1.6, `torch.save` legado).

Interpreta só os opcodes de pickle necessários com `pickletools.genops`, **sem importar
nem executar** nada do ficheiro (não usa `pickle.load`): as referências a classes ficam
como etiquetas ("Global", módulo, nome) e só se aceitam tensores `_rebuild_tensor_v2`.
Assim não é preciso instalar o PyTorch nem confiar no conteúdo do ficheiro descarregado
(que, de qualquer forma, é verificado por SHA-256 antes).
"""
import collections
import pickletools
import struct

import numpy as np


def parse(stream):
    stack, memo, mark = [], {}, []
    for op, arg, _ in pickletools.genops(stream):
        n = op.name
        if n in ("PROTO", "FRAME"):
            continue
        elif n == "STOP":
            return stack.pop()
        elif n == "EMPTY_DICT":
            stack.append({})
        elif n == "EMPTY_LIST":
            stack.append([])
        elif n == "EMPTY_TUPLE":
            stack.append(())
        elif n == "MARK":
            mark.append(len(stack))
        elif n in ("BINPUT", "LONG_BINPUT", "PUT"):
            memo[arg] = stack[-1]
        elif n == "MEMOIZE":
            memo[len(memo)] = stack[-1]
        elif n in ("BINGET", "LONG_BINGET", "GET"):
            stack.append(memo[arg])
        elif n in ("BINUNICODE", "SHORT_BINUNICODE", "UNICODE", "BINSTRING", "SHORT_BINSTRING",
                   "BININT", "BININT1", "BININT2", "LONG1", "INT", "LONG", "BINFLOAT"):
            stack.append(arg)
        elif n == "NONE":
            stack.append(None)
        elif n == "NEWTRUE":
            stack.append(True)
        elif n == "NEWFALSE":
            stack.append(False)
        elif n == "GLOBAL":
            mod, name = arg.split(" ", 1)
            stack.append(("Global", mod, name))
        elif n == "TUPLE":
            k = mark.pop()
            t = tuple(stack[k:])
            del stack[k:]
            stack.append(t)
        elif n in ("TUPLE1", "TUPLE2", "TUPLE3"):
            k = int(n[-1])
            t = tuple(stack[-k:])
            del stack[-k:]
            stack.append(t)
        elif n == "BINPERSID":
            stack.append(("Persistent", stack.pop()))
        elif n == "REDUCE":
            a = stack.pop()
            f = stack.pop()
            stack.append(("Call", f, a, []))
        elif n in ("SETITEM", "SETITEMS"):
            if n == "SETITEM":
                items = stack[-2:]
                del stack[-2:]
            else:
                k = mark.pop()
                items = stack[k:]
                del stack[k:]
            d = stack[-1]
            for i in range(0, len(items), 2):
                if isinstance(d, tuple):
                    d[3].append((items[i], items[i + 1]))
                else:
                    d[items[i]] = items[i + 1]
        elif n == "APPENDS":
            k = mark.pop()
            items = stack[k:]
            del stack[k:]
            stack[-1].extend(items)
        elif n == "APPEND":
            v = stack.pop()
            stack[-1].append(v)
        elif n == "BUILD":
            stack.pop()  # estado (metadados) ignorado
        else:
            raise ValueError(f"opcode não suportado: {n}")
    raise ValueError("fim inesperado")


DT = {"FloatStorage": np.float32, "HalfStorage": np.float16}


def load(path):
    """Devolve {nome: np.ndarray float32} de um state_dict guardado com torch.save (legado)."""
    with open(path, "rb") as f:
        for _ in range(3):  # número mágico, protocolo, informação do sistema
            parse(f)
        obj = parse(f)
        keys = parse(f)
        items = obj[3] if isinstance(obj, tuple) else list(obj.items())
        meta = {}
        for name, v in items:
            if not (v[0] == "Call" and v[1][2] == "_rebuild_tensor_v2"):
                raise ValueError(f"valor inesperado em {name}")
            st, off, size, stride = v[2][:4]
            pid = st[1]  # ('storage', ('Global', 'torch', 'FloatStorage'), chave, local, n)
            meta[name] = (pid[1][2], pid[2], off, size, stride)
        types = {m[1]: m[0] for m in meta.values()}
        raw = {}
        for k in keys:
            cnt = struct.unpack("<q", f.read(8))[0]
            dt = DT[types[k]]
            raw[k] = np.frombuffer(f.read(cnt * np.dtype(dt).itemsize), dtype=dt)
    out = collections.OrderedDict()
    for name, (_, key, off, size, stride) in meta.items():
        a = raw[key]
        out[name] = np.lib.stride_tricks.as_strided(
            a[off:], shape=size, strides=[s * a.itemsize for s in stride]
        ).astype(np.float32).copy()
    return out
