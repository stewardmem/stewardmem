import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, test, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import { shellRoot } from './fixtures';

/**
 * The rerank tier's SURFACE pins (CG-12, P-9 + P-10). The kernel owns the
 * truth (its `/health/db` echo); the section renders it and names who owns
 * a change. What lives HERE:
 *
 *  1. **P-10 — an older core is never guessed at.** A kernel without the
 *     `rerank` field (and a kernel whose echo this build cannot name) reads
 *     `not-reported`, and the section says so. A mutant that invents a
 *     state over a missing field REDS here (damage-proofed in the
 *     evidence: the read was replaced by one that returns `off` over a
 *     missing echo).
 *  2. **P-9 — never writes for a kernel it does not own.** For an external
 *     posture the section renders the ownership line and the facts, and
 *     NOTHING else — a source scan + an invoke spy prove the only IPC the
 *     section can speak is the posture read.
 *  3. **The registry carries the section** in the model category, directly
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
let postureAnswer: { posture: 'app' | 'service' | 'external' } | null = null;

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(async (command: string) => {
		INVOKED.push(command);
		if (command === 'service_posture') {
			if (!postureAnswer) throw new Error('refused posture read');
			return postureAnswer;
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
	postureAnswer = null;
	rerankAnswer = { state: 'not-reported' };
});

describe('rerank section (CG-12)', () => {
	test('P-10: an older core without the echo renders not-reported — never a guessed state', async () => {
		rerankAnswer = { state: 'not-reported' };
		render(RerankSection);
		const line = await screen.findByTestId('settings-rerank-state');
		expect(line.textContent).toContain('settings.rerank.not_reported');
		expect(line.textContent).not.toContain('settings.rerank.off');
		expect(line.textContent).not.toContain('settings.rerank.ready');
	});

	test('P-10: an unknown state value reads not-reported, not a displayable guess', async () => {
		// A future kernel reporting a state this build cannot name is NOT a
		// state this build may display.
		rerankAnswer = { state: 'turbo', model: 'x', topN: 50 };
		render(RerankSection);
		const line = await screen.findByTestId('settings-rerank-state');
		expect(line.textContent).toContain('settings.rerank.not_reported');
	});

	test('the four states render their own verdict lines', async () => {
		const { unmount } = render(RerankSection);
		expect((await screen.findByTestId('settings-rerank-state')).textContent).toContain(
			'settings.rerank.not_reported'
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
		postureAnswer = { posture: 'external' };
		rerankAnswer = { state: 'ready', model: 'byo:mxbai-rerank-large-v1', topN: 50 };
		const view = render(RerankSection);
		await waitFor(() =>
			expect(view.getByTestId('settings-rerank-state').textContent).toContain(
				'settings.rerank.ready'
			)
		);
		const line = view.getByTestId('settings-rerank-state');
		expect(line.textContent).toContain('settings.rerank.ready');
		expect(await screen.findByTestId('settings-rerank-external')).toBeDefined();
		// The ONLY ipc this section spoke is the posture read.
		expect(INVOKED, 'a read-only section may invoke nothing but the posture read').toEqual([
			'service_posture'
		]);
	});

	test('P-9 (structural): the section source has no write path to smuggle', () => {
		// The only dynamic import the component may carry is the tauri core
		// (the posture read). A write command added without the ownership
		// gate reds here on shape, and the invoke spy above reds it live.
		const code = componentSource
			.split('\n')
			.filter((l) => !l.trim().startsWith('//') && !l.trim().startsWith('*'))
			.join('\n');
		const imports = [...code.matchAll(/import\('([^']+)'\)/g)].map((m) => m[1]);
		expect(imports, 'the section dynamically imports only the tauri core').toEqual([
			'@tauri-apps/api/core'
		]);
		expect(code).not.toContain('rerank_enable');
		expect(code).not.toContain('rerank_disable');
	});

	test('the registry carries the section in the model category, after the model section', () => {
		const sections = coreSettings.contributes.settingsSections;
		expect(sections, 'the core manifest contributes sections').toBeDefined();
		const ids = (sections ?? []).map((s) => s.id);
		expect(ids).toContain('rerank');
		const modelIdx = ids.indexOf('model');
		const rerankIdx = ids.indexOf('rerank');
		expect(rerankIdx).toBe(modelIdx + 1);
		expect((sections ?? [])[rerankIdx]?.category).toBe('model');
	});
});
