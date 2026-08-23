#!/bin/sh
# **A Flatpak repository on this machine, so the published one is not the
# first one anybody tries** — [ADR-0045](../../docs/adr/0045-shipping-the-flatpak.md) §1.
#
# The saga this exists to prevent: baz shipped a single-file `.flatpak` bundle
# for five releases. A bundle install leaves a stub remote with no repository
# behind it, so `flatpak update` answers *Nothing to update* for ever — and the
# release notes promised a software centre would keep it current. Nobody
# noticed because nobody had ever run the update path.
#
# So this runs it. Everything the CI job will do — export, sign, deltas, prune,
# serve, install from a `.flatpakref`, update — against `localhost`, where a
# mistake costs a rerun instead of a published release.
#
# # What it does not do
#
# **It is not the publishing job.** It uses a throwaway key in its own keyring
# and serves over plain HTTP on the loopback, neither of which is acceptable
# for the real thing. What it proves is the *sequence*, which is the part that
# was wrong.
#
# # Use
#
#     packaging/flatpak/test-repo.sh build     # build + export + sign + serve
#     packaging/flatpak/test-repo.sh install   # install from the .flatpakref
#     packaging/flatpak/test-repo.sh update    # rebuild, re-export, update
#     packaging/flatpak/test-repo.sh clean     # remove app, remote, repo, key
#
# POSIX `sh`, like `install.sh` beside it, and for the same reason.
set -eu

id='io.github.mattcree.baz'
root="${BAZ_TEST_REPO_ROOT:-${XDG_CACHE_HOME:-$HOME/.cache}/baz-test-repo}"
repo="$root/repo"
keyring="$root/gnupg"
port="${BAZ_TEST_REPO_PORT:-8723}"
ref="$root/baz.flatpakref"
url="http://127.0.0.1:$port/"
manifest="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)/$id.local.yml"

say() { printf '\n\033[1m%s\033[0m\n' "$*"; }

key_id() {
    GNUPGHOME="$keyring" gpg --list-keys --with-colons 2>/dev/null |
        awk -F: '/^fpr:/ { print $10; exit }'
}

ensure_key() {
    [ -n "$(key_id)" ] && return 0
    say 'Making a throwaway signing key'
    mkdir -p "$keyring"
    chmod 700 "$keyring"
    # **No passphrase, and no prompt.** `--batch` alone is not enough: gpg still
    # raises pinentry for the protection passphrase, which on 2026-08-23 put a
    # dialog on the owner's desktop from a script he had not run himself. A test
    # key that guards nothing must not ask anybody for anything, and a harness
    # that cannot run unattended is not a harness. `--passphrase ''` with
    # loopback pinentry is what actually suppresses it.
    #
    # This key is worthless on purpose: it signs a repository served to
    # `127.0.0.1`, it lives in a keyring of its own under the cache, and
    # `clean` deletes it. The published repository's key is a different key,
    # kept somewhere else, and is not made here.
    GNUPGHOME="$keyring" gpg --batch --yes \
        --passphrase '' --pinentry-mode loopback \
        --quick-generate-key 'baz test repository <baz@localhost>' \
        default default never
}

serve() {
    if [ -f "$root/httpd.pid" ] && kill -0 "$(cat "$root/httpd.pid")" 2>/dev/null; then
        return 0
    fi
    say "Serving $repo at $url"
    ( cd "$repo" && exec python3 -m http.server "$port" --bind 127.0.0.1 ) \
        >"$root/httpd.log" 2>&1 &
    echo $! > "$root/httpd.pid"
    sleep 1
}

export_repo() {
    ensure_key
    fpr="$(key_id)"
    mkdir -p "$repo"
    say 'Building and exporting'
    flatpak-builder --user --force-clean --disable-rofiles-fuse \
        --repo="$repo" --gpg-sign="$fpr" --gpg-homedir="$keyring" \
        "$root/build" "$manifest"
    # **Deltas and pruning are not optional** (ADR-0045 §1): without deltas a
    # client refetches whole objects, and without pruning the store grows past
    # what a static host will hold.
    say 'Generating static deltas and pruning to one version'
    flatpak build-update-repo --generate-static-deltas \
        --prune --prune-depth=1 \
        --gpg-sign="$fpr" --gpg-homedir="$keyring" "$repo"
    say 'Writing the .flatpakref'
    {
        printf '[Flatpak Ref]\n'
        printf 'Name=%s\n' "$id"
        printf 'Branch=master\n'
        printf 'Url=%s\n' "$url"
        printf 'IsRuntime=false\n'
        printf 'GPGKey=%s\n' \
            "$(GNUPGHOME="$keyring" gpg --export "$fpr" | base64 -w0)"
        printf 'RuntimeRepo=https://dl.flathub.org/repo/flathub.flatpakrepo\n'
    } > "$ref"
    printf '  %s\n' "$ref"
    du -sh "$repo"
}

case "${1:-build}" in
    build)   export_repo; serve ;;
    install) serve; flatpak install --user -y --from "$ref" ;;
    update)  export_repo; serve; flatpak update --user -y "$id" ;;
    serve)   serve ;;
    clean)
        [ -f "$root/httpd.pid" ] && kill "$(cat "$root/httpd.pid")" 2>/dev/null || true
        flatpak uninstall --user -y "$id" 2>/dev/null || true
        flatpak remote-delete --user "$id-origin" 2>/dev/null || true
        rm -rf "$root"
        say 'Cleaned' ;;
    *) printf 'usage: %s build|install|update|serve|clean\n' "$0" >&2; exit 2 ;;
esac
