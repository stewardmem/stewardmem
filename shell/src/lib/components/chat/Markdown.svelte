<script lang="ts">
	/**
	 * The chat's markdown renderer (round 18): the ONE component assistant
	 * answers render through.
	 *
	 * svelte-exmarkdown (unified/remark → HAST → Svelte COMPONENTS) with the
	 * GFM plugin (tables, task lists, strikethrough, autolinks) and the
	 * shell's code renderer (mermaid fences → the Mermaid island; everything
	 * else → highlight.js). Raw HTML in the source renders as ESCAPED TEXT —
	 * rehype-raw is deliberately absent (the `{@html}` ban, Plan 04 §3).
	 *
	 * `streaming` propagates to the code renderer: a mermaid fence renders
	 * as a "pending" placeholder while the turn grows, and renders for real
	 * only when the turn completes (a half-source diagram is corruption,
	 * not a preview).
	 */
	import Markdown from 'svelte-exmarkdown';
	import { chatMarkdownPlugins, setChatStreaming } from '#lib/chat/markdown.js';
	import CodeBlock from './CodeBlock.svelte';

	let { md, streaming = false }: { md: string; streaming?: boolean } = $props();

	setChatStreaming({
		get streaming() {
			return streaming;
		}
	});

	const plugins = $derived(
		chatMarkdownPlugins({
			pre: CodeBlock
		})
	);
</script>

<div class="chat-markdown grid gap-2 text-sm" data-testid="chat-markdown" data-streaming={streaming}>
	<Markdown md={md} {plugins} />
</div>
