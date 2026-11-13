/**
 * The chat's markdown rendering stack (round 18).
 *
 * ## WHY THIS MODULE EXISTS
 *
 * The chat's answers are markdown documents — headings, lists, tables, code,
 * mermaid diagrams — and the shell used to render them as a plain
 * `whitespace-pre-wrap` paragraph. This module wires the ONE renderer the
 * shell uses: `svelte-exmarkdown` (unified/remark parsing, HAST rendered as
 * Svelte COMPONENTS — the `{@html}` ban (Plan 04 §3, eslint) is respected by
 * construction: raw HTML in the source renders as ESCAPED TEXT unless
 * `rehype-raw` is explicitly added, and this stack never adds it).
 *
 * ## THE SECURITY MODEL
 *
 * - The source (a model answer) is UNTRUSTED input. remark-parse + GFM parse
 *   it into HAST; without rehype-raw, inline HTML survives only as `raw`
 *   text nodes that the renderer emits ESCAPED. No model HTML can become DOM.
 * - Code fences render through the custom `code` renderer: a `mermaid` fence
 *   becomes the Mermaid island (mermaid's own `securityLevel: 'strict'` —
 *   HTML labels off, scripts stripped, the default), any other language
 *   renders through highlight.js's core + an explicit language registry.
 * - The two derived-markup mounts (hljs's spans, mermaid's SVG) are
 *   GRAMMAR-DERIVED over escaped text — never the model's own HTML — and
 *   both ride `use:` actions, not the banned `{@html}` directive. This is the
 *   one deliberate, documented exception: a diagram IS a graph visualisation,
 *   not a text block, and mermaid's output is an SVG string by contract.
 *
 * ## THE STREAMING LAW
 *
 * A mermaid fence renders only when the stream says the turn is complete
 * (`streaming: false`): an unclosed fence is a partial definition, and a
 * diagram rendered from half a source is a corruption, not a preview. While
 * streaming, the fence renders as a plain labelled code block.
 */

import { gfmPlugin } from 'svelte-exmarkdown/gfm';
import type { Plugin } from 'svelte-exmarkdown';
import { getContext, setContext } from 'svelte';

/** The streaming flag's CONTEXT: the renderer map is assembled once, so the
 * per-turn `streaming` state cannot ride component props (the renderer gets
 * HAST properties only) — it flows through context instead, reactive getter
 * and all. CodeBlock reads it to defer mermaid renders while the turn grows
 * (the streaming law: a half-source diagram is corruption, not a preview). */
export const CHAT_STREAMING_KEY = Symbol('chat-streaming');

export type ChatStreamingContext = { streaming: boolean };

export function setChatStreaming(ctx: ChatStreamingContext): void {
	setContext(CHAT_STREAMING_KEY, ctx);
}

export function chatStreaming(): ChatStreamingContext {
	return getContext<ChatStreamingContext>(CHAT_STREAMING_KEY) ?? { streaming: false };
}

/** The rehype half of the code plugin: a FENCED code block is a `pre`
 * wrapping a `code` child — stamp the PRE with the code's raw text
 * (`codeValue`) and its language (`codeLang`), so the renderer component
 * receives both as plain props (the children snippet alone is rendered
 * text and cannot be re-parsed). Inline `code` is untouched — it keeps
 * the default inline rendering. */
const rehypeCodeData = () => (tree: unknown) => {
	const walk = (node: unknown): void => {
		if (node === null || typeof node !== 'object') return;
		const n = node as {
			type?: string;
			tagName?: string;
			properties?: Record<string, unknown>;
			children?: Array<unknown>;
		};
		if (n.type === 'element' && n.tagName === 'pre') {
			const codeChild = (n.children ?? []).find(
				(c) =>
					(c as { type?: string; tagName?: string }).type === 'element' &&
					(c as { tagName?: string }).tagName === 'code'
			) as
				| {
						properties?: Record<string, unknown>;
						children?: Array<{ type?: string; value?: unknown }>;
				  }
				| undefined;
			const first = codeChild?.children?.[0];
			if (codeChild && first?.type === 'text' && typeof first.value === 'string') {
				const classNames = Array.isArray(codeChild.properties?.className)
					? (codeChild.properties?.className as string[])
					: [];
				const lang = /language-([\w-]+)/.exec(classNames.join(' '))?.[1] ?? '';
				n.properties = {
					...n.properties,
					codeValue: first.value,
					codeLang: lang
				};
			}
		}
		for (const child of n.children ?? []) walk(child);
	};
	walk(tree);
};

/** The shell's ONE markdown plugin set: GFM (tables, task lists,
 * strikethrough, autolinks) + the code-data stamp. The `code` renderer
 * itself lives in `CodeBlock.svelte` (exported here as a plugin so the
 * component tree is assembled in one place). */
export function chatMarkdownPlugins(codeRenderer: NonNullable<Plugin['renderer']>): Plugin[] {
	return [gfmPlugin(), { rehypePlugin: rehypeCodeData }, { renderer: codeRenderer }];
}
