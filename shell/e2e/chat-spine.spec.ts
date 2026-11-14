import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';

/**
 * M3-d — the streaming composer over the REAL loopback kernel (plan 25 §5's
 * exit proof, the R139 e2e precedent extended to the shell), extended by the
 * cascade lane (plan 18 §1 — R18-9/10/12, the pins the manual S5 observation
 * owed as regression gates).
 *
 * FIVE tests, and neither ever SKIPS:
 *
 * 1. `the_composer_streams_the_machine_real_posture` — the machine's own
 *    posture IS the assertion. With the AFM bridge resolved by the harness,
 *    a sent question grounds (citations), streams (deltas under the rAF
 *    flush), and finals with the on-device tier. WITHOUT the bridge, the
 *    kernel answers `chat_bridge_unavailable` (503) by name, and the SAME
 *    test asserts the D1 fallback instead: the notice + a deterministic
 *    answer. The posture is never skipped — it is what the machine can prove.
 * 2. `the_fallback_posture_renders_a_deterministic_answer` — the fallback is
 *    STAGED deterministically on every machine: the page's POST /chat/turns
 *    is intercepted with the kernel's exact 503 envelope (measured off
 *    `src/handlers/chat.rs` — code `recall_unavailable`, the marker on the
 *    message), and the turn must render the posture notice beside a
 *    DETERMINISTIC /recall answer, with no fault banner (a refusal is an
 *    answer, never a wire fault).
 * 3. `the_unarmed_turn_renders_the_plain_on_device_answer` (R18-12) — the
 *    OFF law's UI half with BOTH witnesses: the page renders the plain
 *    on_device turn while the unarmed kernel's log names the gate
 *    `enabled=false` for that turn and carries NO cascade route line —
 *    the branch the OFF law keeps untaken is witnessed untaken.
 * 4. `the_armed_cascade_turn_renders_with_its_route_witnessed` (R18-9) —
 *    through the page's OWN origin override (the settings panel surface),
 *    the turn reaches the ARMED kernel (BRAIN_CHAT_CASCADE=1, the second
 *    harness kernel on :8809); the kernel log carries the gate-trace line
 *    (`enabled=true degraded=false hits≥1`) for that turn, and the log's
 *    story and the DOM's tier stamp must AGREE — route line ⇒ cascade tier,
 *    on_device ⇒ the LOUD fall-through refusal. A hidden fallback is RED.
 * 5. `the_cascade_answer_renders_as_a_document_with_citations` (R18-10) —
 *    the seeded document memory (heading + GFM table + fenced mermaid)
 *    served by the evidence-as-answer route must render AS A DOCUMENT:
 *    heading, table, the mermaid island's rendered SVG, and the citation
 *    record below — the row the S5 matrix honestly carried as jsdom-only.
 *
 * The kernels under test are the harness's own (e2e/global-setup.ts): the
 * fresh unarmed kernel on 127.0.0.1:8799 and the armed one on :8809, each
 * seeded through its public routes, each streaming its log to a file the
 * specs read. The operator's 8765 service is never touched.
 */

const BRIDGE_PRESENT = Boolean(process.env['E2E_AFM_BRIDGE']);
const kernelUrl = `http://127.0.0.1:${process.env['E2E_KERNEL_PORT'] ?? 8799}`;
const cascadeUrl = `http://127.0.0.1:${process.env['E2E_CASCADE_PORT'] ?? 8809}`;
const kernelLog = process.env['E2E_KERNEL_LOG'];
const cascadeLog = process.env['E2E_CASCADE_KERNEL_LOG'];

/** The kernel-side witness: the teed log file, read whole. A missing file is
 * a HARNESS regression (the tee never ran) and throws — never an empty pass. */
function logLines(path: string | undefined, label: string): string[] {
	if (!path) throw new Error(`the harness did not expose the ${label} log path`);
	try {
		return readFileSync(path, 'utf8').split('\n');
	} catch {
		throw new Error(`the ${label} log file is missing — the harness tee regressed`);
	}
}

/** The LAST `chat cascade gate:` line — the gate logs once per turn handler
 * entry, so the tail of the log is the most recent turn's story. */
