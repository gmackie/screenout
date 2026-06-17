#!/bin/sh
set -eu

echo "screenout demo process"
echo "pid: $$"
echo "Press Ctrl+Z, then run: screenout --ssh <host-or-ssh-alias>"

count=1
while :; do
    printf 'tick %s: %s\n' "$count" "$(date)"
    count=$((count + 1))
    sleep 1
done
