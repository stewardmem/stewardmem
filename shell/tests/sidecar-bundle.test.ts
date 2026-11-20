import {
	execFileSync,
	spawnSync,
	type SpawnSyncOptionsWithStringEncoding,
	type SpawnSyncReturns
} from 'node:child_process';
import { createHash } from 'node:crypto';
import {
	chmodSync,
	cpSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	writeFileSync,
	type PathLike
} from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, join } from 'node:path';
import { afterAll, afterEach, expect, test } from 'vitest';
import { kernelRoot, shellRoot } from './fixtures';
import {
	checkProvenance,
	clearStaleCryptexCaches,
	machoArchs,
	provenanceFor,
	readVersions,
	stageBuildArgs
} from '../scripts/sidecar.mjs';

/**
 * III-1 — the sidecar ships INSIDE the universal bundle, and the walker FINDS
 * every Mach-O by PATH. The measured traps these pins exist for:
 *
 *  - `find … -exec file {} \; | grep -c Mach-O` counts LINES: a fat binary
 *    prints one line per slice plus a summary, so two fat files report six.
 *    The census is a SET of paths (pin 1).
 *  - `codesign -dv` exits 0 on LINKER-SIGNED Rust binaries, so "it succeeded"
 *    is vacuous. The runtime flag is the discriminator, and a per-file verify
 *    inside a broken bundle must use `--ignore-resources` or every file
 *    reports the bundle's seal error (pin 2).
 *  - the staged and bundled sidecars are NOT byte-identical — Tauri signs the
 *    copy — so provenance hashes the staged artifacts and the bundle is
 *    identified by signature + embedded version stamp + arch parity (C5).
 *
 * Everything here is executable on Ubuntu: the fixture is synthesized Mach-O
 * headers (32 bytes for thin, a fat header with real offset/size entries for
 * universal), and `codesign` is a stub prepended to PATH. The walker never
 * executes the sidecar.
 */

type Artifact = {
	name: string;
	target: string;
	sha256: string;
	size: number;
	archs: string[];
};

type ProvenanceRecord = {
	schema: number;
	kernel_commit: string;
	kernel_describe: string;
	kernel_version: string;
	x_api_version: string;
	source_dirty: boolean;
	built_at: string;
	artifacts: Artifact[];
};

type Versions = { kernelVersion: string; xApiVersion: string };

const CPU = { arm64: 0x0100000c, x86_64: 0x01000007 } as const;
type Arch = keyof typeof CPU;

const scriptPath = join(shellRoot, 'scripts', 'sidecar.mjs');

const scratch: string[] = [];

function tempDir(prefix = 'brain-sidecar-'): string {
	const directory = mkdtempSync(join(tmpdir(), prefix));
	scratch.push(directory);
	return directory;
}

afterEach(() => {
	for (const directory of scratch.splice(0)) {
		rmSync(directory, { recursive: true, force: true });
	}
});

/** A 32-byte thin Mach-O header, padded so the fixture is a real file. */
function thin(arch: Arch, payload = ''): Buffer {
	const header = Buffer.alloc(32);
	header.writeUInt32LE(0xfeedfacf, 0);
	header.writeUInt32LE(CPU[arch], 4);
	header.writeUInt32LE(0, 8); // cpusubtype
	header.writeUInt32LE(2, 12); // MH_EXECUTE
	return Buffer.concat([header, Buffer.from(payload)]);
}

/**
 * A fat Mach-O: big-endian arch table, one real thin slice per entry at the
 * declared offsets. `file` reports three lines for this on both macOS and
 * Ubuntu — which is exactly the line-count trap the census does not use.
 */
function fat(archs: Arch[], payload = ''): Buffer {
	const slices = archs.map((arch) => thin(arch, payload));
	const headerSize = 8 + 20 * slices.length;
	const firstOffset = 4096;
	const header = Buffer.alloc(headerSize);
	header.writeUInt32BE(0xcafebabe, 0);
	header.writeUInt32BE(slices.length, 4);
	let offset = firstOffset;
	slices.forEach((slice, index) => {
		const arch = archs[index];
		if (arch === undefined) throw new Error('fixture fat arch missing');
		const at = 8 + index * 20;
		header.writeUInt32BE(CPU[arch], at);
		header.writeUInt32BE(3, at + 4);
		header.writeUInt32BE(offset, at + 8);
		header.writeUInt32BE(slice.length, at + 12);
		header.writeUInt32BE(12, at + 16);
		offset += slice.length;
	});
	return Buffer.concat([header, Buffer.alloc(firstOffset - headerSize), ...slices]);
}

/**
 * The stub codesign. `PATH` decides which one the module finds, and env vars
 * decide which file fails — so one arm can red while every other arm stays
 * green, which is what "one arm reds, not the run" requires.
 */
