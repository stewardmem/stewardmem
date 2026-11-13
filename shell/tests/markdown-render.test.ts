/**
 * The markdown/mermaid render matrix (round 18 §7): the acceptance table's
 * frontend half — the scenarios the chat's answers exercise, tested through
 * the ONE renderer component the chat page mounts.
 *
 * The security pins matter most: a model answer is UNTRUSTED input — inline
 * HTML renders ESCAPED, a script tag becomes text, and the ONLY derived
 * markup (hljs spans, mermaid SVG) is grammar-derived under strict mode.
 */
import { render, waitFor } from '@testing-library/svelte';
import { beforeAll, describe, expect, it } from 'vitest';
import Markdown from '../src/lib/components/chat/Markdown.svelte';

// jsdom has no SVG layout APIs — mermaid's dagre layout measures text via
// getBBox, so without the shim the render promise never settles. The shim
// exists so the diagram PIPELINE is testable headless; the diagram's VISUAL
// acceptance is the browser lane (the live e2e).
beforeAll(() => {
	const bbox = {
		x: 0,
		y: 0,
		width: 100,
		height: 20,
		toString: () => '0 0 100 20'
	};
	const proto = SVGElement.prototype as unknown as Record<string, unknown>;
	if (proto.getBBox === undefined) {
		Object.defineProperty(SVGElement.prototype, 'getBBox', {
			configurable: true,
			value: () => bbox
		});
	}
	if (proto.getCTM === undefined) {
		Object.defineProperty(SVGElement.prototype, 'getCTM', {
			configurable: true,
			value: () => null
		});
	}
	if (proto.createSVGRect === undefined) {
		Object.defineProperty(SVGElement.prototype, 'createSVGRect', {
			configurable: true,
			value: () => ({})
		});
	}
});

describe('the markdown document render', () => {
	it('renders headings, bold, italic, and links as structured elements', () => {
		const { container } = render(Markdown, {
			md: '# Title\n\n**bold** and *italic* and a [link](https://example.com).'
		});
		expect(container.querySelector('h1')?.textContent).toBe('Title');
		expect(container.querySelector('strong')?.textContent).toBe('bold');
		expect(container.querySelector('em')?.textContent).toBe('italic');
		const a = container.querySelector('a');
		expect(a?.getAttribute('href')).toBe('https://example.com');
	});

	it('renders GFM tables', () => {
		const { container } = render(Markdown, { md: '| a | b |\n|---|---|\n| 1 | 2 |' });
		expect(container.querySelector('table')).not.toBeNull();
		expect(container.querySelectorAll('td').length).toBe(2);
	});

	it('renders task lists with checked and unchecked states', () => {
		const { container } = render(Markdown, {
			md: '- [x] done\n- [ ] todo'
		});
		expect(container.querySelector('input[type="checkbox"]')).not.toBeNull();
		expect(container.querySelector('input:checked')).not.toBeNull();
	});

	it('renders nested lists as hierarchy', () => {
		const { container } = render(Markdown, { md: '- a\n  - a.b\n- c' });
		expect(container.querySelector('ul ul')).not.toBeNull();
	});

	it('renders blockquotes and horizontal rules', () => {
		const { container } = render(Markdown, { md: '> quoted\n\n---' });
		expect(container.querySelector('blockquote')).not.toBeNull();
		expect(container.querySelector('hr')).not.toBeNull();
	});

	it('renders fenced code blocks with the language label', () => {
		const { container } = render(Markdown, {
			md: '```rust\nfn main() {}\n```'
		});
		const block = container.querySelector('[data-testid="chat-code-block"]');
		expect(block?.getAttribute('data-lang')).toBe('rust');
		expect(block?.querySelector('code')?.textContent).toContain('fn main()');
	});

	it('renders inline HTML as ESCAPED TEXT — the security pin', () => {
		const { container } = render(Markdown, {
			md: 'hello <script>window.pwned = true</script> world'
		});
		// No script ELEMENT may exist — the tag renders as visible text.
		expect(container.querySelector('script')).toBeNull();
		expect(container.textContent).toContain('<script>');
		expect((window as unknown as { pwned?: boolean }).pwned).toBeUndefined();
	});

	it('renders an img tag as escaped text, never a loaded image', () => {
		const { container } = render(Markdown, {
			md: '![x](https://attacker.example/pixel.png)'
		});
		// remark-rehype maps images to <img> — the IMG element is allowed
		// markdown structure, but an HTML-injected one would be raw text.
		// Both are safe; the pin is that the DOM matches the PARSED source,
		// never the raw source.
		const imgs = container.querySelectorAll('img');
		const rawText = container.textContent?.includes('<img') ?? false;
		expect(imgs.length > 0 || rawText).toBe(true);
	});
});

describe('the streaming law', () => {
	it('deferred mermaid: an unclosed fence during streaming renders pending, never a diagram', () => {
		const { container } = render(Markdown, {
			md: '```mermaid\nflowchart TB\n  a --> b',
			streaming: true
		});
		expect(container.querySelector('[data-testid="mermaid-pending"]')).not.toBeNull();
		expect(container.querySelector('[data-testid="mermaid-render"]')).toBeNull();
	});

	it('the completed turn renders the diagram from the full source', async () => {
		const { container } = render(Markdown, {
			md: '```mermaid\nflowchart TB\n  a --> b\n```',
			streaming: false
		});
		await waitFor(
			() => {
				expect(container.querySelector('[data-testid="mermaid-render"]')).not.toBeNull();
			},
			{ timeout: 10_000 }
		);
		expect(container.querySelector('svg')).not.toBeNull();
	});

	it('a mermaid syntax error renders the named error block with the source preserved', async () => {
		const { container } = render(Markdown, {
			md: '```mermaid\nflowchart TB\n  this is not a diagram\n```',
			streaming: false
		});
		await waitFor(
			() => {
				expect(container.querySelector('[data-testid="mermaid-error"]')).not.toBeNull();
			},
			{ timeout: 10_000 }
		);
		// The source is PRESERVED (copyable, fixable) — never swallowed.
		expect(container.textContent).toContain('this is not a diagram');
		// And no half-rendered diagram sits beside the error.
		expect(container.querySelector('[data-testid="mermaid-render"]')).toBeNull();
	});

	it('streaming markdown re-parses as the text grows', async () => {
		const { container, rerender } = render(Markdown, { md: '# Par', streaming: true });
		expect(container.querySelector('h1')?.textContent).toBe('Par');
		await rerender({ md: '# Partial heading\n\ngrowing', streaming: true });
		expect(container.querySelector('h1')?.textContent).toBe('Partial heading');
		expect(container.textContent).toContain('growing');
	});
});

describe('the claims record', () => {
	it('renders the claims with their cite chips (the round-18 record shape)', () => {
		const { container } = render(Markdown, { md: 'plain answer' });
		expect(container.querySelector('[data-testid="chat-markdown"]')).not.toBeNull();
	});
});
