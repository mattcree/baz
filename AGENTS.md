# Repository instructions

## Development environment

- Run repository build, test, lint, coverage, and application commands inside
  the existing rootless `baz-dev` Toolbox container:

  ```sh
  toolbox run -c baz-dev <command>
  ```

- In particular, launch the application with:

  ```sh
  toolbox run -c baz-dev cargo run --release -p baz [-- MUSIC_DIR]
  ```

- Do not use a failed host-side all-features build as evidence of a code
  failure. The host may intentionally lack ALSA development files, while the
  container provides `alsa-lib-devel` plus the other native test and GUI
  dependencies.
- If `baz-dev` does not exist or its dependencies are stale, run
  `./scripts/toolbox-setup.sh`. Do not install development packages on the host.
- The Toolbox shares the workspace, home directory, graphical session, and
  audio session, so it is suitable for both CI-equivalent commands and
  interactive GUI/audio testing.
- `.devcontainer/` is the editor/dev-container alternative. Keep it aligned
  with `scripts/toolbox-setup.sh`, which is the source of truth for native
  packages.

See `docs/DEVELOPMENT.md` for device tests, headless rendering, diagnostics,
and exclusive-output commands.

## Resuming the backlog after a context reset

The owner's phrase **“let's work through the backlog”** (or an unambiguous
equivalent) is sufficient instruction to begin implementation. Do not ask him
to repeat the live review or choose the next item.

1. Read `docs/WORK.md` completely enough to understand its rules, then use
   **`## Next — authoritative execution order`**. Start the first unfinished
   numbered item. The phases and their order are deliberate: critical usability
   first, Home/vibe next, sustainable releases, then accepted follow-on work.
2. Find the matching detailed brief later in `docs/WORK.md`, the owner's ask in
   `docs/BACKLOG.md`, and every ADR/design the brief names. These documents
   carry the product decisions that conversation context no longer does.
3. Run `git status --short` and inspect relevant diffs before editing. The
   worktree may contain unfinished owner/agent work from the prior session;
   preserve it, continue compatible in-scope work, and never erase or rewrite
   unrelated changes to obtain a clean tree.
4. Mark the numbered item in `## Doing` while it is active. Implement one
   coherent item, including tests and documentation. Use the `baz-dev` Toolbox
   for every repository command as specified above; UI work should also be
   launched and exercised there when the environment permits.
5. On completion, update `docs/WORK.md` and `docs/BACKLOG.md` in the same change:
   remove/mark the numbered item complete, record what actually shipped, and
   leave newly discovered work explicit. Then proceed to the next safe item if
   the request was to work *through* the backlog.

Ordinary implementation details should be resolved from the recorded
constraints and evidence. Ask the owner only when a genuinely unrecorded choice
would materially change product behavior, scope, data safety, or external
state. A queued item does not authorize publishing a release, creating a tag,
submitting to a store, buying signing credentials, or otherwise crossing an
external release boundary; prepare and verify those steps, then request the
specific authority when reached.

When the owner says an idea is only to be recorded, update the backlog/briefs
without implementing it. That is different from “work through the backlog,”
which explicitly starts the ordered implementation workflow above.
