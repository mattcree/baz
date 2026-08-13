#!/usr/bin/env python3
"""Offline, model-swappable semantic playlist evaluation for Baz.

This is development tooling, not application code. It never downloads a model
or reads Baz's library database: every input path and artifact directory must be
supplied explicitly by the evaluator.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import random
import re
import statistics
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

SCHEMA = 1
RATE = 48_000
SEGMENT_SAMPLES = RATE * 10
HOP_SAMPLES = SEGMENT_SAMPLES // 2
EMBEDDING_DIMENSIONS = 512


def artifact_manifest(candidate: str, explicit: Path | None) -> Path:
    if explicit is not None:
        return explicit
    names = {"dclap": "artifacts.json", "laion": "artifacts-laion.json"}
    try:
        name = names[candidate]
    except KeyError as error:
        raise ValueError(f"unknown candidate: {candidate}") from error
    return Path(__file__).resolve().parent / name


def candidate_system(candidate: str) -> str:
    systems = {
        "dclap": "dclap-v1",
        "laion": "laion-clap-htsat-unfused-quantized",
    }
    try:
        return systems[candidate]
    except KeyError as error:
        raise ValueError(f"unknown candidate: {candidate}") from error


def require_candidate_index(index: dict[str, Any], candidate: str, path: Path) -> None:
    expected_system = candidate_system(candidate)
    if index.get("system") != expected_system:
        raise ValueError(
            f"{path}: candidate {candidate} requires an {expected_system} index, "
            f"found {index.get('system', 'unknown')}"
        )


def read_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if value.get("schema") != SCHEMA:
        raise ValueError(f"{path}: expected schema {SCHEMA}")
    return value


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        json.dump(value, handle, ensure_ascii=False, indent=2)
        handle.write("\n")


def corpus_fingerprint(tracks: list[dict[str, Any]]) -> str:
    """Portable FNV-1a over every identity/ranking-relevant corpus field."""
    value = 0xCBF29CE484222325

    def absorb(text: str) -> None:
        nonlocal value
        data = text.encode("utf-8")
        for byte in len(data).to_bytes(8, "little") + data:
            value ^= byte
            value = (value * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF

    for track in tracks:
        for field in ("id", "path", "title", "artist", "album", "genre"):
            absorb(str(track.get(field, "")))
        tags = track.get("tags", [])
        absorb(str(len(tags)))
        for tag in tags:
            absorb(str(tag))
    return f"fnv1a64:{value:016x}"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while block := handle.read(1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


def verify_artifacts(model_dir: Path, manifest_path: Path) -> dict[str, Any]:
    manifest = read_json(manifest_path)
    failures: list[str] = []
    for expected in manifest["files"]:
        path = model_dir / expected["name"]
        if not path.is_file():
            failures.append(f"missing {expected['name']}")
            continue
        size = path.stat().st_size
        if size != expected["bytes"]:
            failures.append(f"{expected['name']}: {size} bytes, expected {expected['bytes']}")
            continue
        actual = sha256(path)
        if actual != expected["sha256"]:
            failures.append(f"{expected['name']}: sha256 {actual}, expected {expected['sha256']}")
    if failures:
        raise ValueError("artifact verification failed:\n  " + "\n  ".join(failures))
    return manifest


def window_starts(samples: int, maximum: int | None) -> list[int]:
    """Reference 50%-overlap windows, optionally sampled across the whole work."""
    if samples <= SEGMENT_SAMPLES:
        return [0]
    starts = list(range(0, samples - SEGMENT_SAMPLES + 1, HOP_SAMPLES))
    tail = samples - SEGMENT_SAMPLES
    if starts[-1] != tail:
        starts.append(tail)
    if maximum is None or len(starts) <= maximum:
        return starts
    if maximum < 2:
        return [starts[len(starts) // 2]]
    positions = [round(index * (len(starts) - 1) / (maximum - 1)) for index in range(maximum)]
    return [starts[position] for position in positions]


def normalize(vector: Any) -> Any:
    import numpy as np

    vector = np.asarray(vector, dtype=np.float32)
    norm = float(np.linalg.norm(vector))
    if not math.isfinite(norm) or norm <= 1e-9:
        raise ValueError("model returned an empty or non-finite embedding")
    return vector / norm


class Dclap:
    def __init__(
        self,
        model_dir: Path,
        text_model: str,
        *,
        load_audio: bool = True,
        load_text: bool = True,
    ) -> None:
        import onnxruntime as ort
        from tokenizers import Tokenizer

        self.model_dir = model_dir
        providers = ["CPUExecutionProvider"]
        self.audio = (
            ort.InferenceSession(str(model_dir / "model_epoch_36.onnx"), providers=providers)
            if load_audio
            else None
        )
        self.text = (
            ort.InferenceSession(str(model_dir / text_model), providers=providers)
            if load_text
            else None
        )
        self.tokenizer = Tokenizer.from_file(str(model_dir / "tokenizer.json")) if load_text else None
        if self.tokenizer is not None:
            self.tokenizer.enable_truncation(max_length=77)
            self.tokenizer.enable_padding(length=77)

    def embed_texts(self, texts: list[str]) -> Any:
        import numpy as np

        if self.tokenizer is None or self.text is None:
            raise RuntimeError("text tower was not loaded")
        encoded = self.tokenizer.encode_batch(texts)
        inputs = np.asarray([item.ids for item in encoded], dtype=np.int64)
        masks = np.asarray([item.attention_mask for item in encoded], dtype=np.int64)
        vectors = self.text.run(None, {"input_ids": inputs, "attention_mask": masks})[0]
        return np.asarray([normalize(vector) for vector in vectors], dtype=np.float32)

    def embed_audio(self, path: Path, maximum_windows: int | None) -> tuple[Any, int, dict[str, float]]:
        import librosa
        import numpy as np

        if self.audio is None:
            raise RuntimeError("audio tower was not loaded")
        started = time.perf_counter()
        audio, _ = librosa.load(path, sr=RATE, mono=True)
        decode_seconds = time.perf_counter() - started
        audio = np.clip(audio, -1.0, 1.0)
        audio = (audio * 32767.0).astype(np.int16).astype(np.float32) / 32767.0
        starts = window_starts(len(audio), maximum_windows)
        embeddings = []
        mel_seconds = 0.0
        inference_seconds = 0.0
        for start in starts:
            segment = audio[start : start + SEGMENT_SAMPLES]
            if len(segment) < SEGMENT_SAMPLES:
                segment = np.pad(segment, (0, SEGMENT_SAMPLES - len(segment)))
            before = time.perf_counter()
            mel = librosa.feature.melspectrogram(
                y=segment,
                sr=RATE,
                n_fft=2048,
                hop_length=480,
                win_length=2048,
                window="hann",
                center=True,
                pad_mode="reflect",
                power=2.0,
                n_mels=128,
                fmin=0,
                fmax=14_000,
            )
            mel = librosa.power_to_db(mel, ref=1.0, amin=1e-10, top_db=None)
            mel = mel[None, None].astype(np.float32)
            mel_seconds += time.perf_counter() - before
            before = time.perf_counter()
            embeddings.append(self.audio.run(None, {"mel_spectrogram": mel})[0][0])
            inference_seconds += time.perf_counter() - before
        vector = normalize(np.mean(embeddings, axis=0))
        return vector, len(starts), {
            "decode_seconds": decode_seconds,
            "mel_seconds": mel_seconds,
            "inference_seconds": inference_seconds,
        }


class LaionClap:
    """Paired quantized export of the permissive official LAION checkpoint."""

    def __init__(
        self,
        model_dir: Path,
        text_model: str | None,
        *,
        load_audio: bool = True,
        load_text: bool = True,
    ) -> None:
        import onnxruntime as ort
        from tokenizers import Tokenizer

        providers = ["CPUExecutionProvider"]
        self.audio = (
            ort.InferenceSession(str(model_dir / "audio_model_quantized.onnx"), providers=providers)
            if load_audio
            else None
        )
        self.text = (
            ort.InferenceSession(
                str(model_dir / (text_model or "text_model_quantized.onnx")), providers=providers
            )
            if load_text
            else None
        )
        self.tokenizer = Tokenizer.from_file(str(model_dir / "tokenizer.json")) if load_text else None
        self.text_uses_mask = self.text is not None and any(
            item.name == "attention_mask" for item in self.text.get_inputs()
        )
        if self.tokenizer is not None:
            self.tokenizer.enable_truncation(max_length=77)
            if self.text_uses_mask:
                self.tokenizer.enable_padding(length=77, pad_id=1, pad_token="<pad>")

    def embed_texts(self, texts: list[str]) -> Any:
        import numpy as np

        if self.tokenizer is None or self.text is None:
            raise RuntimeError("text tower was not loaded")
        if self.text_uses_mask:
            encoded = self.tokenizer.encode_batch(texts)
            inputs = np.asarray([item.ids for item in encoded], dtype=np.int64)
            masks = np.asarray([item.attention_mask for item in encoded], dtype=np.int64)
            vectors = self.text.run(
                None,
                {"input_ids": inputs, "attention_mask": masks},
            )[0]
        else:
            # The reviewed Xenova graph omits `attention_mask`. Padding several
            # prompts to one length therefore changes their embeddings
            # materially, whichever token id fills the tail. Run each naturally
            # sized prompt; Baz's reproduced export can batch safely above.
            vectors = [
                self.text.run(
                    None,
                    {"input_ids": np.asarray([self.tokenizer.encode(text).ids], dtype=np.int64)},
                )[0][0]
                for text in texts
            ]
        return np.asarray([normalize(vector) for vector in vectors], dtype=np.float32)

    def embed_audio(self, path: Path, maximum_windows: int | None) -> tuple[Any, int, dict[str, float]]:
        import librosa
        import numpy as np

        if self.audio is None:
            raise RuntimeError("audio tower was not loaded")
        started = time.perf_counter()
        audio, _ = librosa.load(path, sr=RATE, mono=True)
        decode_seconds = time.perf_counter() - started
        starts = window_starts(len(audio), maximum_windows)
        embeddings = []
        mel_seconds = 0.0
        inference_seconds = 0.0
        for start in starts:
            segment = audio[start : start + SEGMENT_SAMPLES]
            if len(segment) < SEGMENT_SAMPLES:
                segment = np.pad(segment, (0, SEGMENT_SAMPLES - len(segment)))
            before = time.perf_counter()
            mel = librosa.feature.melspectrogram(
                y=segment,
                sr=RATE,
                n_fft=1024,
                hop_length=480,
                win_length=1024,
                window="hann",
                center=True,
                pad_mode="reflect",
                power=2.0,
                n_mels=64,
                fmin=50,
                fmax=14_000,
                htk=False,
                norm="slaney",
            )
            mel = librosa.power_to_db(mel, ref=1.0, amin=1e-10, top_db=None)
            mel = mel.T[None, None].astype(np.float32)
            mel_seconds += time.perf_counter() - before
            before = time.perf_counter()
            embeddings.append(self.audio.run(None, {"input_features": mel})[0][0])
            inference_seconds += time.perf_counter() - before
        vector = normalize(np.mean(embeddings, axis=0))
        return vector, len(starts), {
            "decode_seconds": decode_seconds,
            "mel_seconds": mel_seconds,
            "inference_seconds": inference_seconds,
        }


def model_for(
    candidate: str,
    model_dir: Path,
    text_model: str | None,
    *,
    load_audio: bool,
    load_text: bool,
) -> tuple[str, Dclap | LaionClap]:
    if candidate == "dclap":
        return (
            candidate_system(candidate),
            Dclap(
                model_dir,
                text_model or "clap_text_model.onnx",
                load_audio=load_audio,
                load_text=load_text,
            ),
        )
    return (
        candidate_system(candidate),
        LaionClap(
            model_dir,
            text_model,
            load_audio=load_audio,
            load_text=load_text,
        ),
    )


def command_verify(args: argparse.Namespace) -> None:
    manifest = verify_artifacts(args.model_dir, artifact_manifest(args.candidate, args.artifacts))
    total = sum(item["bytes"] for item in manifest["files"])
    print(f"verified {len(manifest['files'])} files ({total / 1_000_000:.1f} MB)")
    print(manifest["license_status"])


def command_index(args: argparse.Namespace) -> None:
    verify_artifacts(args.model_dir, artifact_manifest(args.candidate, args.artifacts))
    corpus = read_json(args.corpus)
    system, model = model_for(
        args.candidate,
        args.model_dir,
        args.text_model,
        load_audio=True,
        load_text=False,
    )
    tracks = []
    totals = {"decode_seconds": 0.0, "mel_seconds": 0.0, "inference_seconds": 0.0}
    total_windows = 0
    started = time.perf_counter()
    for number, track in enumerate(corpus["tracks"], 1):
        path = Path(track["path"])
        vector, windows, timing = model.embed_audio(path, args.maximum_windows)
        for key, value in timing.items():
            totals[key] += value
        total_windows += windows
        tracks.append({**track, "embedding": vector.tolist(), "windows": windows})
        print(f"{number}/{len(corpus['tracks'])} {track['id']} · {windows} windows", flush=True)
    elapsed = time.perf_counter() - started
    output = {
        "schema": SCHEMA,
        "system": system,
        "corpus_fingerprint": corpus_fingerprint(corpus["tracks"]),
        "window_policy": "full-overlap" if args.maximum_windows is None else f"even-{args.maximum_windows}",
        "audio_artifact_sha256": sha256(
            args.model_dir
            / ("model_epoch_36.onnx.data" if args.candidate == "dclap" else "audio_model_quantized.onnx")
        ),
        "metrics": {**totals, "elapsed_seconds": elapsed, "tracks": len(tracks), "windows": total_windows},
        "tracks": tracks,
    }
    write_json(args.output, output)


def dot(left: list[float], right: list[float]) -> float:
    return sum(a * b for a, b in zip(left, right, strict=True))


def interpolate(phases: list[tuple[float, list[float]]], position: float) -> list[float]:
    if position <= phases[0][0]:
        return phases[0][1]
    for (left_at, left), (right_at, right) in zip(phases, phases[1:], strict=False):
        if position <= right_at:
            amount = (position - left_at) / max(right_at - left_at, 1e-9)
            return normalize([a + amount * (b - a) for a, b in zip(left, right, strict=True)]).tolist()
    return phases[-1][1]


def sequence(
    tracks: list[dict[str, Any]],
    targets: list[list[float]],
    avoid: list[float] | None,
) -> list[str]:
    remaining = list(range(len(tracks)))
    chosen: list[int] = []
    artists: dict[str, int] = {}
    albums: set[str] = set()
    for target in targets:
        eligible = [
            index
            for index in remaining
            if artists.get(tracks[index].get("artist", ""), 0) < 2
            and (not chosen or tracks[index].get("artist") != tracks[chosen[-1]].get("artist"))
        ]
        fresh = [index for index in eligible if tracks[index].get("album", tracks[index]["id"]) not in albums]
        if fresh:
            eligible = fresh
        if not eligible:
            break
        previous = tracks[chosen[-1]]["embedding"] if chosen else None

        def cost(index: int) -> tuple[float, int]:
            vector = tracks[index]["embedding"]
            relevance = dot(vector, target) - (0.35 * dot(vector, avoid) if avoid else 0.0)
            continuity = dot(vector, previous) if previous else 0.0
            return (-(0.85 * relevance + 0.15 * continuity), index)

        selected = min(eligible, key=cost)
        remaining.remove(selected)
        chosen.append(selected)
        artist = tracks[selected].get("artist", "")
        artists[artist] = artists.get(artist, 0) + 1
        albums.add(tracks[selected].get("album", tracks[selected]["id"]))
    return [tracks[index]["id"] for index in chosen]


def command_query(args: argparse.Namespace) -> None:
    verify_artifacts(args.model_dir, artifact_manifest(args.candidate, args.artifacts))
    index = read_json(args.index)
    require_candidate_index(index, args.candidate, args.index)
    requests = read_json(args.requests)["requests"]
    _, model = model_for(
        args.candidate,
        args.model_dir,
        args.text_model,
        load_audio=False,
        load_text=True,
    )
    all_text = []
    for request in requests:
        all_text.append(request["query"])
        if request.get("avoid"):
            all_text.append(request["avoid"])
        all_text.extend(phase["query"] for phase in request.get("arc", []))
    started = time.perf_counter()
    vectors = model.embed_texts(all_text)
    text_vectors = {text: vector.tolist() for text, vector in zip(all_text, vectors, strict=True)}
    results = []
    count = min(args.limit, len(index["tracks"]))
    for request in requests:
        if request.get("arc"):
            phases = [(float(phase["at"]), text_vectors[phase["query"]]) for phase in request["arc"]]
            targets = [interpolate(phases, slot / max(count - 1, 1)) for slot in range(count)]
        else:
            targets = [text_vectors[request["query"]]] * count
        ranking = sequence(
            index["tracks"],
            targets,
            text_vectors.get(request.get("avoid", "")),
        )
        results.append({"id": request["id"], "kind": request["kind"], "request": request, "ranking": ranking})
    text_model = args.text_model or (
        "clap_text_model.onnx" if args.candidate == "dclap" else "text_model_quantized.onnx"
    )
    output = {
        "schema": SCHEMA,
        "system": f"{index['system']}+{Path(text_model).stem}",
        "corpus_ids": [track["id"] for track in index["tracks"]],
        "corpus_fingerprint": index["corpus_fingerprint"],
        "index": str(args.index),
        "text_artifact_sha256": sha256(
            args.model_dir
            / text_model
        ),
        "query_seconds": time.perf_counter() - started,
        "results": results,
    }
    write_json(args.output, output)


def words(value: str) -> set[str]:
    return {word.casefold() for word in re.findall(r"[^\W_]+", value, flags=re.UNICODE) if len(word) > 1}


def metadata_ranking(tracks: list[dict[str, Any]], request: dict[str, Any], limit: int) -> list[str]:
    wanted = words(request["query"])
    avoided = words(request.get("avoid", ""))
    scored = []
    for index, track in enumerate(tracks):
        fields = [track.get(name, "") for name in ("title", "artist", "album", "genre")]
        fields.extend(track.get("tags", []))
        held = words(" ".join(fields))
        score = len(wanted & held) - len(avoided & held)
        scored.append((index, score))
    scored.sort(key=lambda item: (-item[1], item[0]))
    remaining = [index for index, _ in scored]
    chosen: list[int] = []
    artist_counts: dict[str, int] = {}
    albums: set[str] = set()
    while remaining and len(chosen) < limit:
        eligible = [
            index
            for index in remaining
            if artist_counts.get(tracks[index].get("artist", ""), 0) < 2
            and (not chosen or tracks[index].get("artist") != tracks[chosen[-1]].get("artist"))
        ]
        fresh = [index for index in eligible if tracks[index].get("album", tracks[index]["id"]) not in albums]
        if fresh:
            eligible = fresh
        if not eligible:
            break
        selected = eligible[0]
        remaining.remove(selected)
        chosen.append(selected)
        artist = tracks[selected].get("artist", "")
        artist_counts[artist] = artist_counts.get(artist, 0) + 1
        albums.add(tracks[selected].get("album", tracks[selected]["id"]))
    return [tracks[index]["id"] for index in chosen]


def command_metadata(args: argparse.Namespace) -> None:
    corpus = read_json(args.corpus)
    requests = read_json(args.requests)["requests"]
    results = [
        {
            "id": request["id"],
            "kind": request["kind"],
            "request": request,
            "ranking": metadata_ranking(corpus["tracks"], request, min(args.limit, len(corpus["tracks"]))),
        }
        for request in requests
    ]
    write_json(
        args.output,
        {
            "schema": SCHEMA,
            "system": "metadata-token-overlap-v1",
            "corpus_ids": [track["id"] for track in corpus["tracks"]],
            "corpus_fingerprint": corpus_fingerprint(corpus["tracks"]),
            "results": results,
        },
    )


def make_blind(runs: list[dict[str, Any]], seed: int, limit: int) -> tuple[dict[str, Any], dict[str, Any]]:
    if len(runs) < 2:
        raise ValueError("blind evaluation needs at least two systems")
    corpus = set(runs[0].get("corpus_ids", []))
    if not corpus or any(set(run.get("corpus_ids", [])) != corpus for run in runs[1:]):
        raise ValueError("runs do not identify the same non-empty corpus")
    fingerprint = runs[0].get("corpus_fingerprint")
    if not fingerprint or any(run.get("corpus_fingerprint") != fingerprint for run in runs[1:]):
        raise ValueError("runs were not produced from the same exact corpus")
    by_run = [{item["id"]: item for item in run["results"]} for run in runs]
    request_ids = list(by_run[0])
    if any(set(items) != set(request_ids) for items in by_run[1:]):
        raise ValueError("runs do not contain the same request ids")
    rng = random.Random(seed)
    ballot_items = []
    key_items = []
    for request_id in request_ids:
        order = list(range(len(runs)))
        rng.shuffle(order)
        candidates = []
        mappings = []
        for position, run_index in enumerate(order):
            code = chr(ord("A") + position)
            result = by_run[run_index][request_id]
            candidates.append({
                "code": code,
                "ranking": result["ranking"][:limit],
                "ratings": {name: None for name in ["relevance", "coherence", "transitions", "diversity", "rediscovery", "replay"]},
            })
            mappings.append({"code": code, "system": runs[run_index]["system"]})
        exemplar = by_run[0][request_id]
        ballot_items.append({"id": request_id, "request": exemplar["request"], "candidates": candidates, "preferred": None, "notes": ""})
        key_items.append({"id": request_id, "mapping": mappings})
    return ({"schema": SCHEMA, "seed": seed, "items": ballot_items}, {"schema": SCHEMA, "items": key_items})


def command_blind(args: argparse.Namespace) -> None:
    runs = [read_json(path) for path in args.runs]
    ballot, key = make_blind(runs, args.seed, args.limit)
    write_json(args.ballot, ballot)
    write_json(args.key, key)


def score_ballot(ballot: dict[str, Any], key: dict[str, Any]) -> dict[str, Any]:
    mappings = {item["id"]: {entry["code"]: entry["system"] for entry in item["mapping"]} for item in key["items"]}
    values: dict[str, dict[str, list[float]]] = {}
    wins: dict[str, int] = {}
    for item in ballot["items"]:
        mapping = mappings[item["id"]]
        if item.get("preferred"):
            system = mapping[item["preferred"]]
            wins[system] = wins.get(system, 0) + 1
        for candidate in item["candidates"]:
            system = mapping[candidate["code"]]
            bucket = values.setdefault(system, {})
            for dimension, rating in candidate["ratings"].items():
                if rating is not None:
                    numeric = float(rating)
                    if not 1.0 <= numeric <= 5.0:
                        raise ValueError(f"{item['id']} {candidate['code']} {dimension}: rating must be 1–5")
                    bucket.setdefault(dimension, []).append(numeric)
    systems = {}
    for system, dimensions in values.items():
        means = {name: statistics.fmean(ratings) for name, ratings in dimensions.items() if ratings}
        systems[system] = {"means": means, "mean_all": statistics.fmean(means.values()) if means else None, "preferred": wins.get(system, 0)}
    return {"schema": SCHEMA, "systems": systems}


def command_score(args: argparse.Namespace) -> None:
    write_json(args.output, score_ballot(read_json(args.ballot), read_json(args.key)))


def materialize_ballot(ballot: dict[str, Any], corpus: dict[str, Any], output: Path) -> int:
    """Write identity-free M3U8 candidates which Baz can open for listening."""
    if output.exists():
        raise FileExistsError(f"{output}: refusing to replace an existing listening set")
    tracks = {track["id"]: track for track in corpus["tracks"]}
    output.mkdir(parents=True)
    listening_items = []
    written = 0
    for number, item in enumerate(ballot["items"], 1):
        request_dir = output / f"{number:02d}-{item['id']}"
        request_dir.mkdir()
        candidates = []
        for candidate in item["candidates"]:
            code = candidate["code"]
            path = request_dir / f"{code}.m3u8"
            lines = ["#EXTM3U"]
            for track_id in candidate["ranking"]:
                if track_id not in tracks:
                    raise ValueError(f"{item['id']} {code}: unknown track {track_id}")
                source = tracks[track_id]["path"]
                if "\n" in source or "\r" in source:
                    raise ValueError(f"{track_id}: path contains a playlist line break")
                lines.extend((f"#EXTINF:-1,{track_id}", source))
            path.write_text("\n".join(lines) + "\n", encoding="utf-8")
            candidates.append({"code": code, "playlist": str(path.relative_to(output))})
            written += 1
        listening_items.append(
            {
                "id": item["id"],
                "request": item["request"],
                "candidates": candidates,
            }
        )
    write_json(
        output / "listening.json",
        {"schema": SCHEMA, "items": listening_items},
    )
    return written


def command_materialize(args: argparse.Namespace) -> None:
    count = materialize_ballot(read_json(args.ballot), read_json(args.corpus), args.output)
    print(f"wrote {count} blind playlists under {args.output}")


def parser() -> argparse.ArgumentParser:
    root = Path(__file__).resolve().parent
    result = argparse.ArgumentParser(description=__doc__)
    subcommands = result.add_subparsers(required=True)
    verify = subcommands.add_parser("verify", help="verify local model artifacts; never download")
    verify.add_argument("model_dir", type=Path)
    verify.add_argument("--candidate", choices=["dclap", "laion"], default="dclap")
    verify.add_argument("--artifacts", type=Path)
    verify.set_defaults(function=command_verify)
    index = subcommands.add_parser("index", help="build a local DCLAP evaluation index")
    index.add_argument("corpus", type=Path)
    index.add_argument("model_dir", type=Path)
    index.add_argument("output", type=Path)
    index.add_argument("--maximum-windows", type=int)
    index.add_argument("--candidate", choices=["dclap", "laion"], default="dclap")
    index.add_argument("--text-model")
    index.add_argument("--artifacts", type=Path)
    index.set_defaults(function=command_index)
    query = subcommands.add_parser("query", help="create a semantic ranking run")
    query.add_argument("index", type=Path)
    query.add_argument("model_dir", type=Path)
    query.add_argument("output", type=Path)
    query.add_argument("--requests", type=Path, default=root / "requests.json")
    query.add_argument("--limit", type=int, default=20)
    query.add_argument("--candidate", choices=["dclap", "laion"], default="dclap")
    query.add_argument("--text-model")
    query.add_argument("--artifacts", type=Path)
    query.set_defaults(function=command_query)
    metadata = subcommands.add_parser("metadata", help="create the deliberately simple metadata control run")
    metadata.add_argument("corpus", type=Path)
    metadata.add_argument("output", type=Path)
    metadata.add_argument("--requests", type=Path, default=root / "requests.json")
    metadata.add_argument("--limit", type=int, default=20)
    metadata.set_defaults(function=command_metadata)
    blind = subcommands.add_parser("blind", help="randomize system identities into a ballot and separate key")
    blind.add_argument("runs", type=Path, nargs="+")
    blind.add_argument("--ballot", type=Path, required=True)
    blind.add_argument("--key", type=Path, required=True)
    blind.add_argument("--seed", type=int, default=20260813)
    blind.add_argument("--limit", type=int, default=20)
    blind.set_defaults(function=command_blind)
    score = subcommands.add_parser("score", help="score a completed blind ballot")
    score.add_argument("ballot", type=Path)
    score.add_argument("key", type=Path)
    score.add_argument("output", type=Path)
    score.set_defaults(function=command_score)
    materialize = subcommands.add_parser(
        "materialize",
        help="write identity-free M3U8 candidates for a blind ballot",
    )
    materialize.add_argument("ballot", type=Path)
    materialize.add_argument("corpus", type=Path)
    materialize.add_argument("output", type=Path)
    materialize.set_defaults(function=command_materialize)
    return result


def main() -> None:
    args = parser().parse_args()
    args.function(args)


if __name__ == "__main__":
    main()