const stubDirectory = mkdtempSync(join(tmpdir(), 'brain-codesign-'));
const stubCodesign = join(stubDirectory, 'codesign');
writeFileSync(
	stubCodesign,
	`#!/bin/sh
target=""
mode=""
for arg in "$@"; do
	case "$arg" in
		-dv|--display) mode="display" ;;
		--verify) mode="verify" ;;
		-*) ;;
		*) target="$arg" ;;
	esac
done
unsigned="\${STUB_UNSIGNED:-}"
if [ -n "$unsigned" ]; then
	case "$target" in
		*"$unsigned"*)
			echo "$target: code object is not signed at all" >&2
			exit 1
			;;
	esac
fi
if [ "$mode" = "display" ]; then
	linker="\${STUB_LINKER_SIGNED:-}"
	if [ -n "$linker" ]; then
		case "$target" in
			*"$linker"*)
				echo "CodeDirectory v=20500 flags=0x20002(adhoc,linker-signed)" >&2
				exit 0
				;;
		esac
	fi
	# The display the identity expectation reads, in the shape MEASURED on the
	# real toolchain (2026-10-29): the first Authority line is the signing
	# certificate; a Developer ID signature carries a Timestamp= line (secure
	# timestamping is the codesign default on this toolchain, and absent only
	# under --timestamp=none); a self-signed identity has NO timestamp line;
	# ad-hoc has no Authority line at all.
	# NOTE the parameter expansions WITHOUT the colon: a bash expansion of the
	# form VAR-default falls back only when the variable is UNSET, so an env
	# var set to the empty string means "emit no line" — the ad-hoc and
	# missing-timestamp postures are produced exactly that way (the colon form
	# would silently restore the default and the refusal arms would quietly
	# pass against a green posture).
	flags="\${STUB_FLAGS-0x10002(adhoc,runtime)}"
	echo "CodeDirectory v=20500 size=241 flags=$flags" >&2
	authority="\${STUB_AUTHORITY-Developer ID Application: Mark Fietje (JHB4UNPL3G)}"
	if [ -n "$authority" ]; then
		echo "Authority=$authority" >&2
	fi
	timestamp="\${STUB_TIMESTAMP-29 Oct 2026 at 19:55:41}"
	if [ -n "$timestamp" ]; then
		echo "Timestamp=$timestamp" >&2
	fi
	exit 0
fi
if [ -n "\${STUB_BUNDLE_INVALID:-}" ]; then
	case "$target" in
		*.app)
			echo "$target: a sealed resource is missing or invalid" >&2
			exit 1
			;;
	esac
fi
exit 0
`
);
chmodSync(stubCodesign, 0o755);
afterAll(() => {
	rmSync(stubDirectory, { recursive: true, force: true });
});

const stubEnv: NodeJS.ProcessEnv = {
	PATH: `${stubDirectory}${delimiter}${process.env.PATH ?? ''}`
};

type Fixture = {
	root: string;
	bundle: string;
	staged: string;
	recordPath: string;
	main: string;
	sidecar: string;
	record: ProvenanceRecord;
};

function gitHead(): string {
	return execFileSync('git', ['rev-parse', 'HEAD'], { cwd: kernelRoot, encoding: 'utf8' }).trim();
}

function writeRecord(fixture: Fixture, record: ProvenanceRecord): void {
	fixture.record = record;
	writeFileSync(fixture.recordPath, `${JSON.stringify(record, null, 2)}\n`);
}

function makeFixture(): Fixture {
	const root = tempDir();
	const bundle = join(root, 'StewardMem.app');
	const macos = join(bundle, 'Contents', 'MacOS');
	const binaries = join(bundle, 'Contents', 'Resources', 'binaries');
	const staged = join(root, 'staged');
	mkdirSync(macos, { recursive: true });
	mkdirSync(binaries, { recursive: true });
	mkdirSync(staged, { recursive: true });

	const versions = readVersions(kernelRoot) as Versions;
	const main = join(macos, 'brain-shell');
	const sidecar = join(macos, 'brain-server');
	writeFileSync(main, fat(['x86_64', 'arm64'], 'brain-shell'));
	writeFileSync(sidecar, fat(['x86_64', 'arm64'], `brain-server/${versions.kernelVersion}`));
	chmodSync(main, 0o755);
	chmodSync(sidecar, 0o755);

	// Not Mach-O: an executable script, a plist, a text file. A census that
	// counted them (or counted `file` lines) would over-report.
	const script = join(macos, 'run.sh');
	writeFileSync(script, '#!/bin/sh\necho not a Mach-O\n');
	chmodSync(script, 0o755);
	writeFileSync(
		join(bundle, 'Contents', 'Info.plist'),
		'<?xml version="1.0" encoding="UTF-8"?>\n<plist version="1.0"><dict><key>CFBundleExecutable</key><string>brain-shell</string></dict></plist>\n'
	);
	writeFileSync(join(bundle, 'Contents', 'Resources', 'note.txt'), 'not a Mach-O either\n');

	const specs: Array<{ name: string; target: string; bytes: Buffer }> = [
		{ name: 'brain-server-aarch64-apple-darwin', target: 'aarch64-apple-darwin', bytes: thin('arm64') },
		{ name: 'brain-server-x86_64-apple-darwin', target: 'x86_64-apple-darwin', bytes: thin('x86_64') },
		{
			name: 'brain-server-universal-apple-darwin',
			target: 'universal-apple-darwin',
			bytes: fat(['x86_64', 'arm64'], `brain-server/${versions.kernelVersion}`)
		}
	];
	for (const spec of specs) writeFileSync(join(staged, spec.name), spec.bytes);

	const record = provenanceFor({
		root: kernelRoot,
		artifacts: specs.map((spec) => ({
			name: spec.name,
			target: spec.target,
			path: join(staged, spec.name)
		}))
	}) as ProvenanceRecord;
	// This fixture simulates a record written by `stage` on a CLEAN tree. The
	// test itself runs before the round's commit, so the live tree is dirty.
	record.source_dirty = false;
	const recordPath = join(binaries, 'SIDECAR_PROVENANCE.json');
	writeFileSync(recordPath, `${JSON.stringify(record, null, 2)}\n`);

	return { root, bundle, staged, recordPath, main, sidecar, record };
}

