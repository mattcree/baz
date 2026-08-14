# Baz local Vibe model

These are Baz's reproducible quantized ONNX exports of the official
`laion/clap-htsat-unfused` checkpoint at revision
`8fa0f1c6d0433df6e97c127f64b2a1d6c0dcda8a`.

The paired audio and text towers place local audio and ordinary-language
requests in the same 512-dimensional space. Baz runs both towers on the
listener's device and makes no network request. The text graph includes the
checkpoint's attention mask. Audio preprocessing is implemented and
numerically pinned in `crates/baz-vibe/src/semantic.rs`.

`tools/vibe-eval/export_laion.py` is the exact export and quantization recipe.
`tools/vibe-eval/artifacts-laion-reproduced.json` records byte sizes and SHA-256
hashes for these files. The upstream model and these derived model artifacts
are licensed under Apache License 2.0; see `LICENSE` in this directory. Baz's
application source remains GPL-3.0-or-later.