function lastGateLine(lines: string[]): string {
	for (let i = lines.length - 1; i >= 0; i--) {
		const line = lines[i];
		if (line !== undefined && line.includes('chat cascade gate:')) return line;
	}
	throw new Error('no cascade gate line in the kernel log — the turn never reached the gate');
}

/** The page's OWN origin override (the settings panel's session-local law,
 * loopback-enforced client-side): open the ADVANCED category (the origin
 * section's pane — the rail is the macOS System-Settings shape; 'Advanced'
 * is the en label, decision.spec.ts's locale-coupling precedent), fill
 * #setting-origin, save through the section's row-mate button, and require
 * the input to show the normalized base — `setApiBase` only rewrites the
 * field on SUCCESS. */
async function overrideOrigin(page: Page, base: string): Promise<void> {
	// The e2e build carries the SEAT posture's base (the S-C base law:
	// paths.base '/app' unless SHELL_BASE= overrides) — the pane's URL is
	// /app/settings, and the out-of-base /settings answers the kit's
	// base-mismatch notice, not the page.
	await page.goto('/app/settings');
	await page.getByTestId('settings-rail').getByRole('button', { name: 'Advanced' }).click();
	// The origin override lives in the pane's FOLDED group (the operator's
	// "the items we don't use need to be folded" — native <details>): open
	// it the way an operator does, by its summary.
	await page.getByRole('region', { name: 'Advanced' }).locator('summary').click();
	const input = page.locator('#setting-origin');
	await input.fill(base);
	await input.locator('..').getByRole('button').click();
	await expect(input).toHaveValue(base);
}

/** Back to the chat page the way the override SURVIVES: the sidebar's own
 * link (a client-side route — the module instance persists, so the saved
 * session-local base stays live). `page.goto('/')` is a FULL reload, and
 * the baked env base wins again at module re-init — the client.ts:82-vs-87
 * law the settings panel's session-local design is built around. */
async function backToChat(page: Page): Promise<void> {
	await page.locator('a[href="/app/"]').first().click();
	await expect(page.getByTestId('chat-composer')).toBeVisible();
}

/** What one direct turn answers — the harness probe for the bridge posture
 * (Node fetch against a harness kernel, like sessions.spec's driveTurn). */
async function directTurn(base: string, message: string): Promise<{ ok: boolean; code: string }> {
	const response = await fetch(`${base}/chat/turns`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ message, locale: 'en' })
	});
	if (!response.ok) {
		let code = `HTTP ${response.status}`;
		try {
			const body = (await response.json()) as { error?: { code?: string } };
			code = body.error?.code ?? code;
		} catch {
			// the status is the fact
		}
		return { ok: false, code };
	}
	const reader = response.body!.getReader();
	const decoder = new TextDecoder();
	let buffer = '';
	let sawFinal = false;
	let sawError = false;
	let done = false;
	while (!done) {
		const { value, done: finished } = await reader.read();
		if (finished) break;
		buffer += decoder.decode(value, { stream: true });
		let boundary: number;
		while ((boundary = buffer.indexOf('\n\n')) >= 0) {
			const frame = buffer.slice(0, boundary);
			buffer = buffer.slice(boundary + 2);
			if (frame.includes('event: final')) sawFinal = true;
			if (frame.includes('event: error')) sawError = true;
			if (sawFinal || sawError) done = true;
		}
	}
	return { ok: sawFinal && !sawError, code: sawError ? 'stream_error' : 'ok' };
}

const QUESTION = 'What does the StewardMem seed memory say?';

