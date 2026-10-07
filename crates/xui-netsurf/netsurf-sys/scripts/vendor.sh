#!/bin/sh
# Refreshes vendor/ from the upstream commits in scripts/revisions.
#
# vendor/ holds only what the crate builds from: each library's include/ and
# src/ (and libdom's bindings/), NetSurf's content/, desktop/, include/ and
# utils/ without the JavaScript engine, the four built-in style sheets, and
# every licence. The upstream test suites are left out on purpose: their
# fuzzer-named fixtures (`id:000023,...`) cannot be checked out on Windows.
# Then the changes nsx makes to NetSurf, patches/*.patch, are applied in
# order (opacity, inline generated text, inline spaces, hidden elements).
# Run regen.sh after moving a pin.
set -eu

. "$(dirname "$0")/upstream.sh"
vendor=$here/vendor

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# copy <from> <to> <path>...: copies the listed paths that exist.
copy() {
	from=$1 to=$2
	shift 2
	for path in "$@"; do
		if [ -e "$from/$path" ]; then
			mkdir -p "$to/$(dirname "$path")"
			cp -R "$from/$path" "$to/$path"
		fi
	done
}

rm -rf "$vendor"
pins | while read -r repo rev; do
	fetch "$repo" "$rev" "$work/$repo"
	if [ "$repo" = netsurf ]; then
		copy "$work/$repo" "$vendor/$repo" COPYING README.md \
			content desktop include utils \
			resources/default.css resources/quirks.css \
			resources/internal.css resources/adblock.css
		# JavaScript is built as `none`; duktape alone is 4 MB.
		rm -rf "$vendor/$repo/content/handlers/javascript/duktape" \
			"$vendor/$repo/content/handlers/javascript/WebIDL"
	else
		copy "$work/$repo" "$vendor/$repo" COPYING README README.md \
			include src bindings
	fi
	find "$vendor/$repo" -name Makefile -delete
done

for p in "$here"/patches/*.patch; do
	[ -e "$p" ] || continue
	patch -d "$vendor" -p1 --forward --quiet < "$p"
done

echo "vendored into $vendor"
