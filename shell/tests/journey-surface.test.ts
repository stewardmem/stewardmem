import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import { shellRoot } from './fixtures';

/**
 * IMPROVE15 — the operator journey surface: the kernel's chain-projected
 * transparency read, rendered in the shell. The projection law, surface-side:
 * the page adds ZERO state and renders the kernel's closed sentences
 * verbatim. The pins:
 *
 *   J1  the live render shows the kernel's words verbatim — the closed
 *       sentences, the ROLE actors, the receipt digests (no paraphrase)
 *   J2  the offline render shows METADATA ONLY — counts, never event text
 *       (the III-6 law; the cache type carries no field that could hold a
 *       sentence, and the pin plants a poisoned cache to prove the read
 *       drops it)
 *   J3  a refusal renders a REFUSAL — the journey's own authz law's
 *       existence-blind 404 is named as one answer for unknown client,
 *       another client's client, and a foreign case
 *   J4  the surface is a READ — the page and its api module carry no write
 *       verb on the typed wire
 *   J5  the census, the parity row, the sweep arm and the five catalogs
 *       carry the route
 */

afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
	sessionStorage.clear();
});

// The Rust core answers "which fault is the webview hitting?". The journey's
// offline arm is driven by a REAL unreachable — fetch REJECTS, the client's
// error middleware diagnoses through this mock, and the read names the fault
// (the review-queue test's own machine, reused).
const state = vi.hoisted(() => ({ wireStatus: 'unreachable' }));
vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(async (command: string) => {
		if (command === 'kernel_wire_status') return { kind: state.wireStatus };
		throw new Error(`unexpected command ${command}`);
	})
}));

type Responder = (url: string, init: RequestInit | undefined, method: string) => Promise<Response>;
// A REAL unreachable: fetch rejects (the engine's one network failure), never
// a polite 599 — a response is the kernel SPEAKING, a rejection is the wire
// being gone.
const noWire: Responder = async () => {
	throw new TypeError('fetch failed: the kernel is not answering');
};
let responder: Responder = noWire;
const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
	const url = input instanceof Request ? input.url : String(input);
	const method = init?.method ?? (input instanceof Request ? input.method : 'GET');
	return responder(url, init, method);
});

function jsonResponse(body: unknown, status = 200): Response {
	return new Response(JSON.stringify(body), {
		status,
		headers: { 'content-type': 'application/json' }
	});
}

function journeyBody(): Record<string, unknown> {
	return {
		client: 'acme-us',
		domain: 'acme',
		cases: [
			{
				case_id: 7,
				status: 'running',
				opened_at: 1_753_000_000,
				events: [
					{
						kind: 'case_opened',
						case_id: 7,
						at: 1_753_000_000,
						seq: 7,
						text: 'Your case was opened.',
						actor: 'the operator',
						digest: null,
						count: null,
						phase: null
					},
					{
						kind: 'resolution_approved',
						case_id: 7,
						at: 1_753_100_000,
						seq: 42,
						text: 'A supervisor approved the resolution. Receipt: a1b2.',
						actor: 'a supervisor',
						digest: 'a'.repeat(64),
						count: null,
						phase: null
					}
				]
			}
		]
	};
}

async function renderRoute(): Promise<void> {
	const module = await import('../src/routes/journey/+page.svelte');
	render(module.default);
}

async function waitForTestId(testId: string): Promise<void> {
	await waitFor(
		() => {
			// queryAllBy: a journey renders MANY events under one testid — a
			// plural proof, not a unique-element lookup.
			expect(
				screen.queryAllByTestId(testId).length,
				`the render proof "${testId}" never appeared — a surface that renders nothing must FAIL here, never read as an empty journey`
			).toBeGreaterThan(0);
		},
		{ timeout: 3000 }
	);
}

async function loadClient(name: string, caseId = ''): Promise<void> {
	const { fireEvent } = await import('@testing-library/svelte');
	const clientInput = screen.queryByTestId('journey-client-input');
	expect(clientInput).not.toBeNull();
	await fireEvent.input(clientInput!, { target: { value: name } });
	if (caseId !== '') {
		const caseInput = screen.queryByTestId('journey-case-input');
		expect(caseInput).not.toBeNull();
		await fireEvent.input(caseInput!, { target: { value: caseId } });
	}
	const button = screen.queryByTestId('journey-load');
	expect(button).not.toBeNull();
	await fireEvent.click(button!);
}

test('journey_live_render_shows_the_kernel_words_verbatim', async () => {
	responder = async (url) => {
		if (url.includes('/clients/acme-us/journey')) return jsonResponse(journeyBody());
		return jsonResponse({ error: 'unexpected' }, 404);
	};
	vi.stubGlobal('fetch', fetchMock);
	await renderRoute();
	await loadClient('acme-us');
	await waitForTestId('journey-event');
	// J1: the kernel's closed sentences render verbatim — no paraphrase:
	// (the proof testids are PER-EVENT — plural lookups throughout)
	const texts = [...screen.getAllByTestId('journey-event-text')].map((el) => el.textContent);
	expect(texts).toContain('Your case was opened.');
	expect(texts).toContain('A supervisor approved the resolution. Receipt: a1b2.');
	// the ROLE actors render (the kernel's own words, not a name):
	const actors = [...screen.getAllByTestId('journey-event-actor')].map((el) => el.textContent);
	expect(actors).toContain('a supervisor');
	expect(actors).not.toContain('api');
	// the receipt digest renders:
	const digests = [...screen.getAllByTestId('journey-event-digest')].map((el) =>
		el.textContent
	);
	expect(digests).toContain('a'.repeat(64));
});

