#!/bin/sh
# **Take back exactly what `install.sh` put down**, and nothing else.
#
# It reads the manifest that install wrote rather than guessing at a list of
# paths, which is the difference between an uninstaller and a script that
# deletes files matching a pattern in `~/.local`. If the manifest is missing,
# this refuses instead of improvising: an installer that cannot be reversed is
# a worse bargain than copying files by hand, and one that reverses *more*
# than it did is worse still.
#
# It removes the directories it emptied and stops at the first that is not
# empty, so a shared `~/.local/share/applications` survives.
set -eu

app_id='io.github.mattcree.baz'
mode='user'
prefix=''

usage() {
    cat <<EOF
Remove a baz installed by ./install.sh, with the same flags it was given.

  ./uninstall.sh                 from ~/.local
  ./uninstall.sh --system        from /usr/local   (needs sudo)
  ./uninstall.sh --prefix DIR    from DIR
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --system) mode='system' ;;
        --prefix)
            [ $# -ge 2 ] || { echo "uninstall.sh: --prefix needs a directory" >&2; exit 2; }
            mode='prefix'
            prefix="$2"
            shift
            ;;
        --help | -h) usage; exit 0 ;;
        *) echo "uninstall.sh: unknown option $1" >&2; usage >&2; exit 2 ;;
    esac
    shift
done

case "$mode" in
    user) datadir="${XDG_DATA_HOME:-$HOME/.local/share}" ;;
    system) datadir='/usr/local/share' ;;
    prefix) datadir="$prefix/share" ;;
esac

manifest="$datadir/$app_id/installed-files"
[ -f "$manifest" ] || {
    echo "uninstall.sh: no manifest at $manifest" >&2
    echo "  Either baz was not installed there, or it was installed with" >&2
    echo "  different flags. Nothing has been removed." >&2
    exit 1
}

removed=0
while IFS= read -r path; do
    [ -n "$path" ] || continue
    if [ -e "$path" ]; then
        rm -f -- "$path"
        removed=$((removed + 1))
    fi
    # Walk back up, stopping at the first directory somebody else is using.
    dir=$(dirname -- "$path")
    while [ "$dir" != "/" ] && [ "$dir" != "." ]; do
        rmdir -- "$dir" 2>/dev/null || break
        dir=$(dirname -- "$dir")
    done
done < "$manifest"

command -v gtk-update-icon-cache >/dev/null 2>&1 &&
    gtk-update-icon-cache -f -t "$datadir/icons/hicolor" >/dev/null 2>&1 || true
command -v update-desktop-database >/dev/null 2>&1 &&
    update-desktop-database "$datadir/applications" >/dev/null 2>&1 || true

echo "baz removed: $removed files."
echo "Your library, playlists and settings are untouched — they live in"
echo "  ${XDG_DATA_HOME:-$HOME/.local/share}/baz and ${XDG_CONFIG_HOME:-$HOME/.config}/baz"