test('the_composer_streams_the_machine_real_posture', async ({ page }) => {
	await page.goto('/');

	if (BRIDGE_PRESENT) {
		// ── THE REAL STREAM: grounding, deltas, the on-device tier. ──
		await page.getByTestId('chat-composer').fill(QUESTION);
		await page.getByTestId('chat-send').click();

		// The turn exists, leaves `pending`, and streams: the text node GROWS
		// (≥1 delta rendered) before the terminal replaces it. The node is
		// empty (and Playwright calls an empty node hidden) until the first
		// delta lands — the bridge takes a breath before its first token — so
		// the assert is ATTACHED-then-NON-EMPTY, not merely visible.
		const turn = page.getByTestId('chat-turn').last();
		await expect(turn).toBeVisible();
		const streamText = page.getByTestId('chat-stream-text');
		await expect(streamText).toBeAttached();
		await expect(streamText).not.toHaveText('', { timeout: 60_000 });

		// The grounding card is up (collapsed by default, the M3-b law) — the
		// kernel's grounding event named the seed memory, so the citations
		// count is real. Open it and read the citation ids off the DOM.
		await expect(page.getByTestId('chat-evidence')).toBeVisible();
		await page.getByTestId('chat-evidence').click();
		await expect(page.getByTestId('chat-citation').first()).toBeVisible();

		// The terminal: the turn lands as `generated` (the ANSWER of record)
		// with the tier named — never an error state, never the banner.
		await expect(turn).toHaveAttribute('data-rendering', 'generated', { timeout: 120_000 });
		await expect(page.getByTestId('chat-tier')).toContainText('on_device');
		await expect(page.getByTestId('chat-fault-banner')).toHaveCount(0);
		await expect(page.getByTestId('chat-stream-failed')).toHaveCount(0);

		// The persisted pair (M3-c): the session region reflects the turn —
		// the glance law's fourth trigger fired at the final.
		await expect(page.getByTestId('session-region')).toBeVisible();

		// Leave no rows behind: the turn persisted into the kernel's `default`
		// session (the page had no active key), and the sessions spec asserts
		// EXACT counts against this same harness kernel.
		const cleanup = await fetch(`${kernelUrl}/chat/sessions/default`, { method: 'DELETE' });
		expect(cleanup.ok, 'the drill erases the session it created').toBe(true);
	} else {
		// ── THE MACHINE'S POSTURE: no bridge, so the kernel refuses by name
		//    and D1 renders the fallback. The same assertions plan 25 §5.7
		//    demands of a model-less runner — never skipped. ──
		const probe = await directTurn(kernelUrl, QUESTION);
		expect(probe.ok, 'the model-less posture must refuse by name, not hang').toBe(false);
		expect(probe.code).toBe('recall_unavailable');

		await page.getByTestId('chat-composer').fill(QUESTION);
		await page.getByTestId('chat-send').click();
		await expect(page.getByTestId('chat-fallback-notice')).toBeVisible();
		await expect(page.getByTestId('chat-fallback-notice')).toHaveAttribute(
			'data-posture',
			'bridge_unavailable'
		);
		// A DETERMINISTIC answer beside the notice: the seeded memory answers
		// the question through /recall — an answered turn with evidence, or
		// the store's honest absence. Never the banner; the composer lives.
		const turn = page.getByTestId('chat-turn').last();
		await expect(turn).not.toHaveAttribute('data-rendering', 'pending');
		const rendering = await turn.getAttribute('data-rendering');
		expect(['answered', 'absent', 'abstained']).toContain(rendering);
		await expect(page.getByTestId('chat-fault-banner')).toHaveCount(0);
	}
});

test('the_fallback_posture_renders_a_deterministic_answer', async ({ page }) => {
	// STAGE the posture with the kernel's own 503 envelope — the exact bytes
	// a model-less kernel answers (src/handlers/chat.rs: the code is
	// `recall_unavailable`; the MESSAGE prefix is the marker the spine's
	// posture table classifies on).
	await page.route('**/chat/turns', (route) =>
		route.fulfill({
			status: 503,
			contentType: 'application/json',
			body: JSON.stringify({
				error: {
					code: 'recall_unavailable',
					message: 'chat_bridge_unavailable — no brain-afm binary beside the kernel'
				}
			})
		})
	);

	await page.goto('/');
	await page.getByTestId('chat-composer').fill(QUESTION);
	await page.getByTestId('chat-send').click();

	// THE NOTICE names the posture…
	const notice = page.getByTestId('chat-fallback-notice');
	await expect(notice).toBeVisible();
	await expect(notice).toHaveAttribute('data-posture', 'bridge_unavailable');

	// …and the DETERMINISTIC answer renders beside it: the real kernel's
	// /recall answered the fallback (the route intercept touches only the
	// spine), so the turn is a full answer with its evidence — or the store's
	// honest absence. Never the banner (a refusal is an answer); the composer
		// is alive: typing a new question re-enables Send (the fallback was a
		// turn end, not a lock-up).
	const turn = page.getByTestId('chat-turn').last();
	await expect(turn).not.toHaveAttribute('data-rendering', 'pending');
	const rendering = await turn.getAttribute('data-rendering');
	expect(['answered', 'absent', 'abstained']).toContain(rendering);
	await expect(page.getByTestId('chat-fault-banner')).toHaveCount(0);
	// The composer is ALIVE: the fallback was a turn end, not a lock-up — a
	// new question re-enables Send.
	await page.getByTestId('chat-composer').fill('next question');
	await expect(page.getByTestId('chat-send')).toBeEnabled();
});