test('journey_offline_render_shows_metadata_only_never_event_text', async () => {
	// Go live ONCE (writes the metadata cache), then cut the wire:
	responder = async (url) => {
		if (url.includes('/clients/acme-us/journey')) return jsonResponse(journeyBody());
		return jsonResponse({ error: 'unexpected' }, 404);
	};
	vi.stubGlobal('fetch', fetchMock);
	await renderRoute();
	await loadClient('acme-us');
	await waitForTestId('journey-event');
	cleanup();

	responder = noWire;
	await renderRoute();
	await loadClient('acme-us');
	await waitForTestId('journey-offline');
	// The named offline state carries the age and the metadata-only law:
	expect(screen.queryByTestId('journey-offline-metadata-only')).not.toBeNull();
	expect(screen.queryByTestId('journey-offline-count')).not.toBeNull();
	// J2: NO event text renders offline — the cache cannot hold a sentence:
	expect(screen.queryByTestId('journey-events')).toBeNull();
	expect(screen.queryByTestId('journey-event')).toBeNull();
	expect(document.body.textContent).not.toContain('Your case was opened.');
	expect(document.body.textContent).not.toContain('A supervisor approved the resolution.');
});

test('journey_poisoned_cache_with_event_text_renders_nothing_text_shaped', async () => {
	// The validating read's drill: a cache entry that somehow carries
	// text-shaped fields contributes only the fields the module knows —
	// and the offline render renders counts only.
	sessionStorage.setItem(
		'stewardmem.journey.metadata',
		JSON.stringify({
			cachedAt: Math.floor(Date.now() / 1000),
			cases: [{ case_id: 7, eventCount: 2 }],
			// the poison (an unknown key carrying a sentence):
			text: 'Your case was opened.',
			events: [{ text: 'A supervisor approved the resolution.' }]
		})
	);
	vi.stubGlobal('fetch', fetchMock);
	responder = noWire;
	await renderRoute();
	await loadClient('acme-us');
	await waitForTestId('journey-offline');
	expect(document.body.textContent).not.toContain('Your case was opened.');
	expect(document.body.textContent).not.toContain('A supervisor approved the resolution.');
});

test('journey_refusal_renders_the_existence_blind_refusal_not_an_empty_journey', async () => {
	responder = async (url) => {
		if (url.includes('/clients/ghost/journey')) {
			return jsonResponse({ error: 'client not found' }, 404);
		}
		return jsonResponse({ error: 'unexpected' }, 404);
	};
	vi.stubGlobal('fetch', fetchMock);
	await renderRoute();
	await loadClient('ghost');
	await waitForTestId('journey-refused');
	expect(screen.queryByTestId('journey-events')).toBeNull();
	const reason = screen.queryByTestId('journey-refused-reason');
	expect(reason).not.toBeNull();
	// J3: the existence-blind law is NAMED — one answer for unknown client,
	// another client's client, a foreign case:
	expect(reason!.textContent).toContain('existence-blind');
});

test('journey_empty_is_a_fact_never_an_offline_collapse', async () => {
	responder = async (url) => {
		if (url.includes('/clients/acme-us/journey')) {
			return jsonResponse({ client: 'acme-us', domain: 'acme', cases: [] });
		}
		return jsonResponse({ error: 'unexpected' }, 404);
	};
	vi.stubGlobal('fetch', fetchMock);
	await renderRoute();
	await loadClient('acme-us');
	await waitForTestId('journey-empty');
	expect(screen.queryByTestId('journey-refused')).toBeNull();
	expect(screen.queryByTestId('journey-offline')).toBeNull();
});

test('journey_surface_is_a_read_no_write_verb_on_the_typed_wire', () => {
	// J4: the page and its api module carry no POST/PUT/DELETE/PATCH on the
	// typed client — the journey is a projection, and a projection writes
	// nothing.
	const page = readFileSync(join(shellRoot, 'src/routes/journey/+page.svelte'), 'utf8');
	const api = readFileSync(join(shellRoot, 'src/lib/api/journey.ts'), 'utf8');
	for (const source of [page, api]) {
		expect(source).not.toMatch(/client\.(POST|PUT|DELETE|PATCH)\(/);
	}
	// and the api module's ONLY client call is the journey GET:
	const calls = [...api.matchAll(/client\.(GET|POST|PUT|DELETE|PATCH)\('([^']+)'/g)].map(
		(m) => `${m[1]} ${m[2]}`
	);
	expect(calls).toEqual(['GET /clients/{name}/journey']);
});

test('journey_census_parity_sweep_and_catalogs_carry_the_route', async () => {
	// J5: the route joined every census-keyed artifact — the exact-set
	// assertions live in the parity/a11y suites (they import the census);
	// this pin holds the five-catalog parity for the journey keys BY NAME.
	const { readdirSync } = await import('node:fs');
	const dir = join(shellRoot, 'src/lib/i18n');
	const files = readdirSync(dir).filter((f) => f.endsWith('.json')).sort();
	expect(files).toEqual(['de.json', 'en.json', 'es.json', 'fr.json', 'nl.json']);
	for (const file of files) {
		const catalog = JSON.parse(readFileSync(join(dir, file), 'utf8')) as Record<string, string>;
		for (const key of [
			'nav.journey',
			'journey.title',
			'journey.subtitle',
			'journey.offline.metadata_only',
			'journey.refused.notfound'
		]) {
			expect(
				typeof catalog[key] === 'string' && catalog[key].length > 0,
				`${file}:${key} missing or empty — paraglide would silently serve English`
			).toBe(true);
		}
	}
});
