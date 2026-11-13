<script lang="ts">
	/**
	 * The chat's code-fence renderer (round 18): the ONE component every
	 * fenced block in an answer renders through.
	 *
	 * - `mermaid` fences → the Mermaid island (the actual diagram, strict
	 *   mode; deferred while the turn streams).
	 * - every other fence → highlight.js (core + an explicit language
	 *   registry — the bundle carries only what chat answers actually name)
	 *   mounted through the same documented action exception as the
	 *   mermaid island: the spans are GRAMMAR-DERIVED over escaped text.
	 * - an unknown language renders UN-highlighted but still fenced and
	 *   labelled — the content is never dropped.
	 */
	import Mermaid from './Mermaid.svelte';
	import { chatStreaming } from '#lib/chat/markdown.js';

	type HljsApi = import('highlight.js').HLJSApi;

	let { class: cls = '', codeValue = '', codeLang = '' }:
		{ class?: string; codeValue?: string; codeLang?: string } = $props();
	const lang = $derived(codeLang);
	const code = $derived(codeValue);

	// The dynamic import: hljs + the registered grammars load only when a
	// fence actually names a language — the initial bundle carries none of
	// it. One chunk, all languages (the registry is fixed).
	let hljsPromise: Promise<HljsApi> | null = null;
	async function ensureLanguages(): Promise<HljsApi> {
		if (hljsPromise === null) {
			hljsPromise = (async () => {
				const hljs = (await import('highlight.js/lib/core')).default;
				const langs = {
					bash: await import('highlight.js/lib/languages/bash'),
					css: await import('highlight.js/lib/languages/css'),
					javascript: await import('highlight.js/lib/languages/javascript'),
					json: await import('highlight.js/lib/languages/json'),
					markdown: await import('highlight.js/lib/languages/markdown'),
					python: await import('highlight.js/lib/languages/python'),
					rust: await import('highlight.js/lib/languages/rust'),
					sql: await import('highlight.js/lib/languages/sql'),
					typescript: await import('highlight.js/lib/languages/typescript'),
					xml: await import('highlight.js/lib/languages/xml'),
					yaml: await import('highlight.js/lib/languages/yaml')
				};
				for (const [name, mod] of Object.entries(langs)) {
					hljs.registerLanguage(name, mod.default);
				}
				return hljs;
			})();
		}
		return hljsPromise;
	}

	const streamingCtx = chatStreaming();
	const isMermaid = $derived(lang.toLowerCase() === 'mermaid');
	const knownLang = $derived(lang.toLowerCase());
	let highlighted = $state<string | null>(null);

	$effect(() => {
		if (isMermaid) return;
		void (async () => {
			try {
				const hljs = await ensureLanguages();
				if (knownLang && hljs.getLanguage(knownLang)) {
					highlighted = hljs.highlight(code, { language: knownLang }).value;
				} else {
					highlighted = null; // un-highlighted: the content stays fenced + labelled
				}
			} catch {
				highlighted = null;
			}
		})();
	});
</script>

{#if isMermaid}
	<Mermaid source={code} streaming={streamingCtx.streaming} />
{:else}
	<pre
		class="overflow-x-auto rounded-lg border border-border/60 bg-surface-2/40 p-3 text-xs {cls}"
		data-testid="chat-code-block"
		data-lang={lang}
	>
		{#if highlighted !== null}
			<!-- plugin:highlight — the spans are grammar-derived over escaped text
			     (the documented derived-markup exception; see lib/chat/markdown.ts) -->
			<code class="hljs" use:mountHtml={highlighted}></code>
		{:else}
			<code class="hljs">{code}</code>
		{/if}
	</pre>
{/if}

<script module lang="ts">
	// The action: mount the highlight spans. The html string is hljs's own
	// output — grammar-derived over ESCAPED text (hljs escapes the code it
	// wraps); never the model's own markup. The ONE documented exception,
	// shared with the mermaid island.
	function mountHtml(node: HTMLElement, html: string) {
		node.innerHTML = html;
		return {
			update(newHtml: string) {
				node.innerHTML = newHtml;
			}
		};
	}
</script>
