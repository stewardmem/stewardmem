<script lang="ts">
	/**
	 * The Mermaid island (round 18): one fence, one diagram.
	 *
	 * ## SECURITY
	 * mermaid is initialized ONCE with `securityLevel: 'strict'` (the
	 * library's default and the shell's requirement: HTML labels off,
	 * scripts stripped, event handlers bound only through mermaid's own
	 * API). The rendered SVG mounts through a `use:` action — the ONE
	 * documented exception to the shell's no-raw-HTML posture, because a
	 * diagram is a graph visualisation and mermaid's output is an SVG
	 * string by contract. The SVG is GRAMMAR-DERIVED by mermaid's renderer
	 * from the fence source under strict mode; the model's prose never
	 * reaches the DOM through this path.
	 *
	 * ## ERRORS
	 * A syntax error renders a named error block with the message AND the
	 * original source in a collapsed details element — the source is
	 * preserved (copyable, fixable), never swallowed, and never a silent
	 * empty box.
	 *
	 * ## STREAMING
	 * The parent passes `streaming: true` while the turn is still growing;
	 * this component renders the "diagram pending" placeholder then, never
	 * a premature (half-source) render.
	 */
	type MermaidApi = typeof import('mermaid').default;

	let { source, streaming = false }: { source: string; streaming?: boolean } = $props();

	let svgHtml = $state<string | null>(null);
	let errorText = $state<string | null>(null);
	let renderSeq = 0;

	// One initialization for the module: strict is mermaid's DEFAULT and the
	// shell's requirement; startOnLoad:false because the shell renders
	// explicitly (SPA — no onload scan).
	let mermaidApi: MermaidApi | null = null;
	async function ensureInit(): Promise<MermaidApi> {
		if (mermaidApi === null) {
			// The dynamic import: mermaid is ~1.2 MB gzipped of diagram
			// grammars — the whale stays OUT of the initial bundle and loads
			// only when a fence actually declares itself a diagram.
			mermaidApi = (await import('mermaid')).default;
			mermaidApi.initialize({ startOnLoad: false, securityLevel: 'strict' });
		}
		return mermaidApi;
	}

	async function renderDiagram(src: string, seq: number): Promise<void> {
		const mermaid = await ensureInit();
		try {
			// parse() validates WITHOUT rendering — a syntax error becomes a
			// named error block before any DOM mutation.
			await mermaid.parse(src);
			const { svg } = await mermaid.render(`mmd-${seq}`, src);
			if (seq !== renderSeq) return; // a newer source superseded this render
			svgHtml = svg;
			errorText = null;
		} catch (err) {
			if (seq !== renderSeq) return;
			svgHtml = null;
			errorText = err instanceof Error ? err.message : String(err);
		}
	}

	$effect(() => {
		const src = source;
		renderSeq += 1;
		const seq = renderSeq;
		if (streaming) {
			svgHtml = null;
			errorText = null;
			return;
		}
		void renderDiagram(src, seq);
	});
</script>

{#if streaming}
	<div
		class="rounded-lg border border-border/60 bg-surface-2/50 p-3 text-xs text-muted-foreground"
		data-testid="mermaid-pending"
	>
		diagram pending — the stream is still growing
	</div>
{:else if svgHtml !== null}
	<div
		class="mermaid-island overflow-x-auto rounded-lg border border-border/60 bg-white p-2 dark:bg-surface-2/40"
		data-testid="mermaid-render"
		use:setInnerHtml={svgHtml}
	>
	</div>
{:else if errorText !== null}
	<div
		class="grid gap-2 rounded-lg border border-destructive/50 bg-destructive/5 p-3 text-sm"
		data-testid="mermaid-error"
	>
		<p class="font-semibold text-destructive">diagram failed to render</p>
		<p class="text-xs text-muted-foreground">{errorText}</p>
		<details>
			<summary class="cursor-pointer text-xs text-muted-foreground">diagram source</summary>
			<pre class="overflow-x-auto whitespace-pre-wrap text-xs">{source}</pre>
		</details>
	</div>
{/if}

<script module lang="ts">
	// The action: mount mermaid's SVG. The html string is mermaid's own
	// strict-mode output (see the security note above) — the ONE documented
	// innerHTML exception in the shell.
	function setInnerHtml(node: HTMLElement, html: string) {
		node.innerHTML = html;
		return {
			update(newHtml: string) {
				node.innerHTML = newHtml;
			}
		};
	}
</script>
