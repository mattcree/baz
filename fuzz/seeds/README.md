# Seed corpus

One real input per shape each target parses, so a fuzz run starts from
something rather than from nothing. Read by CI's `fuzz` job as a second
corpus directory (`cargo fuzz run <target> <working corpus> fuzz/seeds/<target>`)
and never written to; the working corpus lives in `fuzz/corpus/`, which is
ignored.

Audio seeds are a 50 ms 440 Hz sine at 8 kHz mono, one file per codec baz
decodes, named without extensions because `.flac` and `.wav` are ignored
repository-wide. Regenerate them from `sine-wav` with ffmpeg; the text seeds
are the documented line formats, copied from the tests that pin them.