function verify(
	bundle: string,
	staged: string | null,
	env: NodeJS.ProcessEnv = {},
	extraArgs: string[] = []
): SpawnSyncReturns<string> {
	const args = [scriptPath, 'verify', bundle, ...extraArgs];
	if (staged !== null) args.push('--staged', staged);
	const options: SpawnSyncOptionsWithStringEncoding = {
		cwd: kernelRoot,
		encoding: 'utf8',
		env: { ...process.env, ...stubEnv, ...env }
	};
	return spawnSync(process.execPath, args, options);
}

/** The lane's posture: the walker runs WITH the identity expectation (III-5). */
function verifyWithIdentity(
	bundle: string,
	staged: string | null,
	env: NodeJS.ProcessEnv = {}
): SpawnSyncReturns<string> {
	return verify(bundle, staged, env, ['--expect-identity', DEVELOPER_ID]);
}

const DEVELOPER_ID = 'Developer ID Application: Mark Fietje (JHB4UNPL3G)';

function output(result: SpawnSyncReturns<string>): string {
	return `${result.stdout ?? ''}${result.stderr ?? ''}`;
}

test('the_walker_finds_every_mach_o_by_path_not_by_line', () => {
	const fixture = makeFixture();

	// The trap, measured: macOS `file` prints a summary plus one line per slice,
	// so two fat files produce SIX Mach-O lines. Ubuntu's `file` prints one line
	// per fat file (two), and the census below -- not the platform's prose -- is
	// the contract. The line-count arm is therefore macOS-only; the walker and
	// the SET assertions still run on both.
	const lines = execFileSync('file', [fixture.main, fixture.sidecar], { encoding: 'utf8' })
		.split('\n')
		.filter((line) => line.includes('Mach-O'));
	if (process.platform === 'darwin') {
		expect(lines).toHaveLength(6);
	} else {
		expect(lines.length).toBeGreaterThan(0);
	}

	const result = verify(fixture.bundle, fixture.staged);
	expect(result.status).toBe(0);
	const text = output(result);
	expect(text).toContain('mach-o files: 2');
	expect(text).toContain('  Contents/MacOS/brain-server [x86_64 arm64]');
	expect(text).toContain('  Contents/MacOS/brain-shell [x86_64 arm64]');
	expect(text).not.toContain('run.sh');
	// The census is a SET: exactly the two Mach-O paths, no line duplicates.
	const census = text.split('\n').filter((line) => line.startsWith('  Contents/'));
	expect(new Set(census)).toEqual(
		new Set([
			'  Contents/MacOS/brain-server [x86_64 arm64]',
			'  Contents/MacOS/brain-shell [x86_64 arm64]'
		])
	);
	expect(census).toHaveLength(2);
	expect(text).toContain('ok — bundle seal, 2 Mach-O files');
});

