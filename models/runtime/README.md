# Offline build runtime

The Linux x86-64 static ONNX Runtime library in this directory is the exact
CPU distribution selected by `ort-sys` 2.0.0-rc.13 for API 27:

- source: `https://cdn.pyke.io/0/pyke:ort-rs/ms@1.28.0/x86_64-unknown-linux-gnu.tar.lzma2`
- archive SHA-256: `e454f710f8a49f53aa5b4ff51e3454ae1835777e431c6c35c5255ce6f205fd68`

It exists so the Flathub build can remain strictly offline. Direct release
cross-builds use `ort-sys`'s verified target distribution. ONNX Runtime is MIT
licensed; see `LICENSE`.
