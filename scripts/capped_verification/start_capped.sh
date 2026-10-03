#!/bin/sh
# The installed path unit starts the fixed service without sudo.
set -eu
exec /home/ran/.local/bin/uv --cache-dir /tmp/cathedral-uv-cache \
    run --no-project --offline python -I -B \
    /home/ran/src/rust/cathedralbevy/scripts/capped_verification/control.py start
