#!/bin/sh
# Double-click in Finder to install tack.
cd "$(dirname "$0")" || exit 1
./install.sh
status=$?
echo
[ "$status" -eq 0 ] && echo "tack installed." || echo "tack install failed (exit $status)."
printf 'Press Return to close. '
read -r _
exit "$status"
