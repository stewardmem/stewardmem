import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, test, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { shellRoot } from './fixtures';

/**
 * The rerank tier's SURFACE pins (CG-12, P-9 + P-10). The kernel owns the
 * truth (its `/health/db` echo); the section renders it and names who owns
 * a change. What lives HERE:
 *
 *  1. **P-10 — an older core is never guessed at.** A kernel without the
 *     `rerank` field (and a kernel whose echo this build cannot name) reads
 *     `not-reported`, and the section says so. Damage-proofed at the READ
 *     layer (`rerank-facts-read.test.ts`: the read mutated to invent `off`
 *     over a missing echo REDS there) and rendered here.
 *  2. **P-9 — never writes for a kernel it does not own.** For an external
 *     posture the section renders the ownership line and the facts; the
 *     write affordance does not EXIST there, and the invoke spy proves the
 *     only IPC the section can speak is reads (`build_channel`,
 *     `rerank_posture`, `service_posture`).
 *  3. **The write affordance is the DIRECT channel's, for owned postures**
 *     (P-4's renderer half): a `mas` channel renders NO toggle — the store
 *     edition cannot arm the tier; an app/service posture does, and its
 *     click invokes `rerank_set_enabled` with the flipped boolean.
 *  4. **The registry carries the section** in the model category, directly
 *     after the model section — the CG-12 placement law.
 *
 * The kernel-facts read is stubbed exactly as the paraglide/i18n mocks are:
 * the stub stands in for the transport, never for the subject (the stubbed
 * shape is the module's own documented `RerankStateRead`).
 */

const componentSource = readFileSync(
	join(shellRoot, 'src/lib/components/settings/rerank-section.svelte'),
	'utf8'
);

const INVOKED: string[] = [];
const WRITES = ['rerank_set_enabled'];
let channelAnswer: string = 'direct';
let rerankPostureAnswer: {
	chosen: boolean;
	staged: boolean;
	total_bytes: number;
	posture: 'app' | 'service' | 'external';
} | null = null;

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(async (command: string) => {
		INVOKED.push(command);
		if (command === 'build_channel') return channelAnswer;
		if (command === 'rerank_posture') {
			if (!rerankPostureAnswer) throw new Error('refused rerank posture read');
			return rerankPostureAnswer;
		}
		if (command === 'rerank_set_enabled') {
			return { outcome: 'restarted', detail: null, error: null };
		}
		if (command === 'service_posture') {
			// An older edge: the section tolerates its absence (facts only).
			throw new Error('refused posture read');
		}
		throw new Error(`unexpected invoke: ${command}`);
	})
}));

let rerankAnswer: unknown = { state: 'not-reported' };

vi.mock('#lib/api/kernel-facts.js', () => ({
	fetchRerankState: vi.fn(async () => rerankAnswer)
}));

vi.mock('$paraglide/messages.js', () => {
	const table = new Proxy(
		{},
		{
			get: (_t, key: string) => {
				return (params?: Record<string, unknown>) => {
					const suffix = params ? ` ${Object.values(params).join(' ')}` : '';
					return `${key}${suffix}`;
				};
			}
		}
	);
	return { m: table };
});

import RerankSection from '../src/lib/components/settings/rerank-section.svelte';
import { coreSettings } from '../src/lib/extensions/core-settings.js';

beforeEach(() => {
	INVOKED.length = 0;
	channelAnswer = 'direct';
	rerankPostureAnswer = null;
	rerankAnswer = { state: 'not-reported' };
});

