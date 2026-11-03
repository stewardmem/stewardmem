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
