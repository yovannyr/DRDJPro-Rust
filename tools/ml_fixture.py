"""Generate a tiny, untrained ONNX Conv/GRU graph using only Python stdlib.

ONNX IR8/opset13, protobuf field numbers from onnx/onnx.proto (Apache-2.0).
Independent scalar reference implements ONNX GRU's linear_before_reset=0.
No framework/weights download, training data or beat-detection claim.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct

INPUTS = [
    [0.0] * 8,
    [0.0, 0.25, -0.5, 1.0, -1.0, 0.75, 0.125, -0.25],
    [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
]
CONV = [0.25, -0.5, 0.125]
W = [0.2, -0.1, 0.3, 0.15, -0.4, 0.25]  # z, r, h; hidden size 2
R = [0.1, -0.2, 0.05, 0.15, -0.1, 0.2, 0.1, -0.05, 0.2, 0.1, -0.15, 0.3]
B = [0.01, -0.02, 0.03, 0.0, -0.01, 0.02, 0.0, 0.01, 0.02, -0.01, 0.01, 0.0]


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def reference(samples):
    """Scalar equations, independent of either inference runtime."""
    weights, recurrent, biases = map(lambda values: list(map(f32, values)), (W, R, B))
    hidden = [0.0, 0.0]
    output = []
    for t in range(6):
        x = sum(f32(samples[t + k]) * f32(CONV[k]) for k in range(3)) + f32(0.05)
        gates = []
        for gate in range(2):
            values = []
            for j in range(2):
                row = gate * 2 + j
                v = weights[row] * x + biases[row] + biases[6 + row]
                v += sum(recurrent[row * 2 + k] * hidden[k] for k in range(2))
                values.append(1.0 / (1.0 + math.exp(-v)))
            gates.append(values)
        z, reset = gates
        candidate = []
        for j in range(2):
            row = 4 + j
            v = weights[row] * x + biases[row] + biases[6 + row]
            v += sum(recurrent[row * 2 + k] * reset[k] * hidden[k] for k in range(2))
            candidate.append(math.tanh(v))
        hidden = [(1 - z[j]) * candidate[j] + z[j] * hidden[j] for j in range(2)]
        output.extend(map(f32, hidden))
    return output


def varint(value):
    if value < 0:
        raise ValueError("only nonnegative protobuf varints used by this fixture")
    out = bytearray()
    while value > 127:
        out.append((value & 127) | 128)
        value >>= 7
    out.append(value)
    return bytes(out)


def integer(field, value):
    return varint(field << 3) + varint(value)


def blob(field, value):
    if isinstance(value, str):
        value = value.encode("utf-8")
    return varint(field << 3 | 2) + varint(len(value)) + value


def tensor(name, shape, values):
    return (b"".join(integer(1, d) for d in shape) + integer(2, 1)
            + blob(8, name) + blob(9, struct.pack("<" + "f" * len(values), *values)))


def value_info(name, shape):
    dimensions = b"".join(blob(1, integer(1, d)) for d in shape)
    return blob(1, name) + blob(2, blob(1, integer(1, 1) + blob(2, dimensions)))


def attribute(name, value):
    if isinstance(value, list):
        return blob(1, name) + integer(20, 7) + b"".join(integer(8, v) for v in value)
    return blob(1, name) + integer(20, 2) + integer(3, value)


def node(op, inputs, outputs, attributes=()):
    return (b"".join(blob(1, x) for x in inputs) + b"".join(blob(2, x) for x in outputs)
            + blob(4, op) + b"".join(blob(5, attribute(k, v)) for k, v in attributes))


def model_bytes():
    graph = blob(2, "drdj-a0-untrained-conv-gru")
    for op in [
        node("Conv", ["input", "conv_w", "conv_b"], ["conv"], [("kernel_shape", [3])]),
        node("Transpose", ["conv"], ["sequence"], [("perm", [2, 0, 1])]),
        node("GRU", ["sequence", "W", "R", "B"], ["output"], [("hidden_size", 2)]),
    ]:
        graph += blob(1, op)
    for initializer in [
        tensor("conv_w", [1, 1, 3], CONV), tensor("conv_b", [1], [0.05]),
        tensor("W", [1, 6, 1], W), tensor("R", [1, 6, 2], R), tensor("B", [1, 12], B),
    ]:
        graph += blob(5, initializer)
    graph += blob(11, value_info("input", [1, 1, 8]))
    graph += blob(12, value_info("output", [6, 1, 1, 2]))
    return integer(1, 8) + blob(2, "drdj-a0-stdlib") + blob(7, graph) + blob(8, integer(2, 13))


def manifest():
    model = model_bytes()
    return {"schema": 1, "model_sha256": hashlib.sha256(model).hexdigest(),
            "model_bytes": len(model), "absolute_tolerance": 1e-5,
            "cases": [{"input": x, "output": reference(x)} for x in INPUTS]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--reference", type=Path)
    args = parser.parse_args()
    args.model.parent.mkdir(parents=True, exist_ok=True)
    args.model.write_bytes(model_bytes())
    if args.reference:
        args.reference.parent.mkdir(parents=True, exist_ok=True)
        args.reference.write_text(json.dumps(manifest(), indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"model": str(args.model), "sha256": manifest()["model_sha256"],
                      "bytes": len(model_bytes())}))


if __name__ == "__main__":
    main()
