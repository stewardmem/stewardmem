import { expect, test, vi } from 'vitest';

/**
 * The heal's STORM GUARD, in its own file: the backoff clock is module
 * state, and a pin that shares a module graph with a test that already
 * spent the window measures the neighbour, not its subject (measured:
 * exactly that — the pin green alone, red after the middleware pin's heal
 * had consumed the window). One heal in flight, at most one ATTEMPT per
 * window — a heal that cannot fix the wire must not turn a busy surface's
 * 401 stream into an invoke storm.
 */

let healAnswers: unknown[];
const invokeLog: string[] = [];

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(async (command: string) => {
		invokeLog.push(command);
		if (command === 'kernel_token') return 'any-token';
		if (command === 'kernel_heal_credential') {
			const next = healAnswers.shift();
			if (next === undefined) throw new Error('unexpected kernel_heal_credential');
			return next;
		}
		throw new Error(`unexpected command ${command}`);
	})
}));

vi.stubGlobal('__TAURI_INTERNALS__', {});

test('the_backoff_holds — repeated 401s start exactly one heal per window', async () => {
	healAnswers = [{ outcome: 'no_source', tried: [] }];
	const store = await import('../src/lib/chat/wire-status.svelte');

	store.noteUnauthorizedResponse();
	await vi.waitFor(() => {
		expect(invokeLog.filter((c) => c === 'kernel_heal_credential').length).toBe(1);
	});
	// Five more refusals inside the window: the guard holds.
	for (let i = 0; i < 5; i += 1) {
		store.noteUnauthorizedResponse();
	}
	await new Promise((resolve) => setTimeout(resolve, 30));
	expect(invokeLog.filter((c) => c === 'kernel_heal_credential').length).toBe(1);
	expect(store.healInFlight()).toBe(false);
});