test('the_walker_names_the_unsigned_mach_o_and_reds', () => {
	const fixture = makeFixture();
	const unsigned = verify(fixture.bundle, fixture.staged, { STUB_UNSIGNED: 'brain-server' });
	expect(unsigned.status).toBe(1);
	const text = output(unsigned);
	expect(text).toContain('FAIL signature Contents/MacOS/brain-server');
	expect(text).toContain('code object is not signed at all');
	// The other Mach-O passes, and the provenance arm is green: one arm reds.
	expect(text).not.toContain('FAIL signature Contents/MacOS/brain-shell');
	expect(text).not.toContain('FAIL provenance');

	// A LINKER-SIGNED binary passes `codesign -dv`, so the runtime flag is
	// the discriminator. This is the arm that makes P2's `-dv` tooth real.
	const linker = verify(fixture.bundle, fixture.staged, {
		STUB_LINKER_SIGNED: 'brain-shell'
	});
	expect(linker.status).toBe(1);
	const linkerText = output(linker);
	expect(linkerText).toContain('FAIL runtime Contents/MacOS/brain-shell');
	expect(linkerText).toContain('flags=0x20002(adhoc,linker-signed)');
	expect(linkerText).not.toContain('FAIL signature Contents/MacOS/brain-shell');
});

/**
 * III-5 — the lane's posture: the walker runs WITH the identity expectation,
 * and every Mach-O must carry the Developer ID signature WITH its secure
 * timestamp. The measured discriminators (2026-10-29, real toolchain): the
 * FIRST `Authority=` line names the signing certificate — a self-signed
 * identity is refused BY NAME — and the `Timestamp=` line exists on every
 * Developer ID signature (secure timestamping is this codesign's default;
 * its absence is the `--timestamp=none` posture, refused). Ad-hoc has no
 * Authority line at all. The expectation is opt-in so the public CI's
 * ad-hoc PROOF build keeps its own weaker pin: CI never signs (D6), and the
 * identity gate lives where the identity lives — the signing lane.
 */
test('the_walker_demands_the_developer_id_identity_and_timestamp_when_expected', () => {
	const fixture = makeFixture();
	const posture = { STUB_FLAGS: '0x10000(runtime)' };

	// GREEN — the posture the lane ships: Developer ID + secure timestamp.
	const green = verifyWithIdentity(fixture.bundle, fixture.staged, posture);
	expect(green.status).toBe(0);
	expect(output(green)).toContain(DEVELOPER_ID);
	expect(output(green)).toContain('identity+timestamp');

	// RED ① — the OLD self-signed identity, refused BY NAME. The lane must
	// never again sign a release with it (READ FIRST ②).
	const selfSigned = verifyWithIdentity(fixture.bundle, fixture.staged, {
		...posture,
		STUB_AUTHORITY: 'brain-server self-signed',
		STUB_TIMESTAMP: ''
	});
	expect(selfSigned.status).toBe(1);
	expect(output(selfSigned)).toContain('FAIL identity Contents/MacOS/brain-server');
	expect(output(selfSigned)).toContain('brain-server self-signed');

	// RED ② — ad-hoc: no Authority line at all (`Signature=adhoc`), the D5
	// posture the lane once shipped as STRUCTURE and now refuses.
	const adhoc = verifyWithIdentity(fixture.bundle, fixture.staged, {
		...posture,
		STUB_AUTHORITY: '',
		STUB_TIMESTAMP: ''
	});
	expect(adhoc.status).toBe(1);
	expect(output(adhoc)).toContain('FAIL identity');
	expect(output(adhoc)).toContain('no Authority line');

	// RED ③ — Developer ID WITHOUT the secure timestamp: the debt every
	// round since III-1 recorded as owed. The absence is refused, never
	// accepted because the identity is right.
	const unstamped = verifyWithIdentity(fixture.bundle, fixture.staged, {
		...posture,
		STUB_TIMESTAMP: ''
	});
	expect(unstamped.status).toBe(1);
	expect(output(unstamped)).toContain('FAIL timestamp Contents/MacOS/brain-server');
});

test('the_walker_refuses_an_empty_walk_and_a_missing_sidecar', () => {
	const emptyRoot = tempDir();
	const empty = join(emptyRoot, 'Empty.app');
	mkdirSync(join(empty, 'Contents', 'MacOS'), { recursive: true });
	writeFileSync(join(empty, 'Contents', 'MacOS', 'readme.txt'), 'no Mach-O here\n');
	const emptyRun = verify(empty, null);
	expect(emptyRun.status).toBe(2);
	expect(output(emptyRun)).toContain('no Mach-O');

	const fixture = makeFixture();
	const missing = join(fixture.root, 'Sidecarless.app');
	cpSync(fixture.bundle, missing, { recursive: true });
	rmSync(join(missing, 'Contents', 'MacOS', 'brain-server'));
	const missingRun = verify(missing, fixture.staged);
	expect(missingRun.status).toBe(1);
	const text = output(missingRun);
	expect(text).toContain('FAIL membership floor');
	expect(text).toContain('Contents/MacOS/brain-server');
	expect(text).toContain('census floor');
});