// ── THE CASCADE LANE (plan 18 §1) ── the manual S5 observation, converted
// into regression gates. The witnesses travel in PAIRS: the page's rendered
// state AND the kernel's teed log. The laws under the pins: the OFF law
// byte-identical (R18-12), the armed gate's story agrees with the DOM
// (R18-9), and a grounded answer renders as a DOCUMENT (R18-10).

test('the_unarmed_turn_renders_the_plain_on_device_answer', async ({ page }) => {
	await page.goto('/');

	if (BRIDGE_PRESENT) {
		// The in-page witness: the turn POSTS to the UN-ARMED kernel (the
		// baked harness base — no override on this page).
		const turnsPost = page.waitForRequest(
			(request) => request.method() === 'POST' && request.url().endsWith('/chat/turns')
		);
		await page.getByTestId('chat-composer').fill('What does the R32 decision explorer seed say?');
		await page.getByTestId('chat-send').click();
		const request = await turnsPost;
		expect(request.url().startsWith(kernelUrl), 'the unarmed page stays on the harness kernel').toBe(
			true
		);

		// The UI half: the plain on_device answer, no fault chrome.
		const turn = page.getByTestId('chat-turn').last();
		await expect(turn).toHaveAttribute('data-rendering', 'generated', { timeout: 120_000 });
		await expect(page.getByTestId('chat-tier')).toContainText('on_device');
		await expect(page.getByTestId('chat-fault-banner')).toHaveCount(0);
		await expect(page.getByTestId('chat-stream-failed')).toHaveCount(0);

		// THE LOG HALF: the gate SAW the turn and named the OFF posture —
		// and NO route line exists anywhere in this kernel's log, because
		// the OFF branch breaks before route classification (the order law:
		// the gate line precedes the answer cache; the route never runs).
		const lines = logLines(kernelLog, 'unarmed kernel');
		const gate = lastGateLine(lines);
		expect(
			gate,
			'the unarmed kernel logs `enabled=false` on every turn it serves'
		).toContain('enabled=false');
		expect(
			lines.some((line) => line.includes('chat cascade: route')),
			'the OFF law: no cascade route line ever logs on an unarmed kernel'
		).toBe(false);
	} else {
		// Bridge-less machine: no turn ever reaches the gate block (the
		// bridge posture refuses at handler entry) — the named 503 plus a
		// log with NO gate line at all is the OFF posture's own witness.
		const probe = await directTurn(kernelUrl, QUESTION);
		expect(probe.ok, 'the model-less posture must refuse by name, not hang').toBe(false);
		expect(probe.code).toBe('recall_unavailable');
		expect(
			logLines(kernelLog, 'unarmed kernel').some((line) => line.includes('chat cascade gate:')),
			'no bridge, no gate: the turn never reached the cascade block'
		).toBe(false);
	}

	// Leave no rows behind — the drill runs where a session COULD have been
	// created. Bridge-less: every turn refuses at handler entry and persists
	// NOTHING (chat.rs's persistence law — a Failed turn has no path), so the
	// drill's premise ("the session it created") does not exist there; the
	// honest pin instead: the default session was never written, and the
	// DELETE answers 404 by name.
	if (BRIDGE_PRESENT) {
		const cleanup = await fetch(`${kernelUrl}/chat/sessions/default`, { method: 'DELETE' });
		expect(cleanup.ok, 'the drill erases the session it created').toBe(true);
	} else {
		const cleanup = await fetch(`${kernelUrl}/chat/sessions/default`, { method: 'DELETE' });
		expect(
			cleanup.status,
			'a bridge-less machine persists nothing: the default session was never created'
		).toBe(404);
	}
});

