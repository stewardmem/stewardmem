<script lang="ts">
	// SETTINGS — the full page (the operator's call, 2026-10-30: ChatGPT-
	// desktop shape — a category rail left, content right; minimal text,
	// less is more). The sections are the REGISTRY's (coreSettings), grouped
	// by each section's `category` — the page owns the layout and the
	// selection, the sections own their state, and adding a setting never
	// means editing the frame (the dsh slots law, kept).
	//
	// Reached from: the sidebar foot, StewardMem → Settings… ⌘, (the native
	// menu emits settings://open; the frame navigates), and the ⌘K palette.
	import { m } from '$paraglide/messages.js';
	import { settingsSections } from '#lib/extensions/registry.js';
	import { fetchKernelFacts } from '#lib/api/kernel-facts.js';
	import { onMount } from 'svelte';

	type CategoryId = 'general' | 'advanced' | 'about';

	// UI2 S13 — THREE categories (five became three: the model and rerank
	// status surfaces folded into advanced, the update surface into about).
	const CATEGORIES: ReadonlyArray<{ id: CategoryId; label: string }> = [
		{ id: 'general', label: m['settings.cats.general']() },
		{ id: 'advanced', label: m['settings.cats.advanced']() },
		{ id: 'about', label: m['settings.cats.about']() }
	];

	let active = $state<CategoryId>('general');
	let kernelVersion = $state<string | null>(null);
	let appVersion = $state<string | null>(null);

	onMount(() => {
		void (async () => {
			const read = await fetchKernelFacts();
			if (read.kind === 'ok') kernelVersion = read.facts.version;
		})();
		if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
			void (async () => {
				const { getVersion } = await import('@tauri-apps/api/app');
				appVersion = await getVersion();
			})();
		}
	});

	const sectionsIn = (category: CategoryId) =>
		settingsSections.filter((section) => section.category === category);
</script>

<div class="grid min-h-full w-full grid-cols-1 gap-8 sm:grid-cols-[180px_minmax(0,1fr)]">
	<!-- The category rail: the macOS System-Settings shape. A nav landmark
	     named for the screen reader; the current row carries aria-current. -->
	<nav aria-label={m['settings.title']()} data-testid="settings-rail" class="sm:pt-2">
		<ul class="grid gap-1 sm:sticky sm:top-2">
			{#each CATEGORIES as cat (cat.id)}
				<li>
					<button
						type="button"
						class="w-full rounded-lg px-3 py-1.5 text-left text-sm transition-colors
							{active === cat.id
							? 'bg-sidebar-active font-medium text-foreground'
							: 'text-muted-foreground hover:bg-sidebar-hover hover:text-foreground'}"
						aria-current={active === cat.id ? 'true' : undefined}
						onclick={() => (active = cat.id)}
					>
						{cat.label}
					</button>
				</li>
			{/each}
		</ul>
	</nav>

	<div class="grid min-w-0 content-start gap-8 pb-6">
		{#each CATEGORIES as cat (cat.id)}
			{#if active === cat.id}
				<section class="grid gap-5" aria-label={cat.label}>
					<h2 class="text-lg font-semibold tracking-tight">{cat.label}</h2>
					{#if cat.id === 'about'}
						<!-- UI2 S13 — the About pane: ONE version line (the two-row dl is
						     gone, and so is its em-dash unknown placeholder). A fact
						     that has not answered renders the honest "not reported"
						     word — never a guessed version, never a dash. The category's
						     SECTIONS render below (the update surface's new home); its
						     verdict copy is unchanged. -->
						<p class="text-sm" data-testid="settings-about">
							{m['settings.about.line']({
								app: appVersion ?? m['settings.about.unknown'](),
								kernel: kernelVersion ?? m['settings.about.unknown']()
							})}
						</p>
					{/if}
					{#each sectionsIn(cat.id) as section (section.id)}
						{@const Section = section.component}
						<Section />
					{/each}
					{#if sectionsIn(cat.id).length === 0}
						<p class="text-sm text-muted-foreground">{m['settings.empty']()}</p>
					{/if}
				</section>
			{/if}
		{/each}
	</div>
</div>
