import { describe, expect, test, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';

/**
 * R147 — the citation-title witness the credential-heal round asked for.
 *
 * The kernel used to store vault titles HTML-ESCAPED at ingest, so the
 * answer card rendered `Green Tea&#x27;s Secret Weapon` verbatim — measured
 * on screen. The kernel now stores the RAW title (escaping is a rendering
 * concern; the HTML emitters already escape), and this pin holds the
 * shell's side of that law: a citation title that carries a RAW apostrophe
 * renders VERBATIM — no decode pass, no corruption, no entity anywhere.
 * The shell renders `citation.title.value` as plain Svelte text
 * interpolation; the pin proves that seam byte-faithful for exactly the
 * class of title the old ingest mangled.
 *
 * The core is stubbed at the IPC seam, exactly as
 * `chat-affordance-a11y.test.ts` stubs it: what is under test is the
 * surface's rendering, not the kernel's storage (that is
 * `tests/ingest_title_pins.rs`).
 */

let nextRecallShape: unknown = null;
let verifyResult: unknown = null;
let forgetResult: unknown = null;

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(async (command: string, args?: Record<string, any>) => {
		if (command === 'kernel_token') return null;
		if (command === 'recall_memory') {
			return { doc: { v: 1, query: 'q', limit: 5, provenance: true }, query_bytes: 1 };
		}
		if (command === 'shape_recall_command') return nextRecallShape;
		if (command === 'verify_claim_command') {
			return {
				doc: { chunk_id: args?.['request'].chunk_id, claim: args?.['request'].claim },
				claim_chars: String(args?.['request'].claim).length
			};
		}
		if (command === 'shape_verify_command') return verifyResult;
		if (command === 'forget_memory') return { doc: { id: String(args?.['request'].id) } };
		if (command === 'shape_forget_command') return forgetResult;
		throw new Error(`unexpected command ${command}`);
	})
}));

// A supported verdict, carrying the ranges the core's own invariant requires
// (a supported body with no ranges is a contract violation on the wire).
const SUPPORTED = {
	state: 'supported',
	chunk_id: 7,
	decision: 'supported',
	supported: true,
	match_ranges: [[0, 30]],
	claim: 'The leaves are steamed before pressing.'
};

const TOMBSTONED = {
	state: 'tombstoned',
	id: '7',
	result: 'tombstoned',
	dsar_certificate: null
};

vi.stubGlobal('__TAURI_INTERNALS__', {});
vi.stubGlobal(
	'fetch',
	vi.fn(async (input: RequestInfo | URL) => {
		const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
		if (url.includes('/chat/turns')) {
			return new Response(
				JSON.stringify({
					error: { code: 'recall_unavailable', message: 'chat_bridge_unavailable — fixture posture' }
				}),
				{ status: 503 }
			);
		}
		return new Response(JSON.stringify({ sessions: [], total: 0 }), { status: 200 });
	})
);

const chatModule = await import('../src/routes/+page.svelte');

const RAW_TITLE = "Green Tea's Secret Weapon";

const ANSWERED = {
	state: 'answered',
	decision: 'ok',
	passages: [
		{
			id: 7,
			title: RAW_TITLE,
			text: 'The leaves are steamed before pressing.',
			snippet: null,
			domain: 'tea',
			score: 0.82,
			valid_from: '2026-03-02',
			valid_to: null,
			flagged: false,
			ingest_kind: null,
			memory_kind: null,
			authority: null
		}
	],
	confidence: 0.82,
	searched: { query: 'green tea', domain: null, limit: 5, domains_searched: [], included_global: null }
};

async function renderAnswered(): Promise<void> {
	nextRecallShape = ANSWERED;
	verifyResult = SUPPORTED;
	forgetResult = TOMBSTONED;
	vi.stubGlobal('__TAURI_INTERNALS__', {});
	vi.stubGlobal(
		'fetch',
		vi.fn(async (input: RequestInfo | URL) => {
			const url =
				typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
			if (url.includes('/chat/turns')) {
				return new Response(
					JSON.stringify({
						error: { code: 'recall_unavailable', message: 'chat_bridge_unavailable — fixture posture' }
					}),
					{ status: 503 }
				);
			}
			return new Response(JSON.stringify({ sessions: [], total: 0 }), { status: 200 });
		})
	);
	render(chatModule.default as any);
	await waitFor(() => expect(screen.getAllByTestId('chat-composer').length).toBeGreaterThan(0));
	await userEvent.type(screen.getByTestId('chat-composer'), 'green tea');
	await userEvent.keyboard('{Meta>}{Enter}{/Meta}');
	await waitFor(() => {
		const turns = screen.queryAllByTestId('chat-turn');
		expect(turns.length).toBeGreaterThan(0);
		expect(turns[turns.length - 1]!.getAttribute('data-rendering')).toBe('answered');
	});
	// Evidence collapses by default; the citation renders inside the
	// disclosure, so it is opened here, exactly as the affordance pins do.
	await userEvent.click(screen.getByTestId('chat-evidence'));
	await waitFor(() =>
		expect(screen.queryAllByTestId('chat-citation').length).toBeGreaterThan(0)
	);
}

describe('the citation title renders byte-faithful', () => {
	test('a_citation_title_with_an_apostrophe_renders_verbatim', async () => {
		await renderAnswered();

		const citation = screen.getAllByTestId('chat-citation')[0]!;
		const text = citation.textContent ?? '';
		expect(text).toContain(RAW_TITLE);
		// No escape form anywhere on the card: the shell neither decodes a
		// legacy entity nor introduces one — the raw bytes in, the raw bytes
		// out (the kernel-side migration owns the legacy decode).
		expect(text).not.toContain('&#x27;');
		expect(text).not.toContain('&#39;');
		expect(text).not.toContain('&amp;');
		cleanup();
	});
});
