#!/usr/bin/env python3
"""Reproduce Baz's paired CLAP ONNX towers from reviewed official files.

The script deliberately has no downloader. Supply a local snapshot which
matches ``official-laion.json``; every input byte is checked before pickle
weights are loaded. Outputs belong under the ignored ``local/`` directory.
"""

from __future__ import annotations

import argparse
import json
import shutil
import time
from pathlib import Path
from typing import Any

import numpy as np
import onnxruntime as ort
import torch
from onnxruntime.quantization import QuantType, quantize_dynamic
from tokenizers import Tokenizer
from transformers import ClapModel

import vibe_eval

OPSET = 17
COPY_FILES = (
    "README.md",
    "config.json",
    "merges.txt",
    "preprocessor_config.json",
    "special_tokens_map.json",
    "tokenizer.json",
    "tokenizer_config.json",
    "vocab.json",
)


class TextTower(torch.nn.Module):
    def __init__(self, model: ClapModel) -> None:
        super().__init__()
        self.model = model

    def forward(self, input_ids: torch.Tensor, attention_mask: torch.Tensor) -> torch.Tensor:
        return self.model.get_text_features(
            input_ids=input_ids,
            attention_mask=attention_mask,
        )


class AudioTower(torch.nn.Module):
    def __init__(self, model: ClapModel) -> None:
        super().__init__()
        self.model = model

    def forward(self, input_features: torch.Tensor) -> torch.Tensor:
        return self.model.get_audio_features(input_features=input_features)


def cosine_rows(left: np.ndarray, right: np.ndarray) -> np.ndarray:
    left = left / np.linalg.norm(left, axis=1, keepdims=True)
    right = right / np.linalg.norm(right, axis=1, keepdims=True)
    return np.sum(left * right, axis=1)


def all_prompts(path: Path) -> list[str]:
    prompts: list[str] = []
    for request in vibe_eval.read_json(path)["requests"]:
        prompts.append(request["query"])
        if request.get("avoid"):
            prompts.append(request["avoid"])
        prompts.extend(phase["query"] for phase in request.get("arc", []))
    return list(dict.fromkeys(prompts))


def text_inputs(tokenizer: Tokenizer, prompts: list[str]) -> tuple[np.ndarray, np.ndarray]:
    tokenizer.enable_truncation(max_length=77)
    tokenizer.enable_padding(length=77, pad_id=1, pad_token="<pad>")
    encoded = tokenizer.encode_batch(prompts)
    return (
        np.asarray([item.ids for item in encoded], dtype=np.int64),
        np.asarray([item.attention_mask for item in encoded], dtype=np.int64),
    )


def run_text(path: Path, ids: np.ndarray, masks: np.ndarray) -> np.ndarray:
    session = ort.InferenceSession(str(path), providers=["CPUExecutionProvider"])
    return session.run(None, {"input_ids": ids, "attention_mask": masks})[0]


def run_audio(path: Path, features: np.ndarray) -> np.ndarray:
    session = ort.InferenceSession(str(path), providers=["CPUExecutionProvider"])
    return session.run(None, {"input_features": features})[0]


def artifact(path: Path) -> dict[str, Any]:
    return {
        "name": path.name,
        "bytes": path.stat().st_size,
        "sha256": vibe_eval.sha256(path),
    }


