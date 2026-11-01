import { describe, expect, test, vi } from 'vitest';

/**
 * The credential SELF-HEAL — the store's law (the Rust core's own pins live
 * in `shell/src-tauri/src/heal_pins.rs`). Every classification site (the
 * page's probe, the frame's facts read, the badge's queue read) flows through
 * `classifyWireFault`, and every typed read's 401 answer flows through the
 * client's middleware — so the heal lives THERE and these pins drive the REAL
 * store and the REAL client:
 *
 * - a credential fault triggers EXACTLY ONE heal; a healed outcome
 *   invalidates the stale memoized bearer (a fresh `kernel_token` read) and
 *   clears the fault; any other outcome leaves the fault standing;
 * - a non-credential fault never invokes the heal; the PWA never heals;
 * - a typed-client 401 reaches the heal through the middleware, the caller
 *   still receives the refusal untouched, and a healed memo makes the next
 *   read answer 200;
 * - the backoff holds: a 401 stream cannot become an invoke storm.
 *
 * Every test re-imports the modules (`vi.resetModules`): the backoff clock
 * and the credential memo are module state, and each pin needs a clean one.
 */

let wireStatusKind: string;
let healAnswers: unknown[];
/** What the (mock) keychain answers — the heal "adopts" by flipping it. */
let keychainValue = 'stale-token';
const invokeLog: string[] = [];

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(async (command: string) => {
		invokeLog.push(command);
		if (command === 'kernel_token') return keychainValue;
		if (command === 'kernel_wire_status') return { kind: wireStatusKind };
		if (command === 'kernel_heal_credential') {
			const next = healAnswers.shift();
			if (next === undefined) throw new Error('unexpected kernel_heal_credential');
			// A healed heal adopted the kernel's token into the keychain.
			if (next && (next as { outcome: string }).outcome === 'healed') {
				keychainValue = 'freshly-healed-token';
			}
			return next;
		}
		throw new Error(`unexpected command ${command}`);
	})
}));

vi.stubGlobal('__TAURI_INTERNALS__', {});

async function fresh(fetchImpl?: (input: RequestInfo | URL) => Promise<Response>) {
	vi.resetModules();
	healAnswers = [];
	invokeLog.length = 0;
	wireStatusKind = 'unknown';
	keychainValue = 'stale-token';
	// The fetch stub MUST be installed BEFORE the client module is imported:
	// openapi-fetch resolves its fetch at creation, so a later stub never
	// sees a request. (Measured: a stub assigned after the import answered
	// nothing, and the reads went to the REAL loopback kernel.)
	if (fetchImpl) vi.stubGlobal('fetch', vi.fn(fetchImpl));
	return {
		store: await import('../src/lib/chat/wire-status.svelte'),
		bootstrap: await import('../src/lib/api/bootstrap.js'),
		client: await import('../src/lib/api/client.js')
	};
}

function healCount(): number {
	return invokeLog.filter((command) => command === 'kernel_heal_credential').length;
}

