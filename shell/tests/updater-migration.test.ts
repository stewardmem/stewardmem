import { execFileSync, spawn } from 'node:child_process';
import type { ChildProcessWithoutNullStreams } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, expect, test } from 'vitest';
import { kernelRoot, shellRoot } from './fixtures';
import { combineKernelVersions, collectPriors, priorTags } from '../scripts/updater.mjs';

// R150 — the migration table. MEASURED (2026-11-03): the shipped manifests
// vouched for exactly ONE kernel — the offered one — and the app gate
// refuses any app whose carried kernel is not on the manifest's
// kernel_versions list (`unknown_local_kernel`), so NO app older than the
// release could ever update: the operator's machine carried 1.29.39 while
// 1.29.41 shipped and was refused on sight. The vouch list is the release's
// promise to the whole installed base, not to itself. These pins hold the
// table's three parts: the combine (pure), the walk (injected fetch), and
// the lane wiring (the real CLI over a real git repo and a real HTTP
// fixture), plus the signing lane's own wiring by source shape.

const scriptPath = join(shellRoot, 'scripts', 'updater.mjs');
const lanePath = join(kernelRoot, 'scripts', 'release-app-sign.sh');
const SPAWN_BOUND_MS = 20_000;

test('the_migration_table_starts_with_this_release_and_dedupes_the_priors', () => {
	expect(combineKernelVersions('1.29.42', ['1.29.41', '1.29.40'])).toEqual([
		'1.29.42',
		'1.29.41',
		'1.29.40'
	]);
	// A re-release of the same kernel dedupes; the order is preserved.
	expect(
		combineKernelVersions('1.29.42', ['1.29.42', '1.29.41', '1.29.41'])
	).toEqual(['1.29.42', '1.29.41']);
	// Junk is skipped, never emitted; the current version is trimmed.
	expect(combineKernelVersions(' 1.29.42 ', [null, '', '  ', '1.29.41'])).toEqual([
		'1.29.42',
		'1.29.41'
	]);
});

// The walker's args, typed LOCALLY: the .mjs is untyped by resolution, so
// the pins type their own contract — the same shape the script's JSDoc
// carries, checked here where the compiler can see it.
type PriorWalk = {
	tags: string[];
	urlFor: (tag: string) => string;
	fetchImpl?: (url: string) => Promise<{ ok: boolean; status: number; json: () => Promise<unknown> }>;
};
const walk = (args: PriorWalk) =>
	collectPriors(args as unknown as {
		tags: string[];
		urlFor: (tag: string) => string;
		fetchImpl?: typeof fetch;
	});

test('the_prior_walk_collects_kernel_versions_skips_pre-chain_releases_and_refuses_a_failed_fetch', async () => {
	const ok = (body: unknown) => ({
		ok: true,
		status: 200,
		json: async () => body
	});

	const priors = await walk({
		tags: ['v1.29.41', 'v1.29.40'],
		urlFor: (tag) => `https://fixture.invalid/${tag}/latest.json`,
		fetchImpl: async (url) => {
			if (url.includes('v1.29.41')) return ok({ kernel_version: '1.29.41' });
			return ok({ kernel_version: '1.29.40' });
		}
	});
	expect(priors).toEqual(['1.29.41', '1.29.40']);

	// A pre-chain release serves no kernel_version: skipped, never invented.
	const skipped = await walk({
		tags: ['v1.29.30'],
		urlFor: (tag) => tag,
		fetchImpl: async () => ok({ version: '1.0.0' })
	});
	expect(skipped).toEqual([]);

	// A fetch that FAILS is the caller's refusal — a table guessed at is a
	// table that lies. Both shapes: an HTTP refusal and a dead transport.
	await expect(
		walk({
			tags: ['v1.29.40'],
			urlFor: (tag) => tag,
			fetchImpl: async () => ({ ok: false, status: 404, json: async () => ({}) })
		})
	).rejects.toThrow(/answered 404/);
	await expect(
		walk({
			tags: ['v1.29.40'],
			urlFor: (tag) => tag,
			fetchImpl: async () => {
				throw new Error('socket down');
			}
		})
	).rejects.toThrow(/could not be fetched/);
});

// ── the git walk ───────────────────────────────────────────────────────────

function fixtureRepo(tags: string[]): string {
	const root = mkdtempSync(join(tmpdir(), 'brain-updater-tags-'));
	const git = (args: string[], date?: string): void => {
		execFileSync('git', args, {
			cwd: root,
			encoding: 'utf8',
			timeout: SPAWN_BOUND_MS,
			env: {
				...process.env,
				GIT_AUTHOR_NAME: 'fixture',
				GIT_AUTHOR_EMAIL: 'fixture@example.invalid',
				GIT_COMMITTER_NAME: 'fixture',
				GIT_COMMITTER_EMAIL: 'fixture@example.invalid',
				...(date === undefined ? {} : { GIT_COMMITTER_DATE: date })
			}
		});
	};
	git(['init', '--quiet']);
	writeFileSync(join(root, 'README.md'), 'fixture\n');
	git(['add', '--all']);
	git(['-c', 'commit.gpgsign=false', 'commit', '--quiet', '-m', 'fixture']);
	// One commit per tag, each with its own explicit date, so the
	// creatordate sort is deterministic regardless of second granularity.
	tags.forEach((tag, index) => {
		writeFileSync(join(root, `f-${index}.txt`), `${tag}\n`);
		git(['add', '--all']);
		git(
			[
				'-c',
				'commit.gpgsign=false',
				'commit',
				'--quiet',
				'-m',
				tag,
				'--date',
				`2026-01-0${index + 1}T00:00:00Z`
			],
			`2026-01-0${index + 1}T00:00:00Z`
		);
		git(['tag', tag]);
	});
	return root;
}

