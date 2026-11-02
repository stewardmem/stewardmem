import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, test, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { shellRoot } from './fixtures';

/**
 * The workspace import's SURFACE pins (the D3 card's promise, made an
 * affordance). The kernel-side laws are pinned in `migrate_merge.rs`; the
 * tool-side in the Rust import module. What lives HERE is the consent
 * surface's three refuses-to-lie arms:
 *
 *  1. the act's NAME is composed from the report's own numbers — a button
 *     that says one count and lands another is a lie;
 *  2. the reversal is reachable FROM the outcome (one click, not a
 *     settings hunt);
 *  3. the shell has NO second path to the store's bytes — the component
 *     invokes only the four IPC commands and parses nothing but the
 *     tool's report (structural: a source scan for SQL/store access).
 *
 * The IPC is stubbed exactly as `chat-surface.test.ts` stubs it: the stub
 * stands in for the transport, never for the subject.
 */

const componentSource = readFileSync(
	join(shellRoot, 'src/lib/components/settings/import-section.svelte'),
	'utf8'
);

const codeOf = (source: string): string =>
	source
		.split('\n')
		.filter((line) => {
			const t = line.trim();
			return !t.startsWith('//') && !t.startsWith('*') && !t.startsWith('/*');
		})
		.join('\n');

const INVOKED: string[] = [];
let nextAnswer: unknown = null;

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(async (command: string, args?: unknown) => {
		INVOKED.push(args && 'batchId' in (args as object) ? `${command}:${(args as { batchId: string }).batchId}` : command);
		if (command === 'import_workspace_candidates') {
			return [
				{ name: 'brain.db', path: '/home/u/.openclaw/workspace/brain.db', sizeBytes: 197_349_376, modifiedEpoch: 0 },
				{ name: 'brain-import-dom-x.db', path: '/home/u/.openclaw/workspace/brain-import-dom-x.db', sizeBytes: 8192, modifiedEpoch: 0 }
			];
		}
		if (command === 'build_channel') return 'direct';
		return nextAnswer;
	})
}));

vi.mock('$paraglide/messages.js', () => {
	const table = new Proxy({}, { get: (_t, key: string) => {
		// parametrized messages render the KEY plus the param values, so the
		// count the button carries is assertable without the real catalogs.
		return (params?: Record<string, unknown>) => {
			const suffix = params ? ` ${Object.values(params).join(' ')}` : '';
			return `${key}${suffix}`;
		};
	}});
	return { m: table };
});

import ImportSection from '../src/lib/components/settings/import-section.svelte';

const PLANNED_REPORT = {
	verdict: 'planned',
	source: { path: '/home/u/.openclaw/workspace/brain.db' },
	plan: {
		batchId: 'wi-2d414d8d6592a7a5',
		knowledge: {
			sourceTotal: 8745,
			toLand: 8609,
			dedupeLive: 0,
			dedupeTargetTombstone: 0,
			sourceTombstoned: 136,
			withoutContentHash: 0,
			quarantinedToLand: 0
		},
		relationships: { toLand: 21103, skippedChunkNotLanded: 847 }
	}
};

const COMMITTED_ANSWER = {
	state: 'committed',
	report: {
		...PLANNED_REPORT,
		verdict: 'committed',
		outcome: {
			knowledgeLanded: 8609,
			entitiesLanded: 3786,
			relationshipsLanded: 21103,
			knowledgeUniqueConflicts: 0
		}
	},
	exitCode: 0,
	stderrTail: []
};

describe('the import surface', () => {
	beforeEach(() => {
		INVOKED.length = 0;
		nextAnswer = null;
		vi.stubGlobal('__TAURI_INTERNALS__', {});
	});

	test('the_act_button_carries_the_reports_own_count_and_path', async () => {
		nextAnswer = { state: 'planned', report: PLANNED_REPORT, exitCode: 0, stderrTail: [] };
		render(ImportSection);
		const user = userEvent.setup();
		// pick the real store by name
		const picker = await screen.findByTestId('settings-import-picker');
		await screen.findByRole('option', { name: /brain.db/ });
		await user.selectOptions(picker, 'brain.db');
		await user.click(screen.getByTestId('settings-import-read-report'));
		await screen.findByTestId('settings-import-report');
		// the commit button's NAME carries the report's own number + the
		// chosen name (the consent sentence) — not a static label.
		const commit = screen.getByTestId('settings-import-commit');
		const name = commit.textContent ?? '';
		expect(name).toContain('8609');
		expect(name).toContain('brain.db');
		// and clicking it commits with THE REPORT'S OWN batch id
		nextAnswer = COMMITTED_ANSWER;
		await user.click(commit);
		expect(INVOKED).toContain('import_commit:wi-2d414d8d6592a7a5');
	});

	test('the_reversal_is_reachable_from_the_outcome_one_click', async () => {
		const user = userEvent.setup();
		// The outcome exists only after the session's own commit — the flow
		// the user actually performs: pick → report → commit → undo THERE.
		nextAnswer = { state: 'planned', report: PLANNED_REPORT, exitCode: 0, stderrTail: [] };
		render(ImportSection);
		const picker = await screen.findByTestId('settings-import-picker');
		await screen.findByRole('option', { name: /brain.db/ });
		await user.selectOptions(picker, 'brain.db');
		await user.click(await screen.findByTestId('settings-import-read-report'));
		await screen.findByTestId('settings-import-report');
		nextAnswer = COMMITTED_ANSWER;
		await user.click(screen.getByTestId('settings-import-commit'));
		await screen.findByTestId('settings-import-outcome');
		const undo = await screen.findByTestId('settings-import-revert');
		nextAnswer = {
			state: 'reverted',
			report: { ...PLANNED_REPORT, verdict: 'committed' },
			exitCode: 0,
			stderrTail: []
		};
		await user.click(undo);
		await screen.findByTestId('settings-import-reverted');
		expect(INVOKED.some((call) => call.startsWith('import_revert:wi-'))).toBe(true);
	});

	test('do_nothing_is_always_available_and_the_commit_needs_a_report_first', async () => {
		nextAnswer = { state: 'planned', report: PLANNED_REPORT, exitCode: 0, stderrTail: [] };
		render(ImportSection);
		await screen.findByTestId('settings-import-picker');
		// With nothing chosen and no report rendered, no commit control
		// exists at all — the resting state cannot land bytes.
		expect(screen.queryByTestId('settings-import-commit')).toBeNull();
		expect(screen.queryByTestId('settings-import-revert')).toBeNull();
	});

	test('no_second_sql_path_the_component_invokes_only_the_four_commands', () => {
		const code = codeOf(componentSource);
		for (const banned of ['sqlite', 'SELECT ', 'INSERT ', 'brain.db`', 'readFileSync', 'tauri-plugin-sql']) {
			expect(code.includes(banned), `the import section names ${banned} — the shell has no second path to the store's bytes`).toBe(false);
		}
		const invokes = [...code.matchAll(/invoke(?:<[^>]*>)?\(\s*'([^']+)'/g)].map((mm) => mm[1]);
		expect(new Set(invokes)).toEqual(
			new Set(['build_channel', 'import_workspace_candidates', 'import_report', 'import_commit', 'import_revert'])
		);
	});
});
