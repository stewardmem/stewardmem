#!/usr/bin/env bash
# THE METALLIB BUNDLE LAW's placement step (round 21, 2026-11-15).
#
#   scripts/stage-metallib-bundle.sh <app-bundle> --staged <binaries-dir> \
#       [--sign <identity|"-" for ad-hoc>] [--entitlements <plist>]
#
# WHAT IT DOES
#   brain-afm loads mlx-swift_Cmlx.bundle from the directory of the INVOKED
#   path; without it the child dies at first MLX use ("Failed to load the
#   default metallib … library not found", exit 255) — the exact failure
#   that killed both serving-kernel writer turns on 2026-11-15. This step
#   copies the STAGED bundle into <app>/Contents/MacOS/ beside the brain-afm
#   sidecar, signs it when asked, and re-seals the parent.
#
# WHY A SCRIPT AND NOT tauri.conf.json's macOS.files (the drill, Posture L,
# 2026-11-15): the bundler's OWN seal refuses an unsigned nested bundle in
# Contents/MacOS — `In subcomponent: mlx-swift_Cmlx.bundle … failed to sign
# app` — and the bundler offers no hook to sign one. The placement must
# happen AFTER the build, with the bundle signed explicitly, and the parent
# re-sealed. The tauri confs deliberately carry NO MacOS files mapping.
#
# THE LAWS
#   - Fail-closed: a brain-afm in the app without a staged bundle is the
#     broken writer tier — refused by name, never shipped around.
#   - The copy is from the STAGED bytes (the same-build-pass law: the
#     SwiftPM build output sidecar.mjs stage staged, never a hand-copy).
#   - Idempotent: an existing bundle in MacOS is replaced from the staged
#     bytes (the pair must move together — a stale bundle against a new
#     binary is a new failure class).
#   - When --sign is given, the bundle is signed BEFORE the parent re-seal
#     (inside-out; the ITMS-90885 law's own order).
set -euo pipefail

APP=""
STAGED=""
SIGN=""
ENTITLEMENTS=""
while [ $# -gt 0 ]; do
    case "$1" in
        --staged) STAGED="$2"; shift 2 ;;
        --sign) SIGN="$2"; shift 2 ;;
        --entitlements) ENTITLEMENTS="$2"; shift 2 ;;
        *) APP="$1"; shift ;;
    esac
done
[ -n "$APP" ] && [ -n "$STAGED" ] || {
    echo "usage: stage-metallib-bundle.sh <app-bundle> --staged <binaries-dir> [--sign <identity>|-] [--entitlements <plist>]" >&2
    exit 2
}

MACOS_DIR="$APP/Contents/MacOS"
BUNDLE_NAME="mlx-swift_Cmlx.bundle"
METALLIB_REL="Contents/Resources/default.metallib"

fail() { echo "error: $1" >&2; exit 1; }

[ -d "$APP" ] || fail "the app bundle does not exist: $APP"
[ -x "$MACOS_DIR/brain-afm" ] || {
    echo "no brain-afm in Contents/MacOS — nothing for the metallib law to place; done"
    exit 0
}
[ -d "$STAGED/$BUNDLE_NAME" ] \
    || fail "the staged dir is missing $BUNDLE_NAME — brain-afm without it dies at first MLX use (exit 255); stage first (node shell/scripts/sidecar.mjs stage)"
[ -f "$STAGED/$BUNDLE_NAME/$METALLIB_REL" ] \
    || fail "the staged $BUNDLE_NAME carries no $METALLIB_REL — the resource bundle without its metallib is the broken shape; refusing"

# The copy: replace whatever is there — the pair moves together.
rm -rf "${MACOS_DIR:?}/$BUNDLE_NAME"
cp -R "$STAGED/$BUNDLE_NAME" "$MACOS_DIR/$BUNDLE_NAME"
[ -f "$MACOS_DIR/$BUNDLE_NAME/$METALLIB_REL" ] \
    || fail "the copied bundle lost its metallib — the copy is broken; refusing"

SIGN_ARGS=(--force)
if [ -n "$SIGN" ]; then
    SIGN_ARGS+=(--sign "$SIGN" --options runtime)
    if [ -n "$ENTITLEMENTS" ]; then
        SIGN_ARGS+=(--entitlements "$ENTITLEMENTS")
    fi
    if [ "$SIGN" != "-" ]; then
        SIGN_ARGS+=(--timestamp)
    fi
    codesign "${SIGN_ARGS[@]}" "$MACOS_DIR/$BUNDLE_NAME" \
        || fail "$BUNDLE_NAME could not be signed — the parent's seal would refuse it"
    # The re-seal applies the hardened-runtime flag UNCONDITIONALLY — for a
    # real identity AND for ad-hoc. The runtime bit is the one flag the
    # release walker demands on EVERY Mach-O (sidecar.mjs: `runtime is
    # pinned`; the refusal text names linker-signed shapes), and the
    # bundler's own seal CARRIES it on both lanes (the public ad-hoc build
    # is "ad-hoc, hardened runtime" by the workflow's own step name) — so
    # a re-seal without `--options runtime` STRIPS the flag the build just
    # set, on either lane. The first public run after the conditional's
    # removal proved it live: the ad-hoc branch re-sealed brain-shell to
    # flags=0x2(adhoc) and the walker refused the bundle (v1.29.46's first
    # lane run, 2026-11-18). A secure TIMESTAMP, by contrast, requires a
    # real identity, so it stays gated. (The previous form's own comment —
    # "the public proof lanes walk for shape, not runtime" — was measured
    # false by that run: the walk is unconditional.)
    RESEAL_ARGS=(--force --sign "$SIGN" --options runtime)
    if [ "$SIGN" != "-" ]; then
        RESEAL_ARGS+=(--timestamp)
    fi
    codesign "${RESEAL_ARGS[@]}" "$APP" \
        || fail "the parent app could not be re-sealed after the bundle placement"
    echo "metallib bundle placed: $MACOS_DIR/$BUNDLE_NAME (metallib present, signed, parent re-sealed)"
else
    # COPY-ONLY mode: the caller's own machinery owns the seal (the MAS
    # lane's nested-exec sweep signs the bundle with the reduced
    # entitlements and re-seals the parent right after this step).
    echo "metallib bundle placed: $MACOS_DIR/$BUNDLE_NAME (metallib present; the caller's seal chain owns signing)"
fi
