import { expect, test, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';

// The SvelteKit routing modules are mocked at the boundary: the palette's
// navigation law (goto over resolve) is what this test pins.
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$app/paths', () => ({ resolve: (p: string) => p }));
vi.mock('$app/state', () => ({
	page: { url: new URL('http://localhost/'), params: {} }
}));


// M3-b: AppShell reads the review queue on mount (the badge's wire). The
// host import below is STATIC, so the typed client would capture the real
// `fetch` before any stub could land — the queue read is mocked at the
// module boundary instead, answering an EMPTY queue: this file's subject is
// the palette/token flows, and an absent badge is the neutral chrome state.
// The sidebar frame added two more mount-time reads (the kernel facts and
// the chain verdict); both are mocked the same way, answering nothing.
vi.mock('#lib/queue/client.js', () => ({
	fetchPendingQueue: vi.fn(async () => ({ kind: 'ok', rows: [] }))
}));
vi.mock('#lib/api/kernel-facts.js', () => ({
	fetchKernelFacts: vi.fn(async () => ({
		kind: 'ok',
		facts: { version: null, count: null }
	}))
}));
vi.mock('#lib/api/audit.js', () => ({
	verifyAuditChain: vi.fn(async () => ({ kind: 'refused', status: 403, fault: 'unknown' }))
}));
// UI2 S12 — the palette's Chats group: the session store is mocked at the
// module boundary (the file's own discipline) so the group has a population
// to filter. Every export AppShell imports is provided; `activate` is a spy
// because Enter ACTIVATING the session is the pin's second arm.
vi.mock('#lib/shell/sessions.svelte.js', () => ({
	activate: vi.fn(),
	currentActiveKey: vi.fn(() => null),
	currentSessions: vi.fn(() => [
		{
			session_key: 's-gate',
			label: 'Gate notes',
			turn_count: 2,
			created_at: 1_760_000_000,
			last_turn_at: 1_760_000_000
		},
		{
			session_key: 's-retention',
			label: 'Retention question',
			turn_count: 1,
			created_at: 1_760_000_000,
			last_turn_at: 1_760_000_000
		}
	]),
	fresh: vi.fn(() => 's-fresh'),
	refresh: vi.fn(async () => {}),
	remove: vi.fn(async () => true),
	rename: vi.fn(async () => true),
	sessionGroupOf: vi.fn(() => 'today'),
	currentReadState: vi.fn(() => 'ok'),
	currentTotal: vi.fn(() => 2),
	currentSessionsFault: vi.fn(() => null),
	currentSessionsRefusedStatus: vi.fn(() => null),
	fetchTurns: vi.fn(async () => []),
	mintSessionKey: vi.fn(() => 's-minted'),
	noteFinal: vi.fn()
}));
import { activate as activateSession } from '#lib/shell/sessions.svelte.js';
import { goto } from '$app/navigation';
import AppShellHost from './AppShellHost.svelte';

const PALETTE_PLACEHOLDER = 'Type a command or search…';

beforeEach(() => {
	vi.mocked(goto).mockClear();
});

afterEach(() => {
	vi.unstubAllGlobals();
});

/**
 * command_palette_opens_and_navigates — ⌘K opens the palette (the in-page
 * keyboard law; NO OS global shortcut), the items navigate via goto over
 * resolve, and the settings entry opens the settings panel instead of
 * navigating.
 */
test('command_palette_opens_and_navigates', async () => {
	const user = userEvent.setup();
	render(AppShellHost);

	// ⌘K opens the palette.
	await user.keyboard('{Meta>}{k}{/Meta}');
	const dialog = await screen.findByRole('dialog');
	expect(dialog).toBeTruthy();
	const input = screen.getByPlaceholderText(PALETTE_PLACEHOLDER);
	expect(input).toBeTruthy();

	// Selecting the Memory item navigates (goto over resolve) and closes
	// the palette. The sidebar speaks the same label, so the query is
	// scoped to the dialog.
	await user.type(input, 'Memory');
	await user.click(within(dialog as HTMLElement).getByText('Memory'));
	await waitFor(() => expect(vi.mocked(goto)).toHaveBeenCalledWith('/recall'));
	await waitFor(() =>
		expect(screen.queryByPlaceholderText(PALETTE_PLACEHOLDER)).toBeNull()
	);

	// ⌘K reopens the palette; the Settings entry NAVIGATES (2026-10-30:
	// the settings is a PAGE — the ChatGPT-desktop shape). The sidebar's
	// settings BUTTON shares the accessible name — the palette item is the
	// one inside the dialog.
	await user.keyboard('{Meta>}{k}{/Meta}');
	const reopened = await screen.findByRole('dialog');
	await user.click(within(reopened as HTMLElement).getByText('Settings'));
	await waitFor(() => expect(vi.mocked(goto)).toHaveBeenCalledWith('/settings'));
	await waitFor(() => expect(screen.queryByPlaceholderText(PALETTE_PLACEHOLDER)).toBeNull());
	expect(vi.mocked(goto)).toHaveBeenCalledTimes(2);
});

/**
 * UI2 S12 — the palette's Chats group: the current session list, filtered by
 * label (the palette's own case-insensitive substring), Enter ACTIVATES the
 * session. No new read cadence: the glance refresh already runs at palette
 * open, so the group renders the store the frame already holds.
 */
test('the_palette_chats_group_filters_by_label_and_enter_activates', async () => {
	const user = userEvent.setup();
	render(AppShellHost);

	await user.keyboard('{Meta>}{k}{/Meta}');
	const dialog = await screen.findByRole('dialog');
	const input = within(dialog as HTMLElement).getByPlaceholderText(PALETTE_PLACEHOLDER);

	// A matching label surfaces the row; the non-matching one is filtered
	// out by the palette's own substring filter (the group contributes
	// items, the machine filters).
	await user.type(input, 'gate');
	const hit = await within(dialog as HTMLElement).findByText('Gate notes');
	expect(hit).toBeTruthy();
	expect(within(dialog as HTMLElement).queryByText('Retention question')).toBeNull();

	// Enter activates the SURVIVING item (autohighlight preselects the top
	// result): the session store's activate ran with the row's key and the
	// frame navigated home (the transcript replays there).
	await user.keyboard('{Enter}');
	await waitFor(() => expect(vi.mocked(activateSession)).toHaveBeenCalledWith('s-gate'));
	await waitFor(() => expect(vi.mocked(goto)).toHaveBeenCalledWith('/'));
});