describe('rerank section (CG-12)', () => {
	test('P-10: an older core without the echo renders not-reported — never a guessed state', async () => {
		rerankAnswer = { state: 'not-reported' };
		render(RerankSection);
		const line = await screen.findByTestId('settings-rerank-state');
		await waitFor(() => expect(line.textContent).toContain('settings.rerank.not_reported'));
		expect(line.textContent).not.toContain('settings.rerank.ready');
	});

	test('the four states render their own verdict lines', async () => {
		const { unmount } = render(RerankSection);
		await waitFor(() =>
			expect(screen.getByTestId('settings-rerank-state').textContent).toContain(
				'settings.rerank.not_reported'
			)
		);
		await unmount();

		rerankAnswer = { state: 'off', model: null, topN: 50 };
		const a = render(RerankSection);
		await waitFor(() =>
			expect(a.getByTestId('settings-rerank-state').textContent).toContain('settings.rerank.off')
		);
		await a.unmount();

		rerankAnswer = { state: 'ready', model: 'byo:mxbai-rerank-large-v1', topN: 50 };
		const b = render(RerankSection);
		await waitFor(() =>
			expect(b.getByTestId('settings-rerank-state').textContent).toContain(
				'settings.rerank.ready'
			)
		);
		const ready = b.getByTestId('settings-rerank-state');
		expect(ready.textContent).toContain('byo:mxbai-rerank-large-v1');
		expect(ready.textContent).toContain('50');
		await b.unmount();

		rerankAnswer = { state: 'failed', model: null, topN: 50 };
		const c = render(RerankSection);
		await waitFor(() =>
			expect(c.getByTestId('settings-rerank-state').textContent).toContain(
				'settings.rerank.failed'
			)
		);
		await c.unmount();

		rerankAnswer = { state: 'loading', model: null, topN: 50 };
		const d = render(RerankSection);
		await waitFor(() =>
			expect(d.getByTestId('settings-rerank-state').textContent).toContain(
				'settings.rerank.loading'
			)
		);
		await d.unmount();
	});

	test('P-9: an external kernel renders the ownership line and NOTHING writes', async () => {
		rerankPostureAnswer = {
			chosen: false,
			staged: false,
			total_bytes: 651_238_061,
			posture: 'external'
		};
		rerankAnswer = { state: 'ready', model: 'byo:mxbai-rerank-large-v1', topN: 50 };
		const view = render(RerankSection);
		await waitFor(() =>
			expect(view.getByTestId('settings-rerank-state').textContent).toContain(
				'settings.rerank.ready'
			)
		);
		expect(await screen.findByTestId('settings-rerank-external')).toBeDefined();
		// The write affordance does not EXIST for an external kernel.
		expect(screen.queryByTestId('settings-rerank-toggle')).toBeNull();
		// The ONLY ipc this section spoke is reads.
		for (const call of INVOKED) {
			expect(WRITES, `an external kernel must never see the write ${call}`).not.toContain(call);
		}
		expect(INVOKED.length).toBeGreaterThan(0);
	});

	test('P-4 (renderer half): the mas channel renders NO write affordance', async () => {
		channelAnswer = 'mas';
		rerankAnswer = { state: 'off', model: null, topN: 50 };
		const view = render(RerankSection);
		await waitFor(() =>
			expect(view.getByTestId('settings-rerank-state').textContent).toContain(
				'settings.rerank.off'
			)
		);
		expect(screen.queryByTestId('settings-rerank-toggle')).toBeNull();
		// The facts render in mas — the read path is un-gated.
		expect(INVOKED).not.toContain('rerank_set_enabled');
	});

	test('the owned, direct-channel posture offers the flip, and its click writes once', async () => {
		rerankPostureAnswer = {
			chosen: false,
			staged: false,
			total_bytes: 651_238_061,
			posture: 'app'
		};
		rerankAnswer = { state: 'off', model: null, topN: 50 };
		const view = render(RerankSection);
		const toggle = await screen.findByTestId('settings-rerank-toggle');
		// The disclose line names the one-time download before the act.
		expect(view.getByTestId('settings-rerank-disclose').textContent).toContain('651238061');
		const button = toggle.querySelector('button');
		expect(button).not.toBeNull();
		await userEvent.click(button!);
		await waitFor(() => {
			expect(INVOKED).toContain('rerank_set_enabled');
		});
		expect(INVOKED.filter((c) => c === 'rerank_set_enabled').length).toBe(1);
	});

	test('P-9 (structural): the section carries ONE write command, reads beside it', () => {
		const code = componentSource
			.split('\n')
			.filter((l) => !l.trim().startsWith('//') && !l.trim().startsWith('*'))
			.join('\n');
		const imports = [...code.matchAll(/import\('([^']+)'\)/g)].map((m) => m[1]);
		expect(
			[...new Set(imports)],
			'the section dynamically imports only the tauri core'
		).toEqual(['@tauri-apps/api/core']);
		// The write rides the ONE command; every other invoke is a read.
		expect(code).toContain("'rerank_set_enabled'");
		for (const call of code.matchAll(/invoke\('([a-z_]+)'/g)) {
			const name = call[1] ?? '';
			if (WRITES.includes(name)) continue;
			expect(
				['build_channel', 'rerank_posture', 'service_posture'],
				`the section invokes only reads beside the one write: ${name}`
			).toContain(name);
		}
	});

	test('the registry carries the section in the model category, after the model section', () => {
		const sections = coreSettings.contributes.settingsSections;
		expect(sections, 'the core manifest contributes sections').toBeDefined();
		const ids = (sections ?? []).map((s) => s.id);
		const modelIdx = ids.indexOf('model');
		const rerankIdx = ids.indexOf('rerank');
		expect(rerankIdx).toBe(modelIdx + 1);
		expect((sections ?? [])[rerankIdx]?.category).toBe('model');
	});
});
