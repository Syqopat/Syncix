#!/bin/sh
# Luau syntax check.
#
# The luau CLI has no "compile only" flag, so each file is executed. The Roblox API
# (game, Instance, plugin) does not exist here, so every file fails at runtime;
# that is EXPECTED and ignored.
#
# How the two are told apart: luau prints both kinds of error as
# "file:line: message" + "stacktrace:". The difference: for a COMPILE (syntax)
# error the stack trace is EMPTY, because no code ran. A runtime error has at
# least one frame.
#
# Usage:  sh tools/luau-check.sh <file or folder> ...
#         sh tools/luau-check.sh packages/studio-plugin/src examples
#
# A folder is walked here rather than with $(find ...): the example place has a folder
# with a space in its name ("Candy Blossom"), and an unquoted find expansion split that
# into two paths that do not exist.

FAILED_FLAG=$(mktemp)

check_one() {
	f="$1"
	OUTPUT=$(luau "$f" 2>&1)

	if [ -z "$OUTPUT" ]; then
		echo "OK                $f"
		return
	fi

	# Number of non-empty lines AFTER the "stacktrace:" line
	FRAMES=$(echo "$OUTPUT" | sed -n '/^stacktrace:/,$p' | tail -n +2 | grep -c '[^[:space:]]')

	if [ "$FRAMES" -eq 0 ]; then
		echo "SYNTAX ERROR      $f"
		echo "$OUTPUT" | head -2 | sed 's/^/    /'
		echo 1 > "$FAILED_FLAG"
	else
		echo "OK                $f"
	fi
}

for target in "$@"; do
	if [ -d "$target" ]; then
		find "$target" -name "*.lua" -print0 | while IFS= read -r -d '' file; do
			check_one "$file"
		done
	else
		check_one "$target"
	fi
done

if [ -s "$FAILED_FLAG" ]; then
	rm -f "$FAILED_FLAG"
	echo "RESULT: syntax errors found"
	exit 1
fi

rm -f "$FAILED_FLAG"
echo "RESULT: all files clean"
