<script lang="ts">
	// The workspace import (the D3 card's "import later from Settings",
	// made an affordance): choose the store BY NAME → the tool's report
	// renders (counts, skips, the batch id) → ONE named act whose label is
	// composed FROM THE REPORT'S OWN NUMBERS → the outcome with the reversal
	// one click away. "Do nothing" is always available: the resting state is
	// the picker, and the commit fires only from the report the user saw.
	//
	// The shell has NO second path to the store's bytes: every store fact on
	// this screen comes from the sidecar's `--import` JSON (the Rust command
	// spawns it; this component parses nothing but that report) — and the
	// candidate list is the detection's metadata-only listing.
	//
	// cfg(not(mas)): the store build has nothing to import from — the
	// channel check renders the section inert there (the commands are
	// compiled out of the mas build).
	import { m } from '$paraglide/messages.js';
	import { onMount } from 'svelte';
	import { formatBytes } from '#lib/detection.js';

	type Candidate = { name: string; path: string; sizeBytes: number; modifiedEpoch: number };
	type ImportReport = {
		verdict: string;
		source?: { path?: string };
		plan?: {
			batchId: string;
			knowledge: {
				toLand: number;
				sourceTotal: number;
				dedupeLive: number;
				dedupeTargetTombstone: number;
				sourceTombstoned: number;
				withoutContentHash: number;
				quarantinedToLand: number;
			};
			relationships: { toLand: number; skippedChunkNotLanded: number };
			entities?: { referencedByLandedEdges: number };
		};
		outcome?: {
			knowledgeLanded: number;
			entitiesLanded: number;
			relationshipsLanded: number;
			knowledgeUniqueConflicts: number;
		};
	};

	type ImportOutcome = {
		state: 'planned' | 'committed' | 'reverted' | 'refused';
		report: ImportReport | null;
		exitCode: number | null;
		stderrTail: string[];
	};

	let supported = $state<boolean | null>(null);
	let candidates = $state<Candidate[]>([]);
	let chosen = $state<string>('');
	let outcome = $state<ImportOutcome | null>(null);
	let busy = $state(false);
	let lastRefusal = $state('');

	/** The artifact disclosure, computed from the listing (never a
	 * hardcoded count — the workspace moved 521 → 538 between rounds): the
	 * 8 KB single-table scraping artifacts are named once, here. */
	let artifactCount = $derived(candidates.filter((c) => c.sizeBytes <= 16 * 1024).length);
	let realStores = $derived(candidates.filter((c) => c.sizeBytes > 16 * 1024));

	function inTauri(): boolean {
		return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
	}

	onMount(async () => {
		if (!inTauri()) return;
		try {
			const { invoke } = await import('@tauri-apps/api/core');
			if ((await invoke('build_channel')) !== 'direct') return;
			supported = true;
			// A null answer from the core is the honest empty list, never a crash.
			candidates = ((await invoke('import_workspace_candidates')) as Candidate[] | null) ?? [];
		} catch {
			// The core did not answer: the section stays inert rather than
			// pretending a workspace it cannot see.
			supported = false;
		}
	});

	async function runReport() {
		if (!chosen || busy) return;
		busy = true;
		lastRefusal = '';
		try {
			const { invoke } = await import('@tauri-apps/api/core');
			const answer = (await invoke('import_report', {
				sourcePath: chosenPath()
			})) as ImportOutcome;
			outcome = answer;
			if (answer.state === 'refused') {
				lastRefusal = answer.stderrTail.at(-1) ?? 'unknown';
			}
		} catch (error) {
			lastRefusal = String(error);
			outcome = null;
		} finally {
			busy = false;
		}
	}

	async function commitImport() {
		const report = outcome?.report;
		if (!report?.plan || busy) return;
		busy = true;
		lastRefusal = '';
		try {
			const { invoke } = await import('@tauri-apps/api/core');
			const answer = (await invoke('import_commit', {
				sourcePath: chosenPath(),
				batchId: report.plan.batchId
			})) as ImportOutcome;
			outcome = answer;
			if (answer.state === 'refused') {
				lastRefusal = answer.stderrTail.at(-1) ?? 'unknown';
			}
		} catch (error) {
			lastRefusal = String(error);
		} finally {
			busy = false;
		}
	}

	async function revertImport() {
		const batch = outcome?.report?.plan?.batchId;
		if (!batch || busy) return;
		busy = true;
		lastRefusal = '';
		try {
			const { invoke } = await import('@tauri-apps/api/core');
			const answer = (await invoke('import_revert', { batchId: batch })) as ImportOutcome;
			outcome = answer;
			if (answer.state === 'refused') {
				lastRefusal = answer.stderrTail.at(-1) ?? 'unknown';
			}
		} catch (error) {
			lastRefusal = String(error);
		} finally {
			busy = false;
		}
	}

	/** The full path exactly as the core listed it — the UI never
	 * reconstructs a path. */
	function chosenPath(): string {
		return candidates.find((candidate) => candidate.name === chosen)?.path ?? '';
	}

	function planSkipLine(report: ImportReport): string {
		const k = report.plan?.knowledge;
		if (!k) return '';
		return [
			k.sourceTombstoned > 0 ? m['settings.import.skips.tombstoned']({ count: k.sourceTombstoned }) : '',
			k.dedupeLive > 0 ? m['settings.import.skips.duplicates']({ count: k.dedupeLive }) : '',
			k.dedupeTargetTombstone > 0
				? m['settings.import.skips.deleted']({ count: k.dedupeTargetTombstone })
				: ''
		]
			.filter(Boolean)
			.join(' · ');
	}
