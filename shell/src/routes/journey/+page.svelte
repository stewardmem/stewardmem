<script lang="ts">
	// IMPROVE15 — the operator journey: the customer-facing chain projection,
	// rendered in the shell. THE PROJECTION LAW, surface-side: this page adds
	// ZERO state — every event is the kernel's own closed sentence read off
	// `/clients/{name}/journey` (the pure projection), rendered verbatim. No
	// paraphrase, no local merge, no derived timeline: a second vocabulary
	// drifting from the kernel's is the false-coverage defect the round's
	// constitution forbids.
	//
	// The states are DISTINCT and never share a rendering — the review
	// queue's machine, reused:
	//   idle    — no client asked yet (the form, nothing else)
	//   loading — the read is in flight
	//   live    — the kernel answered with the journey (the only state with
	//             event text)
	//   empty   — the kernel said the client has no cases (a FACT)
	//   offline — the kernel could not be reached; the last-known METADATA
	//             is named with its age. Metadata only: the cache carries
	//             counts and ids, never a sentence (the III-6 law — the
	//             projection's own words are content, and offline content is
	//             a cached lie about a chain that moved).
	//   refused — the kernel refused (the journey's OWN authz law answers
	//             everyone but the client's own principal + the operator with
	//             the SAME existence-blind 404 — the page shows the refusal,
	//             never a queue)
	//   malformed — an answer this page cannot prove.
	import { onMount } from 'svelte';
	import { m } from '$paraglide/messages.js';
	import { wireFaultMessage } from '#lib/api/wire-fault.js';
	import type { WireFault } from '#lib/api/client.js';
	import {
		fetchClientJourney,
		toJourneyMetadata,
		type JourneyCache,
		type JourneyCase
	} from '#lib/api/journey.js';
	import { Alert } from '#lib/components/ui/alert/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';

	type View =
		| { kind: 'idle' }
		| { kind: 'loading' }
		| { kind: 'live'; journey: readonly JourneyCase[]; client: string; domain: string }
		| { kind: 'empty' }
		| { kind: 'offline'; cached: JourneyCache | null; client: string }
		| { kind: 'refused'; status: number | null; fault: WireFault }
		| { kind: 'malformed' };

	let clientName = $state('');
	let caseFilter = $state('');
	let view = $state<View>({ kind: 'idle' });
	let generation = 0;

	const CACHE_KEY = 'stewardmem.journey.metadata';

	function readCache(): JourneyCache | null {
		if (typeof window === 'undefined') return null;
		try {
			const raw = window.sessionStorage.getItem(CACHE_KEY);
			if (raw === null) return null;
			const parsed: unknown = JSON.parse(raw);
			if (
				typeof parsed !== 'object' ||
				parsed === null ||
				!Array.isArray((parsed as JourneyCache).cases)
			) {
				return null;
			}
			// The validating read: a cache entry contributes only the fields
			// this module knows — and the fields are counts and ids ONLY.
			// Anything text-shaped is dropped by the type itself.
			return parsed as JourneyCache;
		} catch {
			return null;
		}
	}

	function writeCache(cache: JourneyCache): void {
		if (typeof window === 'undefined') return;
		try {
			window.sessionStorage.setItem(CACHE_KEY, JSON.stringify(cache));
		} catch {
			// a full session store is a lost cache, never a lost read
		}
	}

	function ageLabel(cachedAt: number): string {
		const minutes = Math.max(0, Math.floor(Date.now() / 1000 - cachedAt) / 60);
		const whole = Math.floor(minutes);
		if (whole < 1) return m['journey.age.now']();
		return m['journey.age.minutes']({ count: whole });
	}

	function refusalMessage(status: number | null, fault: WireFault): string {
		if (status === 401) return m['journey.refused.unauthorized']();
		if (status === 404) return m['journey.refused.notfound']();
		if (status !== null) return m['journey.refused.status']({ status: String(status) });
		return wireFaultMessage(fault);
	}

	async function load(): Promise<void> {
		const name = clientName.trim().toLowerCase();
		if (name === '') {
			view = { kind: 'idle' };
			return;
		}
		const current = ++generation;
		view = { kind: 'loading' };
		const parsedCase = caseFilter.trim() === '' ? undefined : Number(caseFilter.trim());
		if (caseFilter.trim() !== '' && (parsedCase === undefined || Number.isNaN(parsedCase))) {
			view = { kind: 'malformed' };
			return;
		}
		const read = await fetchClientJourney(name, parsedCase);
		if (current !== generation) return;
		if (read.kind === 'ok') {
			if (read.journey.cases.length === 0) {
				view = { kind: 'empty' };
				return;
			}
			writeCache(toJourneyMetadata(read.journey, Math.floor(Date.now() / 1000)));
			view = {
				kind: 'live',
				journey: read.journey.cases,
				client: read.journey.client,
				domain: read.journey.domain
			};
			return;
		}
		if (read.kind === 'offline') {
			view = { kind: 'offline', cached: readCache(), client: name };
			return;
		}
		if (read.kind === 'refused') {
			view = { kind: 'refused', status: read.status, fault: read.fault };
			return;
		}
		view = { kind: 'malformed' };
	}

	function submit(event: SubmitEvent): void {
		event.preventDefault();
		void load();
	}

	// SSR guard: the shell renders client-side only.
	onMount(() => {});
