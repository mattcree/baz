# CLAP HTSAT unfused

Source: `laion/clap-htsat-unfused`, revision
`8fa0f1c6d0433df6e97c127f64b2a1d6c0dcda8a`.

CLAP learns a joint representation of audio and natural language. Baz uses
the checkpoint only for local music retrieval: it embeds bounded windows from
the listener's own tracks and compares them with a listener-entered musical
description. The application does not upload audio, prompts, embeddings, or
results.

The upstream checkpoint is released under Apache License 2.0. Its intended
research task is audio-text retrieval; outputs are similarity estimates, not
objective labels or facts about a person. Rankings can reflect limitations in
the checkpoint's training data. Baz therefore presents the result as an
editable playlist preview and never uses it to infer listener identity or
sensitive attributes.

Upstream model card:
https://huggingface.co/laion/clap-htsat-unfused/tree/8fa0f1c6d0433df6e97c127f64b2a1d6c0dcda8a
