import { describe, expect, test, vi, beforeEach, afterEach } from 'vitest';

/**
 * P-10's READ half (CG-12): the rerank echo read refuses to guess.
 *
 * The section pins (`rerank-section.test.ts`) stub this module — they pin
 * what the section RENDERS over a read's answer. What lives HERE is the
 * read itself against a stubbed wire: an older core without the `rerank`
 * field, a malformed echo, an unknown state value, a refused read and an
 * unreachable kernel each read `not-reported` — the ONE value the section
 * is allowed to render as "not reported by this kernel". A read that
 * invents a state over a missing field REDS here (damage-proofed in the
 * evidence transcript: the missing-field branch was mutated to return
 * `off` and `an_older_core_without_the_field_reads_not_reported` failed).
 *
 * The wire is stubbed at global `fetch` — openapi-fetch binds fetch ONCE
 * (the createClient default-param capture), so the global's identity never
 * changes; per-test wires swap the responder underneath.
 */

type Responder = (url: string) => Promise<Response>;
const noWire: Responder = async () => new Response('no wire', { status: 599 });
let responder: Responder = noWire;
const fetchMock = vi.fn(async (input: RequestInfo | URL) =>
	responder(input instanceof Request ? input.url : String(input))
);
vi.stubGlobal('fetch', fetchMock);

function serveHealthDb(body: unknown, status = 200): void {
	responder = async (url) =>
		url.includes('/health/db')
			? new Response(JSON.stringify(body), {
					status,
					headers: { 'content-type': 'application/json' }
				})
			: new Response('not found', { status: 404 });
}

beforeEach(() => {
	responder = noWire;
});
afterEach(() => {
	responder = noWire;
});

async function readRerank(): Promise<unknown> {
	const { fetchRerankState } = await import('../src/lib/api/kernel-facts.js');
	return fetchRerankState();
}

describe('fetchRerankState refuses to guess (P-10 read half)', () => {
	test('a live echo parses to its state, model and top_n', async () => {
		serveHealthDb({
			status: 'ok',
			version: '1.29.42',
			rerank: { state: 'ready', model: 'byo:mxbai-rerank-large-v1', top_n: 50 }
		});
		expect(await readRerank()).toEqual({
			state: 'ready',
			model: 'byo:mxbai-rerank-large-v1',
			topN: 50
		});
	});

	test('an older core without the field reads not-reported', async () => {
		// The pre-CG-12 wire shape: NO rerank key at all. The read must not
		// invent `off` (this kernel may HAVE the tier armed and simply not
		// echo it — the honest answer is that this read did not name it).
		serveHealthDb({ status: 'ok', version: '1.28.92', model_state: 'ready' });
		expect(await readRerank()).toEqual({ state: 'not-reported' });
	});

	test('a malformed or unknown echo reads not-reported', async () => {
		serveHealthDb({ status: 'ok', rerank: { state: 'turbo', model: 'x', top_n: 50 } });
		expect(await readRerank()).toEqual({ state: 'not-reported' });

		serveHealthDb({ status: 'ok', rerank: 'ready' });
		expect(await readRerank()).toEqual({ state: 'not-reported' });

		serveHealthDb({ status: 'ok', rerank: { model: 'x', top_n: 50 } });
		expect(await readRerank()).toEqual({ state: 'not-reported' });
	});

	test('a refused read reads not-reported — never a state over a refusal', async () => {
		serveHealthDb({ status: 'error' }, 500);
		expect(await readRerank()).toEqual({ state: 'not-reported' });
	});

	test('an unreachable kernel reads not-reported (the fetch throw is caught)', async () => {
		responder = async () => {
			throw new Error('connection refused');
		};
		expect(await readRerank()).toEqual({ state: 'not-reported' });
	});
});