test('the_walker_refuses_an_unrecorded_or_stale_sidecar', () => {
	const unrecorded = makeFixture();
	rmSync(unrecorded.recordPath);
	const missingRecord = verify(unrecorded.bundle, unrecorded.staged);
	expect(missingRecord.status).toBe(1);
	expect(output(missingRecord)).toContain('unrecorded');

	const foreignCommit = makeFixture();
	const zero = '0'.repeat(40);
	writeRecord(foreignCommit, { ...foreignCommit.record, kernel_commit: zero });
	const commitRun = verify(foreignCommit.bundle, foreignCommit.staged);
	expect(commitRun.status).toBe(1);
	const commitText = output(commitRun);
	expect(commitText).toContain('FAIL provenance kernel_commit');
	expect(commitText).toContain(zero);
	expect(commitText).toContain(gitHead());

	const staleVersion = makeFixture();
	writeRecord(staleVersion, { ...staleVersion.record, kernel_version: '9.9.9' });
	const versionRun = verify(staleVersion.bundle, staleVersion.staged);
	expect(versionRun.status).toBe(1);
	const versionText = output(versionRun);
	expect(versionText).toContain('FAIL provenance kernel_version');
	expect(versionText).toContain('9.9.9');
	expect(versionText).toContain((readVersions(kernelRoot) as Versions).kernelVersion);

	const tampered = makeFixture();
	const artifact = join(tampered.staged, 'brain-server-aarch64-apple-darwin');
	writeFileSync(artifact, Buffer.concat([readFileSync(artifact), Buffer.from('tamper')]));
	const tamperedRun = verify(tampered.bundle, tampered.staged);
	expect(tamperedRun.status).toBe(1);
	expect(output(tamperedRun)).toContain(
		'FAIL staged brain-server-aarch64-apple-darwin: sha256'
	);
});

test('the_staging_record_carries_the_kernel_commit_version_and_api', () => {
	const root = tempDir();
	const aarch64 = join(root, 'brain-server-aarch64-apple-darwin');
	const x86 = join(root, 'brain-server-x86_64-apple-darwin');
	const universal = join(root, 'brain-server-universal-apple-darwin');
	writeFileSync(aarch64, thin('arm64'));
	writeFileSync(x86, thin('x86_64'));
	writeFileSync(universal, fat(['x86_64', 'arm64'], `brain-server/${(readVersions(kernelRoot) as Versions).kernelVersion}`));

	const record = provenanceFor({
		root: kernelRoot,
		artifacts: [
			{ name: 'brain-server-aarch64-apple-darwin', target: 'aarch64-apple-darwin', path: aarch64 },
			{ name: 'brain-server-x86_64-apple-darwin', target: 'x86_64-apple-darwin', path: x86 },
			{
				name: 'brain-server-universal-apple-darwin',
				target: 'universal-apple-darwin',
				path: universal
			}
		]
	}) as ProvenanceRecord;
	const versions = readVersions(kernelRoot) as Versions;

	expect(record.schema).toBe(1);
	expect(record.kernel_commit).toBe(gitHead());
	expect(record.kernel_version).toBe(versions.kernelVersion);
	expect(record.x_api_version).toBe(versions.xApiVersion);
	expect(record.artifacts[0]?.sha256).toBe(
		createHash('sha256').update(readFileSync(aarch64)).digest('hex')
	);
	expect(record.artifacts[0]?.size).toBe(readFileSync(aarch64).length);
	expect(record.artifacts[0]?.archs).toEqual(['arm64']);
	expect(record.artifacts[1]?.archs).toEqual(['x86_64']);
	expect(machoArchs(aarch64)).toEqual(['arm64']);

	const written = join(root, 'SIDECAR_PROVENANCE.json');
	writeFileSync(written, `${JSON.stringify(record, null, 2)}\n`);
	expect(JSON.parse(readFileSync(written, 'utf8'))).toEqual(record);

	// checkProvenance is the reader of the record `provenanceFor` writes.
	// Against a bundle that carries the stamp, a clean record passes; the
	// missing-sidecar arm is the negative control.
	const bundle = join(root, 'Stamp.app');
	const macos = join(bundle, 'Contents', 'MacOS');
	mkdirSync(macos, { recursive: true });
	writeFileSync(join(macos, 'brain-server'), thin('arm64', `brain-server/${versions.kernelVersion}`));
	const accepted = checkProvenance({
		record: { ...record, source_dirty: false },
		recordPath: written,
		bundle
	});
	expect(accepted).toEqual([]);
});