def export(args: argparse.Namespace) -> None:
    source_manifest = vibe_eval.verify_artifacts(args.official, args.manifest)
    args.output.mkdir(parents=True, exist_ok=True)
    for name in COPY_FILES:
        shutil.copyfile(args.official / name, args.output / name)

    started = time.perf_counter()
    model = ClapModel.from_pretrained(args.official, local_files_only=True).eval()
    tokenizer = Tokenizer.from_file(str(args.official / "tokenizer.json"))
    ids, masks = text_inputs(tokenizer, all_prompts(args.requests))
    rng = np.random.default_rng(20260813)
    audio_features = rng.normal(-30.0, 10.0, (1, 1, 1001, 64)).astype(np.float32)

    text_fp32 = args.output / "text_model.onnx"
    audio_fp32 = args.output / "audio_model.onnx"
    with torch.inference_mode():
        reference_text = model.get_text_features(
            input_ids=torch.from_numpy(ids),
            attention_mask=torch.from_numpy(masks),
        ).numpy()
        reference_audio = model.get_audio_features(
            input_features=torch.from_numpy(audio_features)
        ).numpy()

        torch.onnx.export(
            TextTower(model),
            (torch.from_numpy(ids[:2]), torch.from_numpy(masks[:2])),
            text_fp32,
            input_names=["input_ids", "attention_mask"],
            output_names=["text_embeds"],
            dynamic_axes={
                "input_ids": {0: "batch_size", 1: "sequence_length"},
                "attention_mask": {0: "batch_size", 1: "sequence_length"},
                "text_embeds": {0: "batch_size"},
            },
            opset_version=OPSET,
            do_constant_folding=True,
            dynamo=False,
        )
        torch.onnx.export(
            AudioTower(model),
            torch.from_numpy(audio_features),
            audio_fp32,
            input_names=["input_features"],
            output_names=["audio_embeds"],
            dynamic_axes={
                "input_features": {0: "batch_size"},
                "audio_embeds": {0: "batch_size"},
            },
            opset_version=OPSET,
            do_constant_folding=True,
            dynamo=False,
        )

    text_quantized = args.output / "text_model_quantized.onnx"
    audio_quantized = args.output / "audio_model_quantized.onnx"
    quantize_dynamic(text_fp32, text_quantized, per_channel=True, weight_type=QuantType.QInt8)
    # CPUExecutionProvider does not implement the signed per-channel
    # ConvInteger emitted for CLAP's one patch projection. Its weights are only
    # a few KiB, so leave that Conv in FP32 and quantize the 99 substantial
    # matrix multiplications.
    quantize_dynamic(
        audio_fp32,
        audio_quantized,
        per_channel=True,
        weight_type=QuantType.QInt8,
        op_types_to_quantize=["MatMul", "Gemm"],
    )

    fp32_text_cos = cosine_rows(reference_text, run_text(text_fp32, ids, masks))
    quantized_text_cos = cosine_rows(reference_text, run_text(text_quantized, ids, masks))
    fp32_audio_cos = cosine_rows(reference_audio, run_audio(audio_fp32, audio_features))
    quantized_audio_cos = cosine_rows(
        reference_audio,
        run_audio(audio_quantized, audio_features),
    )
    checks = {
        "fp32_text_mean_cosine": float(fp32_text_cos.mean()),
        "fp32_text_worst_cosine": float(fp32_text_cos.min()),
        "quantized_text_mean_cosine": float(quantized_text_cos.mean()),
        "quantized_text_worst_cosine": float(quantized_text_cos.min()),
        "fp32_audio_cosine": float(fp32_audio_cos[0]),
        "quantized_audio_cosine": float(quantized_audio_cos[0]),
    }
    if checks["fp32_text_worst_cosine"] < 0.9999 or checks["fp32_audio_cosine"] < 0.9999:
        raise RuntimeError(f"FP32 export did not reproduce PyTorch: {checks}")
    if (
        checks["quantized_text_worst_cosine"] < 0.94
        or checks["quantized_audio_cosine"] < 0.95
    ):
        raise RuntimeError(f"quantized export exceeded the alignment budget: {checks}")

    outputs = [text_fp32, audio_fp32, text_quantized, audio_quantized]
    report = {
        "schema": 1,
        "source": source_manifest["source"],
        "source_weight_sha256": vibe_eval.sha256(args.official / "pytorch_model.bin"),
        "opset": OPSET,
        "versions": {
            "torch": torch.__version__,
            "transformers": __import__("transformers").__version__,
            "onnxruntime": ort.__version__,
        },
        "elapsed_seconds": time.perf_counter() - started,
        "checks": checks,
        "files": [artifact(path) for path in outputs],
    }
    vibe_eval.write_json(args.output / "export-report.json", report)
    print(json.dumps(report, indent=2))


def parser() -> argparse.ArgumentParser:
    root = Path(__file__).resolve().parent
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("official", type=Path, help="reviewed official checkpoint directory")
    result.add_argument("output", type=Path, help="ignored local output directory")
    result.add_argument("--manifest", type=Path, default=root / "official-laion.json")
    result.add_argument("--requests", type=Path, default=root / "requests.json")
    result.set_defaults(function=export)
    return result


def main() -> None:
    args = parser().parse_args()
    args.function(args)


if __name__ == "__main__":
    main()