test('the_armed_cascade_turn_renders_with_its_route_witnessed', async ({ page }) => {
	// The armed kernel is the harness's SECOND kernel (:8809) — reached
	// through the page's own origin override, the product surface the
	// settings panel serves (loopback-enforced, session-local).
	await overrideOrigin(page, cascadeUrl);
	await backToChat(page);

	if (BRIDGE_PRESENT) {
		// The in-page witness: the turn POSTS to the ARMED kernel.
		const turnsPost = page.waitForRequest(
			(request) => request.method() === 'POST' && request.url().endsWith('/chat/turns')
		);
		await page.getByTestId('chat-composer').fill(QUESTION);
		await page.getByTestId('chat-send').click();
		const request = await turnsPost;
		expect(request.url().startsWith(cascadeUrl), 'the override carries the turn to :8809').toBe(
			true
		);

		// The answer renders honestly — the S5 observation, now a gate.
		const turn = page.getByTestId('chat-turn').last();
		await expect(turn).toBeVisible();
		await expect(turn).toHaveAttribute('data-rendering', 'generated', { timeout: 120_000 });
		await expect(page.getByTestId('chat-evidence')).toBeVisible();
		await expect(page.getByTestId('chat-fault-banner')).toHaveCount(0);
		await expect(page.getByTestId('chat-stream-failed')).toHaveCount(0);

		// THE KERNEL-SIDE WITNESS: the armed gate line — hits ≥ 1 (the
		// grounding fed the coordinator), not degraded.
		const lines = logLines(cascadeLog, 'armed kernel');
		const gate = lastGateLine(lines);
		expect(
			gate,
			'the armed kernel logs `enabled=true degraded=false hits>=1` on a grounded turn'
		).toMatch(/chat cascade gate: enabled=true degraded=false hits=[1-9]/);

		// THE AGREEMENT LAW: the log's story after the gate line and the
		// DOM's tier stamp must AGREE — a cascade tier names its route, an
		// on_device tier names the LOUD fall-through. A hidden fallback is
		// exactly the gap this pin exists to catch.
		const afterGate = lines.slice(lines.lastIndexOf(gate) + 1);
		const tier = (await page.getByTestId('chat-tier').textContent()) ?? '';
		if (tier.includes('deterministic')) {
			expect(
				afterGate.some((line) => line.includes('chat cascade: route answer_from_memory')),
				'the deterministic tier must name its answer_from_memory route in the log'
			).toBe(true);
		} else if (tier.includes('mlx_native')) {
			expect(
				afterGate.some(
					(line) => line.includes('chat cascade: route ') && !line.includes('answer_from_memory')
				),
				'the mlx_native tier must name its writer route in the log'
			).toBe(true);
		} else {
			expect(
				tier.includes('on_device'),
				`an unexpected tier stamp on the armed kernel: ${tier}`
			).toBe(true);
			expect(
				afterGate.some((line) => line.includes('route classification refused')),
				'an on_device turn on the ARMED kernel must carry the LOUD fall-through refusal'
			).toBe(true);
		}
	} else {
		// Bridge-less machine: the ARMED kernel refuses by name like any
		// other — the override surface and the named posture are the pin.
		const probe = await directTurn(cascadeUrl, QUESTION);
		expect(probe.ok, 'the armed kernel must refuse by name without a bridge').toBe(false);
		expect(probe.code).toBe('recall_unavailable');
	}

	// Leave no rows behind — in the ARMED kernel's session, under the same
	// law as the unarmed drill: the erase runs where a session could have
	// been created; a bridge-less machine pins the 404 instead (a refused
	// turn persists nothing).
	if (BRIDGE_PRESENT) {
		const cleanup = await fetch(`${cascadeUrl}/chat/sessions/default`, { method: 'DELETE' });
		expect(cleanup.ok, 'the drill erases the session it created').toBe(true);
	} else {
		const cleanup = await fetch(`${cascadeUrl}/chat/sessions/default`, { method: 'DELETE' });
		expect(
			cleanup.status,
			'a bridge-less machine persists nothing: the default session was never created'
		).toBe(404);
	}
});