</script>

<div class="grid gap-5" data-testid="journey-surface">
	<div>
		<h1>{m['journey.title']()}</h1>
		<p class="text-muted-foreground">{m['journey.subtitle']()}</p>
	</div>

	<p class="text-xs text-muted-foreground" data-testid="journey-disclaimer">
		{m['journey.disclaimer']()}
	</p>

	<form class="flex flex-wrap items-end gap-3" onsubmit={submit} data-testid="journey-form">
		<label class="grid gap-1 text-sm">
			<span class="text-muted-foreground">{m['journey.client.label']()}</span>
			<Input
				bind:value={clientName}
				placeholder={m['journey.client.placeholder']()}
				data-testid="journey-client-input"
			/>
		</label>
		<label class="grid gap-1 text-sm">
			<span class="text-muted-foreground">{m['journey.case.label']()}</span>
			<Input
				bind:value={caseFilter}
				placeholder={m['journey.case.placeholder']()}
				data-testid="journey-case-input"
			/>
		</label>
		<Button type="submit" size="sm" data-testid="journey-load">{m['journey.load']()}</Button>
	</form>

	{#if view.kind === 'idle'}
		<p role="status" data-testid="journey-idle">{m['journey.idle']()}</p>
	{:else if view.kind === 'loading'}
		<p aria-busy="true" data-testid="journey-loading">{m['journey.loading']()}</p>
	{:else if view.kind === 'refused'}
		<Alert variant="destructive" role="alert" data-testid="journey-refused">
			<p class="font-medium">{m['journey.refused.title']()}</p>
			<p data-testid="journey-refused-reason">{refusalMessage(view.status, view.fault)}</p>
		</Alert>
		<Button variant="outline" size="sm" onclick={() => void load()} data-testid="journey-retry">
			{m['journey.retry']()}
		</Button>
	{:else if view.kind === 'malformed'}
		<Alert variant="destructive" role="alert" data-testid="journey-malformed">
			{m['journey.malformed']()}
		</Alert>
		<Button variant="outline" size="sm" onclick={() => void load()} data-testid="journey-retry">
			{m['journey.retry']()}
		</Button>
	{:else if view.kind === 'empty'}
		<p role="status" data-testid="journey-empty">{m['journey.empty']()}</p>
	{:else if view.kind === 'offline'}
		<Alert role="status" data-testid="journey-offline">
			<p class="font-medium">{m['journey.offline.title']()}</p>
			{#if view.cached === null}
				<p data-testid="journey-offline-nocache">{m['journey.offline.nocache']()}</p>
			{:else}
				<p data-testid="journey-offline-age">
					{m['journey.offline.age']({ when: ageLabel(view.cached.cachedAt) })}
				</p>
				<p data-testid="journey-offline-metadata-only">
					{m['journey.offline.metadata_only']()}
				</p>
				<p role="status" data-testid="journey-offline-count">
					{m['journey.count.cases']({ count: view.cached.cases.length })}
				</p>
			{/if}
		</Alert>
	{:else if view.kind === 'live'}
		<p class="text-xs text-muted-foreground" data-testid="journey-scope">
			{m['journey.scope']({ client: view.client, domain: view.domain })}
		</p>
		{#each view.journey as journeyCase (journeyCase.case_id)}
			<section
				class="grid gap-3 rounded-lg border p-4"
				data-testid="journey-case"
				data-case-id={journeyCase.case_id}
			>
				<div class="flex flex-wrap items-center gap-2">
					<h2 class="text-base font-semibold" data-testid="journey-case-title">
						{m['journey.case.title']({ id: String(journeyCase.case_id) })}
					</h2>
					<span
						class="text-xs text-muted-foreground"
						data-testid="journey-case-status">{journeyCase.status}</span
					>
				</div>
				<ol class="grid gap-2" data-testid="journey-events">
					{#each journeyCase.events as event (event.seq)}
						<li
							class="grid gap-1 rounded-md border p-3"
							data-testid="journey-event"
							data-kind={event.kind}
						>
							<p class="text-sm" data-testid="journey-event-text">{event.text}</p>
							<p class="text-xs text-muted-foreground" data-testid="journey-event-actor">
								{event.actor}
							</p>
							{#if event.digest !== null}
								<p class="break-all font-mono text-xs" data-testid="journey-event-digest">
									{event.digest}
								</p>
							{/if}
						</li>
					{/each}
				</ol>
			</section>
		{/each}
	{/if}
</div>