test('prior_tags_walks_the_releases_before_this_one_newest_first_bounded', () => {
	const root = fixtureRepo(['v1.29.38', 'v1.29.39', 'v1.29.40', 'v1.29.41', 'v1.29.42', 'v1.29.43']);
	try {
		// The releases BEFORE v1.29.43, newest first, the window bounds the walk.
		expect(priorTags(root, 'v1.29.43', 5)).toEqual([
			'v1.29.42',
			'v1.29.41',
			'v1.29.40',
			'v1.29.39',
			'v1.29.38'
		]);
		// A narrower window truncates from the OLDEST end.
		expect(priorTags(root, 'v1.29.43', 2)).toEqual(['v1.29.42', 'v1.29.41']);
		// A tag that is not in the list walks the whole list (the fixture's
		// own honesty: the writer's tag check is a separate law).
		expect(priorTags(root, 'v9.9.9', 1)).toEqual(['v1.29.43']);
	} finally {
		rmSync(root, { recursive: true, force: true });
	}
});

// ── the CLI, end to end, over a real HTTP fixture ─────────────────────────

let server: ReturnType<typeof createServer> | null = null;
let serverPort = 0;
afterAll(() => {
	server?.close();
});

test('prior_kernels_cli_prints_the_table_and_refuses_a_failed_fetch', async () => {
	// The fixture chain: two prior releases with manifests, one without.
	const manifests: Record<string, unknown> = {
		'/releases/download/v1.29.41/latest.json': { kernel_version: '1.29.41' },
		'/releases/download/v1.29.40/latest.json': { kernel_version: '1.29.40' },
		'/releases/download/v1.29.39/latest.json': { version: '1.0.0' }
	};
	server = createServer((request, response) => {
		const body = manifests[request.url ?? ''];
		if (body === undefined) {
			response.writeHead(404).end('no such fixture asset');
			return;
		}
		response.writeHead(200, { 'content-type': 'application/json' });
		response.end(JSON.stringify(body));
	});
	await new Promise<void>((resolve) => server!.listen(0, '127.0.0.1', resolve));
	serverPort = (server.address() as { port: number }).port;

	const root = fixtureRepo(['v1.29.39', 'v1.29.40', 'v1.29.41', 'v1.29.42']);
	const staged = join(root, 'staged');
	mkdirSync(staged, { recursive: true });
	writeFileSync(
		join(staged, 'SIDECAR_PROVENANCE.json'),
		`${JSON.stringify({
			schema: 1,
			kernel_version: '1.29.42',
			x_api_version: '1.29.42',
			source_dirty: false,
			built_at: '2026-11-03T00:00:00.000Z',
			artifacts: []
		})}\n`
	);
	// The bound runner is ASYNC on purpose: the fixture HTTP server lives in
	// THIS process, and a synchronous spawn would park the event loop the
	// server needs to answer the child's fetches — a self-deadlock the 20 s
	// bound would then report as "the subprocess hit the bound". spawn +
	// await keeps the loop free; the kill timer is the same SPAWN_BOUND_MS.
		const bound = async (args: string[]) => {
		const child: ChildProcessWithoutNullStreams = spawn('node', [scriptPath, ...args]);
		let stdout = '';
		let stderr = '';
		child.stdout.on('data', (chunk: Buffer) => {
			stdout += String(chunk);
		});
		child.stderr.on('data', (chunk: Buffer) => {
			stderr += String(chunk);
		});
		const kill = setTimeout(() => child.kill('SIGKILL'), SPAWN_BOUND_MS);
		const [status] = await new Promise<[number | null]>((resolve) => {
			child.on('close', (code: number | null) => resolve([code]));
		});
		clearTimeout(kill);
		return { status, stdout, stderr };
	};
	try {
		// The happy path: this release's kernel first, then the priors'
		// (newest first), the pre-chain release skipped — one comma line.
		const good = await bound([
			'prior-kernels',
			'--root',
			root,
			'--staged',
			staged,
			'--tag',
			'v1.29.42',
			'--url-template',
			`http://127.0.0.1:${serverPort}/releases/download/{tag}`
		]);
		expect(good.status).toBe(0);
		expect(good.stdout.trim()).toBe('1.29.42,1.29.41,1.29.40');

		// A fetch that fails REFUSES the table: the lane dies on this exit.
		const dead = await bound([
			'prior-kernels',
			'--root',
			root,
			'--staged',
			staged,
			'--tag',
			'v1.29.42',
			'--url-template',
			'http://127.0.0.1:1/releases/download/{tag}'
		]);
		expect(dead.status).toBe(3);
		expect(dead.stderr).toMatch(/refusing to compute the migration table/);
	} finally {
		rmSync(root, { recursive: true, force: true });
	}
});

// ── the lane's wiring, by source shape ────────────────────────────────────

test('the_signing_lane_computes_the_table_and_passes_it_to_the_writer', () => {
	const lane = readFileSync(lanePath, 'utf8');
	expect(lane).toContain('prior-kernels');
	expect(lane).toContain('--kernel-versions "$KERNEL_VERSIONS"');
	expect(lane).toMatch(/migration table could not be computed/);
});