describe('the credential self-heal', () => {
	test('a_classified_unauthorized_fault_invokes_the_heal_once_and_a_heal_clears_the_fault', async () => {
		const { store } = await fresh();
		wireStatusKind = 'unauthorized';
		healAnswers.push({ outcome: 'healed' });

		store.classifyWireFault('unauthorized');
		expect(store.currentWireFault()).toBe('unauthorized');

		await vi.waitFor(() => expect(healCount()).toBe(1));
		await vi.waitFor(() => {
			// The healed wire invalidates the stale memoized bearer: a fresh
			// keychain read — without it the session would keep 401-ing on the
			// stale in-memory token even after the keychain was repaired.
			expect(invokeLog).toContain('kernel_token');
		});
		expect(store.currentWireFault()).toBeNull();
		expect(store.healInFlight()).toBe(false);
	});

	test('a_no_credential_fault_heals_too', async () => {
		const { store } = await fresh();
		wireStatusKind = 'no_credential';
		healAnswers.push({ outcome: 'healed' });

		store.classifyWireFault('no_credential');
		await vi.waitFor(() => expect(healCount()).toBe(1));
		expect(store.currentWireFault()).toBeNull();
	});

	test('a_not_needed_outcome_still_clears_the_fault_the_wire_already_works', async () => {
		const { store } = await fresh();
		wireStatusKind = 'unauthorized';
		healAnswers.push({ outcome: 'not_needed' });

		store.classifyWireFault('unauthorized');
		await vi.waitFor(() => expect(healCount()).toBe(1));
		// not_needed means the probe found the wire REACHABLE: the classified
		// fault is stale by definition, and it comes down.
		expect(store.currentWireFault()).toBeNull();
	});

	test('a_failed_heal_keeps_the_fault_named_and_the_stale_bearer', async () => {
		const { store } = await fresh();
		wireStatusKind = 'unauthorized';
		healAnswers.push({ outcome: 'no_source', tried: [] });

		store.classifyWireFault('unauthorized');
		await vi.waitFor(() => expect(healCount()).toBe(1));
		// The banner's fact stands: nothing was adopted, so nothing changed.
		expect(store.currentWireFault()).toBe('unauthorized');
		// No bearer invalidation — the keychain was not touched.
		expect(invokeLog).not.toContain('kernel_token');
		expect(store.healInFlight()).toBe(false);
	});

	test('a_non_credential_fault_never_invokes_the_heal', async () => {
		const { store } = await fresh();
		for (const fault of ['unreachable', 'origin_not_allowed', 'unexpected', 'unknown'] as const) {
			store.clearWireFault();
			store.classifyWireFault(fault);
			expect(store.currentWireFault()).toBe(fault);
		}
		// Let any stray async work land.
		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(healCount()).toBe(0);
	});

	test('the_heal_fires_once_per_classification_not_in_a_loop', async () => {
		const { store } = await fresh();
		wireStatusKind = 'unauthorized';
		healAnswers.push({ outcome: 'no_source', tried: [] });

		store.classifyWireFault('unauthorized');
		await vi.waitFor(() => expect(healCount()).toBe(1));
		// A heal that cannot fix the wire is NOT retried by the store: the
		// manual paths (Restart, Settings) own the stubborn cases.
		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(healCount()).toBe(1);
	});

	test('the_pwa_never_heals — no core, the operator owns the kernel there', async () => {
		const { store } = await fresh();
		const internals = (globalThis as Record<string, unknown>)['__TAURI_INTERNALS__'];
		delete (globalThis as Record<string, unknown>)['__TAURI_INTERNALS__'];
		try {
			wireStatusKind = 'unauthorized';
			store.classifyWireFault('unauthorized');
			await new Promise((resolve) => setTimeout(resolve, 20));
			expect(healCount()).toBe(0);
			// The fault stands: without a core there is nothing to repair with.
			expect(store.currentWireFault()).toBe('unauthorized');
		} finally {
			(globalThis as Record<string, unknown>)['__TAURI_INTERNALS__'] = internals;
		}
	});

	test('a_typed_client_401_reaches_the_heal_through_the_client_middleware', async () => {
		// The CHOKE POINT: the review queue's refusal (a 401 answer) is not a
		// banner fault, but it is the credential being stale — and the heal
		// fires from the client's own middleware, no surface changes needed.
		let statsCalls = 0;
		const seenAuth: (string | undefined)[] = [];
		const { store, client } = await fresh(async (input) => {
			const url =
				typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
			if (url.includes('/stats')) {
				statsCalls += 1;
				const auth =
					typeof input === 'object' && input !== null && 'headers' in input
						? (input as Request).headers.get('authorization')
						: undefined;
				seenAuth.push(auth ?? undefined);
				// 401 until the heal lands, then 200 — the healed wire.
				return new Response(statsCalls === 1 ? '{}' : '{"count":1}', {
					status: statsCalls === 1 ? 401 : 200
				});
			}
			if (url.includes('/health')) {
				return new Response('{"status":"ok","version":"1.29.39"}', { status: 200 });
			}
			return new Response('{}', { status: 200 });
		});
		healAnswers.push({ outcome: 'healed' });
		const result = await client.client.GET('/stats');
		// The refusal still reaches the CALLER unchanged (the taxonomy: the
		// kernel spoke) — but the heal ran beside it.
		expect(result.response.status).toBe(401);
		await vi.waitFor(() => expect(healCount()).toBe(1));
		// The heal's invalidation hop: refreshBearer re-reads the keychain.
		await vi.waitFor(() => expect(invokeLog).toContain('kernel_token'));
		// The healed wire invalidated the memo; a SECOND read now answers.
		const second = await client.client.GET('/stats');
		expect(second.response.status).toBe(200);
		// The witness: the first read carried no working credential, the
		// second carried the HEALED one.
		expect(seenAuth[0]).not.toContain('freshly-healed-token');
		expect(seenAuth[1]).toContain('freshly-healed-token');
		void store;
		vi.unstubAllGlobals();
	});

	test('a_cors_blocked_401_still_reaches_the_heal_through_the_error_diagnosis', async () => {
		// THE PRODUCTION REALITY the 2026-11-01 drill measured: the kernel's
		// 401 answer carries no `access-control-allow-origin`, so the webview
		// CORS-blocks it and fetch() REJECTS — no Response ever reaches the
		// onResponse choke point (twelve 401s on the wire, zero heal
		// invocations). The heal's arm therefore lives where the fault is
		// NAMED: the error path's diagnosis. The caller still receives the
		// named fault untouched, and the heal runs beside it.
		const { client } = await fresh(async () => {
			// Exactly what the webview raises for the CORS-blocked 401.
			throw new TypeError('Failed to fetch');
		});
		// The middleware pin above ends with `unstubAllGlobals`, which also
		// removed the `__TAURI_INTERNALS__` stub this file installed at load —
		// without it the store takes the PWA branch and never heals. Re-stub.
		vi.stubGlobal('__TAURI_INTERNALS__', {});
		wireStatusKind = 'unauthorized';
		healAnswers.push({ outcome: 'healed' });
		let thrown: unknown = null;
		try {
			await client.client.GET('/stats');
		} catch (error) {
			thrown = error;
		}
		// The caller sees the NAMED fault (the taxonomy holds — the kernel
		// spoke, the surface renders the credential sentence).
		expect((thrown as { fault?: string } | null)?.fault).toBe('unauthorized');
		// And the heal fired beside it — once, with the invalidation hop.
		await vi.waitFor(() => expect(healCount()).toBe(1));
		await vi.waitFor(() => expect(invokeLog).toContain('kernel_token'));
		vi.unstubAllGlobals();
	});

	// The storm-guard pin lives in `heal-backoff.test.ts` — its own module
	// graph, because the backoff clock is module state a neighbour test can
	// spend (measured: this pin green alone, red after the middleware pin's
	// heal had consumed the 30 s window).
});
