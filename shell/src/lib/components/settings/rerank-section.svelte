<script lang="ts">
	// CG-12: the rerank tier's fact surface + its OWNED choice. The kernel
	// owns the truth (its `/health/db` echo is the honest snapshot of its
	// boot/lazy load decision); this section renders it and names who owns a
	// change. An older kernel that predates the echo reads `not-reported`
	// and the section says so instead of guessing a state (P-10). The READ
	// lives in `#lib/api/kernel-facts.ts` (the route-parity law); the poll
	// runs ONLY while the state is `loading` — 1.5 s cadence, stopped at
	// the terminal states — never a standing request.
	//
	// The WRITE (the choice): a 0600 marker beside the posture marker, the
	// disclosed one-time model download, and a kernel restart — executed by
	// `rerank_set_enabled` on the Rust side. It is offered ONLY for kernels
	// the app OWNS: the direct channel (the store edition compiles the
	// write commands out — nothing can arm the tier in MAS, P-4) and a
	// non-external posture (an external kernel renders its facts read-only
	// and names who owns the change — P-9).
	import { m } from '$paraglide/messages.js';
	import { fetchRerankState, type RerankStateRead } from '#lib/api/kernel-facts.js';
	import { onMount } from 'svelte';

	type Posture = 'app' | 'service' | 'external';

	interface RerankPostureDto {
		chosen: boolean;
		staged: boolean;
		total_bytes: number;
		posture: Posture;
	}

	interface RerankOutcomeDto {
		outcome: string;
		detail?: string | null;
		error?: string | null;
	}

	let read = $state<RerankStateRead>({ state: 'not-reported' });
	let posture = $state<Posture | null>(null);
	let chosen = $state(false);
	let staged = $state(false);
	let disclosedBytes = $state(0);
	let canWrite = $state(false);
	let busy = $state(false);
	let outcome = $state<string | null>(null);
	let mounted = $state(true);

	onMount(() => {
		mounted = true;
		void poll();
		return () => {
			mounted = false;
		};
	});

	// The posture + choice read names who owns a change: `external` kernels
	// are somebody else's — the section stays facts-only for them. The write
	// affordance additionally requires the direct channel (the mas build's
	// write commands do not exist — an invoke would be a refused call, so
	// the affordance is gated on the channel, the run-mode section's own
	// pattern).
	onMount(async () => {
		try {
			const { invoke } = await import('@tauri-apps/api/core');
			if ((await invoke('build_channel')) !== 'direct') return;
			const dto = (await invoke('rerank_posture')) as RerankPostureDto;
			posture = dto.posture;
			chosen = dto.chosen;
			staged = dto.staged;
			disclosedBytes = dto.total_bytes;
			canWrite = dto.posture !== 'external';
		} catch {
			posture = null; // an older core or no Tauri: facts only, never a guess
		}
	});

	async function poll(): Promise<void> {
		while (mounted) {
			const r = await fetchRerankState();
			read = r;
			if (r.state !== 'loading') return; // terminal state — the poll stops
			await new Promise((res) => setTimeout(res, 1500));
		}
	}

	async function setEnabled(enable: boolean): Promise<void> {
		if (!canWrite || busy) return;
		const { invoke } = await import('@tauri-apps/api/core');
		busy = true;
		outcome = null;
		try {
			const result = (await invoke('rerank_set_enabled', { enable })) as RerankOutcomeDto;
			outcome = result.outcome;
			// The facts re-read after the restart: the echo is the REALITY.
			read = await fetchRerankState();
			const { invoke: inv } = await import('@tauri-apps/api/core');
			const dto = (await inv('rerank_posture')) as RerankPostureDto;
			chosen = dto.chosen;
			staged = dto.staged;
		} catch {
			outcome = 'failed';
		} finally {
			busy = false;
		}
	}
</script>

<div class="grid gap-2">
	<h3 class="text-sm font-semibold">{m['settings.rerank.heading']()}</h3>
	<p class="flex items-center gap-2 text-sm" data-testid="settings-rerank-state">
		{#if read.state === 'not-reported'}
			<span class="inline-block size-2 shrink-0 rounded-full bg-muted-foreground/40" aria-hidden="true"></span>
			{m['settings.rerank.not_reported']()}
		{:else if read.state === 'off'}
			<span class="inline-block size-2 shrink-0 rounded-full bg-muted-foreground/40" aria-hidden="true"></span>
			{m['settings.rerank.off']()}
		{:else if read.state === 'loading'}
			<span
				class="inline-block size-3 shrink-0 animate-spin rounded-full border-2 border-muted-foreground/30 border-t-foreground"
				aria-hidden="true"></span>
			{m['settings.rerank.loading']()}
		{:else if read.state === 'ready'}
			<span class="inline-block size-2 shrink-0 rounded-full bg-ok" aria-hidden="true"></span>
			{m['settings.rerank.ready']({ model: read.model ?? m['settings.rerank.fallback_name'](), topN: read.topN })}
		{:else}
			<span class="inline-block size-2 shrink-0 rounded-full bg-destructive" aria-hidden="true"></span>
			{m['settings.rerank.failed']()}
		{/if}
	</p>
	<p class="text-xs text-muted-foreground">{m['settings.rerank.hint']()}</p>
	{#if posture === 'external'}
		<p class="text-xs text-muted-foreground" data-testid="settings-rerank-external">
			{m['settings.rerank.external_hint']()}
		</p>
	{:else if canWrite}
		<div class="grid gap-1" data-testid="settings-rerank-toggle">
			<button
				type="button"
				class="justify-self-start rounded-md border px-2 py-1 text-xs hover:bg-muted disabled:opacity-50"
				disabled={busy}
				onclick={() => setEnabled(!chosen)}
			>
				{chosen
					? m['settings.rerank.action_disable']()
					: m['settings.rerank.action_enable']()}
			</button>
			{#if !chosen && !staged}
				<p class="text-xs text-muted-foreground" data-testid="settings-rerank-disclose">
					{m['settings.rerank.disclose']({ bytes: disclosedBytes })}
				</p>
			{/if}
			{#if outcome}
				<p class="text-xs text-muted-foreground" data-testid="settings-rerank-outcome">
					{outcome === 'restarted'
						? m['settings.rerank.applied']()
						: m['settings.rerank.failed_act']()}
				</p>
			{/if}
		</div>
	{/if}
</div>
