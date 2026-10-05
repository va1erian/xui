# Sourced by vendor.sh and regen.sh: clones the pinned upstream repositories.

here=$(cd "$(dirname "$0")/.." && pwd)

# Prints `<repository> <commit>` for every pin in scripts/revisions.
pins() {
	grep -v '^#' "$here/scripts/revisions" | grep -v '^$'
}

# fetch <repository> <commit> <dest>: a shallow checkout of one commit.
fetch() {
	git init -q "$3"
	git -C "$3" fetch -q --depth 1 "https://github.com/netsurf-browser/$1.git" "$2"
	git -C "$3" checkout -q FETCH_HEAD
}
