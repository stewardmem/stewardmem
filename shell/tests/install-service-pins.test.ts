import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from 'vitest';
import { shellRoot } from './fixtures';

// scripts/install-service.sh installs the locally-built kernel and the
// operator CLIs into ~/.local/bin. On 2026-11-03 a run MEASURED the
// write-through: `cp -f "$src" "$dest"` with $dest a symlink (the Tools
// pkg's `brain-server -> ".../StewardMem/tools/1.29.40/brain-server"`)
// silently overwrote the versioned binary in place — the directory's
// name stopped naming its contents, the previous version's bytes were
// gone, and the pkg's `brain-server-1.29.40` alias was left pointing at
// the liar. These pins hold the installer's landing laws so the
// write-through cannot come back.

const script = readFileSync(
	join(shellRoot, '..', 'scripts', 'install-service.sh'),
	'utf8'
);

test('the_installer_never_writes_through_a_symlink_destination', () => {
	// The destination's shape is resolved before any copy lands.
	expect(script).toContain('[[ -L "$dest" ]]');
	expect(script).toContain('readlink');
	// A symlink whose target is NOT the adopted layout is refused, naming
	// the link and its target — an unknown link's target is not ours to
	// overwrite (the fail-closed law every other edge here follows).
	expect(script).toContain('refusing to write through the symlink');
	expect(script).toContain('TOOLS_ROOT');
});

test('the_stale_bundle_refusal_names_the_bundle_dir_it_actually_reads', () => {
	// The installer reads the bundle from the aarch64 target dir
	// (ARM64-ONLY since III-5, 2026-10-29 — APP_BUNDLE_DIR above). The
	// stale-bundle refusal's remedy must name the target that lands a
	// bundle in THAT dir; the old text named universal-apple-darwin, the
	// pre-III-5 shape — a command that builds a bundle the installer
	// never reads (and that no longer builds: tauri's universal path
	// demands the crate's every src/bin target, measured 2026-11-13).
	expect(script).toContain(
		'pnpm tauri build --bundles app --target aarch64-apple-darwin'
	);
	expect(script).not.toContain('--target universal-apple-darwin');
});

test('a_tools_layout_link_gets_the_pkg_own_versioned_install_and_flip', () => {
	// The version is MEASURED from the built binary, fail-closed — an
	// unparseable `--version` is a refusal, never an empty dir name (the
	// app gate's NEW_KERNEL law).
	expect(script).toContain('BUILD_VERSION');
	expect(script).toContain('die "cannot read the built version');
	// The binary lands in tools/<version>/ — never inside an older
	// version's directory.
	expect(script).toContain('mkdir -p "$dir"');
	// The link is re-pointed by atomic rename of a temp link, not edited
	// in place; the pkg's <name>-<version> aliases are untouched and keep
	// naming their own versions.
	expect(script).toContain('mv -f "$tmp_link" "$dest"');
});

test('no_plain_copy_remains_in_the_install_path', () => {
	// The measured defect was exactly this line writing through a symlink.
	expect(script).not.toContain('cp -f "$src" "$dest"');
	// Every landed file arrives by temp-copy + atomic rename — no
	// truncated-binary window when the destination is being served.
	expect(script).toContain('mv -f "$tmp" "$dest"');
});

test('the_installer_pins_the_shell_build_as_the_seat_dist', () => {
	// The /app seat's dist is an INSTALLER-MANAGED fact (plan 19 S1): the
	// Dioxus-era plist key pointed at client/dist, which the decommission
	// deletes — and the seat's in-process default (the RELATIVE shell/build)
	// only resolves when the server's CWD is the repo, which launchd's is
	// not. The installer pins the repo-absolute shell/build with
	// replace-any-prior-value (remove-then-insert, idempotent), never
	// writing a dist dir that does not exist: an absent build is a refusal
	// naming the build command, not a dead seat.
	expect(script).toContain('SHELL_DIST="$REPO/shell/build"');
	expect(script).toContain('plutil -remove EnvironmentVariables.BRAIN_CLIENT_DIST');
	expect(script).toContain('plutil -insert EnvironmentVariables.BRAIN_CLIENT_DIST -string "$SHELL_DIST"');
	// Fail-closed on an absent dist: the refusal names the build command.
	expect(script).toContain('pnpm build');
	expect(script).toContain('die "shell/build is absent');
});
