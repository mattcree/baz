#!/bin/sh
# **Put baz where a desktop can find it** — the five copies `docs/INSTALL.md`
# used to ask a reader to make by hand.
#
# The owner, 2026-08-18: *"can we get it set up to be installed not just built
# — as in when it shows up it has a proper icon set etc."* Every piece needed
# for that already travelled in the Linux archive; what was missing was the
# step that puts them in the four places a desktop actually looks. A download
# that ships a menu entry, an icon ladder and an AppStream file, and then
# hands you a list of `install -Dm644` lines, has not been installed.
#
# # Where it puts things, and why there is a choice
#
# Per-user by default, into `~/.local`, which needs no privilege and is on the
# XDG search path of every desktop that has shipped this decade. `--system`
# writes to `/usr/local` for a shared machine, and is the only mode that wants
# `sudo`. `--prefix` takes anywhere else.
#
# # What it deliberately does not do
#
# **No package manager, and no pretending to be one.** This is a tarball; the
# packaged routes are Flatpak (`docs/INSTALL.md`) and, one day, a distribution
# package. So it keeps a manifest of exactly what it wrote and
# `uninstall.sh` removes that and nothing else — an installer that cannot be
# reversed is a worse bargain than copying files by hand, because at least you
# remember where you put those.
#
# **No `PATH` editing.** Changing a login shell's profile from an installer is
# a surprise a listener finds months later in a file they did not write. If
# `~/.local/bin` is not on `PATH` this says so, plainly, and prints the one
# line that fixes it.
#
# POSIX `sh`, because the one machine that most needs this to work is a fresh
# one, and `bash` is not a given on every desktop image.
set -eu

app_id='io.github.mattcree.baz'
mode='user'
prefix=''

usage() {
    cat <<EOF
Install baz for the current user (default) or for everyone.

  ./install.sh                 into ~/.local
  ./install.sh --system        into /usr/local   (needs sudo)
  ./install.sh --prefix DIR    into DIR
  ./install.sh --help

Removes cleanly with ./uninstall.sh, passing the same flags.
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --system) mode='system' ;;
        --prefix)
            [ $# -ge 2 ] || { echo "install.sh: --prefix needs a directory" >&2; exit 2; }
            mode='prefix'
            prefix="$2"
            shift
            ;;
        --help | -h) usage; exit 0 ;;
        *) echo "install.sh: unknown option $1" >&2; usage >&2; exit 2 ;;
    esac
    shift
done

case "$mode" in
    user)
        prefix="${XDG_DATA_HOME:-$HOME/.local/share}"
        bindir="$HOME/.local/bin"
        datadir="$prefix"
        ;;
    system)
        bindir='/usr/local/bin'
        datadir='/usr/local/share'
        ;;
    prefix)
        bindir="$prefix/bin"
        datadir="$prefix/share"
        ;;
esac

here=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
[ -f "$here/baz" ] || {
    echo "install.sh: no baz binary beside this script (expected $here/baz)" >&2
    exit 1
}

# The manifest lives beside the desktop entry so an uninstall can find it
# without being told where the install went.
manifest="$datadir/$app_id/installed-files"
: > "/tmp/baz-install.$$"
record() { printf '%s\n' "$1" >> "/tmp/baz-install.$$"; }

place() { # source, destination, mode
    mkdir -p "$(dirname -- "$2")"
    cp -f -- "$1" "$2"
    chmod "$3" "$2"
    record "$2"
}

place "$here/baz" "$bindir/baz" 755
[ -f "$here/$app_id.desktop" ] &&
    place "$here/$app_id.desktop" "$datadir/applications/$app_id.desktop" 644
[ -f "$here/$app_id.metainfo.xml" ] &&
    place "$here/$app_id.metainfo.xml" "$datadir/metainfo/$app_id.metainfo.xml" 644

# **The icon ladder, copied as a tree.** The archive already carries it in the
# hicolor layout it is installed in, so this is one walk rather than eight
# named files — and a size added to `packaging/icons/` ships without this
# script learning about it.
if [ -d "$here/icons" ]; then
    find "$here/icons" -type f | while IFS= read -r icon; do
        relative=${icon#"$here/icons/"}
        place "$icon" "$datadir/icons/hicolor/$relative" 644
    done
fi

# **`Exec=` has to name the binary that was actually installed.** The entry
# ships as a bare `Exec=baz`, which is right for a system install and a lie
# for a per-user one on a machine where `~/.local/bin` is not on the desktop's
# own `PATH` — and a desktop's `PATH` is not the shell's. An absolute path is
# true in every case.
entry="$datadir/applications/$app_id.desktop"
if [ -f "$entry" ]; then
    tmp="$entry.tmp.$$"
    sed "s|^Exec=baz$|Exec=$bindir/baz|" "$entry" > "$tmp"
    mv -f "$tmp" "$entry"
fi

mkdir -p "$(dirname -- "$manifest")"
cp -f "/tmp/baz-install.$$" "$manifest"
printf '%s\n' "$manifest" >> "$manifest"
rm -f "/tmp/baz-install.$$"

# Caches, both optional. The icon resolves without either; these only make the
# lookup faster and the menu entry appear without a re-login.
command -v gtk-update-icon-cache >/dev/null 2>&1 &&
    gtk-update-icon-cache -f -t "$datadir/icons/hicolor" >/dev/null 2>&1 || true
command -v update-desktop-database >/dev/null 2>&1 &&
    update-desktop-database "$datadir/applications" >/dev/null 2>&1 || true

echo "baz installed:"
echo "  binary     $bindir/baz"
echo "  menu entry $datadir/applications/$app_id.desktop"
echo "  icons      $datadir/icons/hicolor/…"

# Said rather than fixed — see the note on `PATH` above.
case ":${PATH}:" in
    *":$bindir:"*) ;;
    *)
        echo
        echo "note: $bindir is not on your PATH, so \`baz\` will not run from a"
        echo "      terminal. The menu entry works either way. To fix it:"
        echo
        echo "        echo 'export PATH=\"$bindir:\$PATH\"' >> ~/.profile"
        ;;
esac