</script>

<div class="grid gap-2" data-testid="settings-import">
	<h3 class="text-sm font-semibold">{m['settings.import.heading']()}</h3>
	{#if supported === null || supported === false}
		<p class="text-sm text-muted-foreground" data-testid="settings-import-inert">
			{m['settings.import.inert']()}
		</p>
	{:else}
		<p class="text-xs text-muted-foreground">{m['settings.import.hint']()}</p>
		{#if candidates.length > 0 && artifactCount > 0}
			<p class="text-xs text-muted-foreground" data-testid="settings-import-disclosure">
				{m['settings.import.disclosure']({ count: artifactCount, real: realStores.length })}
			</p>
		{/if}
		<label class="grid gap-1">
			<span class="text-sm">{m['settings.import.pick']()}</span>
			<select class="text-sm" bind:value={chosen} data-testid="settings-import-picker">
				<option value="">{m['settings.import.none']()}</option>
				{#each realStores as store (store.name)}
					<option value={store.name}>
						{store.name} — {formatBytes(store.sizeBytes)}
					</option>
				{/each}
			</select>
		</label>
		<div class="flex items-center gap-2">
			<button
				type="button"
				class="text-sm"
				disabled={!chosen || busy}
				onclick={runReport}
				data-testid="settings-import-read-report"
			>
				{m['settings.import.read']()}
			</button>
			{#if outcome?.state === 'planned' && outcome.report?.plan}
				<button
					type="button"
					class="text-sm"
					disabled={busy}
					onclick={commitImport}
					data-testid="settings-import-commit"
				>
					{m['settings.import.act']({
						count: outcome.report.plan.knowledge.toLand,
						path: chosen
					})}
				</button>
			{/if}
			{#if outcome?.state === 'committed' && outcome.report?.plan}
				<button
					type="button"
					class="text-sm"
					disabled={busy}
					onclick={revertImport}
					data-testid="settings-import-revert"
				>
					{m['settings.import.undo']()}
				</button>
			{/if}
		</div>
		{#if outcome?.report?.plan}
			<div
				class="rounded-md border border-border p-2 text-xs"
				data-testid="settings-import-report"
			>
				<p class="text-sm">
					{m['settings.import.report.line']({
						total: outcome.report.plan.knowledge.sourceTotal,
						land: outcome.report.plan.knowledge.toLand,
						edges: outcome.report.plan.relationships.toLand
					})}
				</p>
				{#if planSkipLine(outcome.report)}
					<p class="mt-1">{planSkipLine(outcome.report)}</p>
				{/if}
				<p class="mt-1 text-muted-foreground">
					{m['settings.import.batch']({ batch: outcome.report.plan.batchId })}
				</p>
			</div>
		{/if}
		{#if outcome?.state === 'committed'}
			<p class="text-sm text-brand" role="status" data-testid="settings-import-outcome">
				{m['settings.import.done']({
					landed: outcome.report?.outcome?.knowledgeLanded ?? 0
				})}
			</p>
		{/if}
		{#if outcome?.state === 'reverted'}
			<p class="text-sm text-brand" role="status" data-testid="settings-import-reverted">
				{m['settings.import.reverted.line']({ batch: outcome.report?.plan?.batchId ?? '?' })}
			</p>
		{/if}
		{#if lastRefusal}
			<p class="text-sm text-destructive" data-testid="settings-import-refused">
				{m['settings.import.refused']({ reason: lastRefusal })}
			</p>
		{/if}
	{/if}
</div>