test('the_staged_kernel_compiles_the_shipped_feature_set', () => {
	// The bundled sidecar's `--import` — the workspace import's engine —
	// lives behind the kernel's `migrate` feature, and the shipped feature
	// set everywhere else is `bench,migrate,rerank-tier` (release.yml's
	// Linux build, the bench lane, the migrate suite; CG-12's ship-set
	// decision added the rerank tier to the set the app's choice can act
	// on). A stage that compiled the DEFAULT features would ship a sidecar
	// that silently ignores `--import` and falls through to the server
	// boot — the stale-binary near-miss shape, one release away from the
	// operator's store. The argv carries the feature set BY NAME, per
	// triple, and stays locked.
	for (const target of ['aarch64-apple-darwin', 'x86_64-apple-darwin']) {
		const args = stageBuildArgs(target) as string[];
		expect(args).toContain('--locked');
		expect(args).toContain('--features');
		const features = args[args.indexOf('--features') + 1] ?? '';
		expect(features.split(',')).toEqual(expect.arrayContaining(['bench', 'migrate']));
		// The CG-12 ship-set: the rerank tier rides by name where the
		// runtime ships a prebuilt for the arch — arm64 only. ort-sys
		// carries no x86_64-apple-darwin prebuilt (its dist.tsv has no
		// such row), so the x86_64 leg building with the tier fails at
		// the ort build script; the tier compiles OFF there instead,
		// which is a valid posture (the health echo reads `off`).
		if (target.startsWith('x86_64')) {
			expect(features.split(',')).not.toContain('rerank-tier');
		} else {
			expect(features.split(','), 'the CG-12 ship-set: the rerank tier rides by name').toContain(
				'rerank-tier'
			);
		}
		expect(args[args.indexOf('--bin') + 1]).toBe('brain-server');
		expect(args[args.indexOf('--target') + 1]).toBe(target);
	}
});

