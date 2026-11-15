#!/bin/bash
# The CORE component's postinstall — brain-server, brain, mcp (+ the
# detection core's own binary, which stays in the anchor unlinked: it is the
# preserve law's instrument, not a user tool).
#
# pkgbuild runs this as the invoking user under
# `installer -target CurrentUserHomeDirectory`; $3 is the install destination
# (the payload anchor) and $HOME is the user's home. The laws live in
# lib-postinstall.sh (sourced beside this file by the builder).
set -u

. "$(dirname "$0")/lib-postinstall.sh"

install_modes

# THE METALLIB BUNDLE LAW (round 21): brain-afm staged without
# mlx-swift_Cmlx.bundle beside it is the broken pair — the link is refused
# LOUDLY (the writer tier would die at first MLX use: "Failed to load the
# default metallib", exit 255 — the exact recurrence witnessed 2026-11-15),
# the rest of the payload installs.
AFM_BROKEN=0
if [ -e "$ANCHOR/brain-afm" ] && [ ! -d "$ANCHOR/mlx-swift_Cmlx.bundle" ]; then
    AFM_BROKEN=1
fi

# Every payload binary gets its ~/.local/bin link by the preserve law — the
# payload IS the list (self-describing; no baked names to drift). Directory
# entries (the mlx-swift_Cmlx.bundle the brain-afm pair needs) are not
# linkable tools and are skipped here; the bundle law below places them.
for tool_path in "$ANCHOR"/*; do
    name=$(basename "$tool_path")
    [ "$name" = "stewardmem-detect" ] && continue
    [ -f "$tool_path" ] || continue
    if [ "$name" = "brain-afm" ] && [ "$AFM_BROKEN" = "1" ]; then
        echo "REFUSED: brain-afm staged without mlx-swift_Cmlx.bundle beside it — the writer tier would die at first MLX use (Failed to load the default metallib, exit 255); no brain-afm link was created"
        continue
    fi
    install_tool_link "$name"
done

install_bundle_beside_link

print_hand_off
exit 0
