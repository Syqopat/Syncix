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
# Usage:  sh tools/luau-check.sh file1.lua file2.lua ...
#         sh tools/luau-check.sh $(find studio-plugin/src -name "*.lua")

FAILED=0

for f in "$@"; do
	OUTPUT=$(luau "$f" 2>&1)

	if [ -z "$OUTPUT" ]; then
		echo "OK                $f"
		continue
	fi

	# Number of non-empty lines AFTER the "stacktrace:" line
	FRAMES=$(echo "$OUTPUT" | sed -n '/^stacktrace:/,$p' | tail -n +2 | grep -c '[^[:space:]]')

	if [ "$FRAMES" -eq 0 ]; then
		echo "SYNTAX ERROR      $f"
		echo "$OUTPUT" | head -2 | sed 's/^/    /'
		FAILED=1
	else
		echo "OK                $f"
	fi
done

if [ "$FAILED" -ne 0 ]; then
	echo "RESULT: syntax errors found"
else
	echo "RESULT: all files clean"
fi

exit $FAILED