test('the_cascade_answer_renders_as_a_document_with_citations', async ({ page }) => {
	await overrideOrigin(page, cascadeUrl);
	await backToChat(page);

	if (BRIDGE_PRESENT) {
		await page.getByTestId('chat-composer').fill('Show the payload seal drill document.');
		await page.getByTestId('chat-send').click();

		const turn = page.getByTestId('chat-turn').last();
		await expect(turn).toHaveAttribute('data-rendering', 'generated', { timeout: 120_000 });
		await expect(page.getByTestId('chat-fault-banner')).toHaveCount(0);

		// THE ROUTE WITNESS: the document row needs the evidence-as-answer
		// route (the answer IS the stored content — only then is the
		// document assertion honest). The coordinator's other routes would
		// generatively REWRITE the document, so this pin names the route it
		// requires and REDS LOUD if the coordinator named another — the
		// honest boundary recorded in the plan's evidence, never a skip.
		const lines = logLines(cascadeLog, 'armed kernel');
		const gate = lastGateLine(lines);
		const afterGate = lines.slice(lines.lastIndexOf(gate) + 1);
		expect(
			gate,
			'the document turn is grounded and armed'
		).toMatch(/chat cascade gate: enabled=true degraded=false hits=[1-9]/);
		expect(
			afterGate.some((line) => line.includes('chat cascade: route answer_from_memory')),
			'the coordinator must route the document question answer_from_memory (the evidence-as-answer route)'
		).toBe(true);
		await expect(page.getByTestId('chat-tier')).toContainText('deterministic');

		// THE DOCUMENT: the stored GFM renders whole — heading, table row,
		// and the mermaid island's rendered SVG (the strict-mode island, the
		// one documented raw-HTML action). The whale loads on demand (the
		// lazy grammar bundle), so the island gets its own generous window.
		await expect(turn.locator('h2', { hasText: 'Payload seal drill' })).toBeVisible();
		await expect(turn.locator('td', { hasText: 'verified' })).toBeVisible();
		const island = page.getByTestId('mermaid-render');
		await expect(island).toBeVisible({ timeout: 60_000 });
		await expect(island.locator('svg')).toBeAttached({ timeout: 60_000 });

		// THE CITATION RECORD BELOW: the grounding card opens and names the
		// document it served — the citation's identifier is the memory's
		// TITLE (the seeded fixture's own), and the support verdict rides
		// the final.
		await page.getByTestId('chat-evidence').click();
		const citation = page.getByTestId('chat-citation').first();
		await expect(citation).toBeVisible();
		await expect(citation).toContainText('Cascade e2e payload drill');
		await expect(page.getByTestId('chat-support')).toHaveAttribute(
			'data-support',
			/supported|partial|unsupported|abstained|degraded/
		);
	} else {
		// Bridge-less machine: the named refusal through the override.
		const probe = await directTurn(cascadeUrl, 'Show the payload seal drill document.');
		expect(probe.ok, 'the armed kernel must refuse by name without a bridge').toBe(false);
		expect(probe.code).toBe('recall_unavailable');
	}

	// The same drill law as its two siblings: erase where a session could
	// have been created; pin the 404 where nothing could persist.
	if (BRIDGE_PRESENT) {
		const cleanup = await fetch(`${cascadeUrl}/chat/sessions/default`, { method: 'DELETE' });
		expect(cleanup.ok, 'the drill erases the session it created').toBe(true);
	} else {
		const cleanup = await fetch(`${cascadeUrl}/chat/sessions/default`, { method: 'DELETE' });
		expect(
			cleanup.status,
			'a bridge-less machine persists nothing: the default session was never created'
		).toBe(404);
	}
});
