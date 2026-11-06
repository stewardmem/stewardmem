<script lang="ts">
	// CG-12: the rerank tier's FACT surface. The kernel owns the truth (its
	// `/health/db` echo is the honest snapshot of its boot/lazy load
	// decision); this section renders it and names who owns a change —
	// nothing here writes, ever, for a kernel the app does not manage
	// (P-9). An older kernel that predates the echo reads `not-reported`
	// and the section says so instead of guessing a state (P-10). The READ
	// lives in `#lib/api/kernel-facts.ts` (the route-parity law); the poll
	// runs ONLY while the state is `loading` — 1.5 s cadence, stopped at
	// the terminal states — never a standing request.
	import { m } from '$paraglide/messages.js';
	import { fetchRerankState, type RerankStateRead } from '#lib/api/kernel-facts.js';
	import { onMount } from 'svelte';

	type Posture = 'app' | 'service' | 'external';

	let read = $state<RerankStateRead>({ state: 'not-reported' });
	let posture = $state<Posture | null>(null);
	let mounted = $state(true);

	onMount(() => {
		mounted = true;
		void poll();
		return () => {
			mounted = false;
		};
	});

	// The posture read names who owns a change: `external` kernels are
	// somebody else's — the section stays facts-only for them (best-effort:
	// a refused posture read hides the ownership line, never the facts).
	onMount(async () => {
		try {
			const { invoke } = await import('@tauri-apps/api/core');
			const dto = (await invoke('service_posture')) as { posture: Posture };
			posture = dto.posture;
		} catch {
			posture = null;
		}
	});

	async function poll(): Promise<void> {
		while (mounted && read.state === 'not-reported') {
			const r = await fetchRerankState();
			read = r;
			if (r.state !== 'loading') return; // terminal state — the poll stops
			await new Promise((res) => setTimeout(res, 1500));
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
	{/if}
</div>