test('the_swift_sidecars_build_and_stage_arm64_only', () => {
	// The named act: the Swift sidecars (brain-afm, shell-vision) left the
	// kernel's three-file law. Every app lane builds aarch64, the Tools set
	// installs the aarch64 slice, and no consumer anywhere names a Swift
	// Intel or universal slice (measured: zero references) — while the x86_64
	// leg of each universal Swift build paid full compile time and printed
	// the SDK's deprecation warning on every stage. The pins hold the law
	// structurally: the swift argv carries exactly one --arch, the kernel's
	// universal expectation is untouched, and lipo serves the KERNEL
	// universal alone (the Swift product of a single-arch build is already
	// thin — copied, never thinned).
	const source = readFileSync(scriptPath, 'utf8');
	expect(source, 'the Swift build must not request an x86_64 slice').not.toContain("'--arch', 'x86_64'");
	expect(source).toMatch(/\['build', '-c', 'release', '--arch', 'arm64'\]/);
	expect(source, 'the KERNEL keeps the universal three-file law').toContain(
		"const expectedArchs = ['arm64', 'x86_64'];"
	);
	expect(source).toContain('const SWIFT_TARGET = ');
	const lipoCallSites = source.match(/run\('lipo'/g) ?? [];
	expect(
		lipoCallSites,
		'lipo serves the kernel universal alone — the Swift sidecars copy a thin product'
	).toHaveLength(1);
	// The stale-shape law: a stage that no longer writes the universal and
	// Intel slices must not leave a prior stage's copies behind — the staged
	// set is exactly what this run's record names.
	expect(source).toContain("for (const staleShape of [UNIVERSAL_TRIPLE, 'x86_64-apple-darwin'])");
});

test('the_cryptex_guard_clears_stale_mount_references_and_spares_live_ones', () => {
	// The OS rotates its asset cryptexes under OS updates; SwiftPM's cached
	// build plan and the Metal dep files keep naming the OLD mount, and the
	// next swift build dies spawning `metal` from a dead path — the failure
	// that reads like a toolchain break and has killed staging twice. The
	// guard clears the poisoned caches (the XCBuildData plan cache
	// wholesale + every cache file naming a dead mount) and spares live
	// references, clean files, and the checkouts/ sources.
	const root = tempDir('brain-cryptex-');
	const macosDir = join(root, 'shell-macos');
	const xcData = join(macosDir, '.build', 'out', 'Intermediates.noindex', 'XCBuildData');
	const metalDir = join(macosDir, '.build', 'out', 'Intermediates.noindex', 'mlx-swift_Cmlx-b.build', 'Metal');
	const checkouts = join(macosDir, '.build', 'checkouts', 'mlx-swift');
	mkdirSync(xcData, { recursive: true });
	mkdirSync(metalDir, { recursive: true });
	mkdirSync(checkouts, { recursive: true });
	const dead = '/var/run/com.apple.security.cryptexd/mnt/com.apple.MobileAsset.MetalToolchain-v27.2.47.0.deadM0unt';
	const live = '/var/run/com.apple.security.cryptexd/mnt/com.apple.MobileAsset.MetalToolchain-v27.2.47.0.liveM0unt';
	writeFileSync(join(xcData, 'build.db'), `plan names ${dead}/Metal.xctoolchain/usr/bin/metal`);
	writeFileSync(join(metalDir, 'random.dat'), `${dead}/usr/bin/metal random.metal\n`);
	writeFileSync(join(metalDir, 'live.dat'), `${live}/usr/bin/metal live.metal\n`);
	writeFileSync(join(metalDir, 'clean.dat'), 'no mount references at all');
	writeFileSync(join(checkouts, 'Generated.c'), `// ${dead}/header`); // sources are not caches

	const liveMounts = new Set([live]);
	// The probe parameter is the SEAM the guard calls through — typed as
	// node's own `existsSync` shape (PathLike), so the test's stand-in must
	// accept exactly what the guard may hand it (never a narrower string).
	const report = clearStaleCryptexCaches(
		macosDir,
		(p: PathLike) => liveMounts.has(String(p)) || existsSync(p)
	);

	expect(report.staleMounts).toEqual([dead]);
	expect(report.removedPaths).toContain(join(metalDir, 'random.dat'));
	expect(report.removedPaths).toContain(xcData);
	expect(existsSync(join(metalDir, 'random.dat'))).toBe(false);
	expect(existsSync(join(metalDir, 'live.dat'))).toBe(true);
	expect(existsSync(join(metalDir, 'clean.dat'))).toBe(true);
	expect(existsSync(join(checkouts, 'Generated.c'))).toBe(true);
});

test('the_cryptex_guard_is_a_noop_on_clean_and_absent_build_trees', () => {
	// A guard that fires on a clean tree would be churn; a guard that fires
	// without scanning (probe hostile, tree empty) would be a false clear.
	// Both arms must be no-ops that touch nothing.
	const absent = clearStaleCryptexCaches(join(tempDir('brain-cryptex-'), 'nope'), () => false);
	expect(absent.staleMounts).toEqual([]);
	expect(absent.removedPaths).toEqual([]);

	const root = tempDir('brain-cryptex-');
	const out = join(root, '.build', 'out');
	mkdirSync(out, { recursive: true });
	writeFileSync(join(out, 'build.db'), 'references no mount');
	const clean = clearStaleCryptexCaches(root, () => false);
	expect(clean.staleMounts).toEqual([]);
	expect(clean.removedPaths).toEqual([]);
	expect(existsSync(join(out, 'build.db'))).toBe(true);
});

test('the_macos_window_hides_its_native_title_and_keeps_the_accessible_one', () => {
	// UI2 S1 — the overlay titlebar still draws the window TITLE centered on
	// macOS (measured on the built app: "StewardMem" painted over the first
	// session row), because `titleBarStyle: "Overlay"` extends content under
	// the bar but does not, by itself, stop the title text. WindowConfig's
	// `hiddenTitle` ("If true, sets the window title to be hidden on macOS",
	// Tauri v2 reference) removes exactly that. The `title` value STAYS:
	// Mission Control, ⌘Tab and the accessibility tree read it — only the
	// painted text goes. The pin holds both facts: the flag is true, and the
	// title is non-empty (an empty title would trade a collision for an
	// unnamed window).
	const overlay = JSON.parse(
		readFileSync(join(shellRoot, 'src-tauri', 'tauri.macos.conf.json'), 'utf8')
	) as { app: { windows: Array<Record<string, unknown>> } };
	const main = overlay.app.windows.find((w) => w['label'] === 'main');
	expect(main, 'the main window block exists in the macOS overlay').toBeDefined();
	expect(
		main?.['hiddenTitle'],
		'the macOS window must set hiddenTitle: true — without it the overlay ' +
			'titlebar still paints the centered native title over the first session row'
	).toBe(true);
	expect(
		typeof main?.['title'] === 'string' && (main?.['title'] as string).length > 0,
		'the window title must stay non-empty: Mission Control and the ' +
			'accessibility tree read it even while hiddenTitle stops the paint'
	).toBe(true);
	expect(main?.['titleBarStyle']).toBe('Overlay');
});

test('the_sidecar_is_wired_macos_only_and_its_artifacts_are_ignored', () => {
	const base = JSON.parse(
		readFileSync(join(shellRoot, 'src-tauri', 'tauri.conf.json'), 'utf8')
	) as Record<string, unknown>;
	expect(JSON.stringify(base)).not.toContain('externalBin');
	expect(JSON.stringify(base)).not.toContain('SIDECAR_PROVENANCE');

	const overlay = JSON.parse(
		readFileSync(join(shellRoot, 'src-tauri', 'tauri.macos.conf.json'), 'utf8')
	) as {
		app: { windows: Array<{ windowEffects?: { effects: string[] } }> };
		bundle: {
			externalBin: string[];
			resources: string[];
			macOS: {
				minimumSystemVersion: string;
				hardenedRuntime: boolean;
				signingIdentity: string;
				infoPlist: string;
				files: Record<string, string>;
			};
		};
	};
	// brain-afm joins the overlay's externalBin (the chat spine, v1.29.32):
	// the AFM bridge ships beside the kernel; the base config stays clean.
	// shell-vision joins them (the OCR sidecar, IMPROVE05) — one bundle
	// list, three sidecars, all resolved beside the kernel at runtime.
	expect(overlay.bundle.externalBin).toEqual([
		'binaries/brain-server',
		'binaries/brain-afm',
		'binaries/shell-vision'
	]);
	expect(overlay.bundle.resources).toEqual(['binaries/SIDECAR_PROVENANCE.json']);
	expect(overlay.bundle.macOS).toEqual({
		minimumSystemVersion: '15.0',
		hardenedRuntime: true,
		signingIdentity: 'Developer ID Application: Mark Fietje (JHB4UNPL3G)',
		// The platform-keys overlay (round 18 S-E): the bundler's sanctioned
		// infoPlist hook merges CFBundleSupportedPlatforms=[MacOSX] +
		// DTPlatformName=macosx LAST, so System Information reads the app as
		// Apple Silicon (arch_arm) instead of the iOS-kind fall-through.
		infoPlist: './macos-info.plist'
	});
	// THE METALLIB BUNDLE LAW (round 21) — measured, Posture L 2026-11-15:
	// the placement must NOT ride the tauri conf's macOS.files map. The
	// bundler's own seal refuses an unsigned nested bundle in Contents/
	// MacOS (`In subcomponent: mlx-swift_Cmlx.bundle … failed to sign app`)
	// and offers no hook to sign one; the placement lives in
	// scripts/stage-metallib-bundle.sh, called by the release lane (CI),
	// the Developer-ID lane, and the MAS lane (whose nested-exec sweep
	// signs the bundle with the reduced entitlements). The pin keeps the
	// conf clean; a future files-mapping attempt would re-break the seal.
	expect(overlay.bundle.macOS.files).toBeUndefined();
	// The placement's wired legs: the helper exists, and the release lane
	// calls it BEFORE the verify walk (whose metallib check is the law's
	// CI-level proof). The MAS and Developer-ID lanes carry the same call
	// in their own scripts (release-app-store.sh / release-app-sign.sh).
	const helper = readFileSync(
		join(kernelRoot, 'scripts', 'stage-metallib-bundle.sh'),
		'utf8'
	);
	expect(helper).toContain('mlx-swift_Cmlx.bundle');
	expect(helper).toContain('default.metallib');
	// THE RE-SEAL'S RUNTIME FLAG IS UNCONDITIONAL (v1.29.46's first public
	// lane run, 2026-11-18): the walker demands `runtime` on EVERY Mach-O
	// — ad-hoc lane included — and the bundler's own seal carries it on
	// both lanes, so a re-seal without `--options runtime` STRIPS the flag
	// the build just set (the live red: brain-shell re-sealed to
	// flags=0x2(adhoc), the walk refused the bundle). The identity-gated
	// branch below must gate ONLY the timestamp (a secure timestamp needs
	// a real identity; the runtime bit does not) — asserted structurally:
	// the runtime option rides the UNCONDITIONAL argument lists, and the
	// `[ "$SIGN" != "-" ]` branch carries no `--options` of its own.
	expect(
		helper,
		'the re-seal must apply --options runtime unconditionally (both lanes)'
	).toMatch(/RESEAL_ARGS=\(--force --sign "\$SIGN" --options runtime\)/);
	expect(
			helper,
		'the Cmlx bundle signing must apply --options runtime unconditionally (both lanes)'
		).toMatch(/SIGN_ARGS=\(--force\)/); // followed by --sign + runtime outside the identity branch
	const unconditionalRuntime = /SIGN_ARGS=\(--force\)\n\s*if \[ -n "\$SIGN" \]; then\n\s*SIGN_ARGS\+=\(--sign "\$SIGN" --options runtime\)/.test(
		helper
	);
	expect(
		unconditionalRuntime,
		'the Cmlx signing assembles --options runtime BEFORE the identity branch (adhoc keeps it)'
	).toBe(true);
	const timestampBranchHasNoOptions = !/\[ "\$SIGN" != "-" \][^\n]*\n[^\n]*--options/.test(helper);
	expect(
		timestampBranchHasNoOptions,
		'the identity branch must gate only the timestamp — a --options inside it strips runtime from the adhoc lane'
	).toBe(true);
	const releaseWorkflow = readFileSync(
		join(kernelRoot, '.github', 'workflows', 'release.yml'),
		'utf8'
	);
	expect(releaseWorkflow).toContain('scripts/stage-metallib-bundle.sh');
	expect(overlay.app.windows[0]?.windowEffects?.effects).toContain('sidebar');

	expect(() =>
		execFileSync('git', ['check-ignore', '-q', 'shell/src-tauri/binaries/'], { cwd: kernelRoot })
	).not.toThrow();
	expect(
		execFileSync('git', ['ls-files', 'shell/src-tauri/binaries'], {
			cwd: kernelRoot,
			encoding: 'utf8'
		}).trim()
	).toBe('');
});
