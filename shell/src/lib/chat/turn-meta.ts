/**
 * UI1 S2 — the turn's ONE meta row, as a pure token list.
 *
 * THE DESIGN LAW (the round's headline): provenance collapses into ONE
 * compact meta row per turn — the sentences STAY (they are the pins'
 * surface and the honesty), the NOISE goes (three stacked rows, borders,
 * and the permanent hint become one 11px muted row of tokens). This
 * module decides WHICH tokens a turn's row carries; the page renders the
 * same compiled body for every arm that shows one, so a token can never
 * render as a second stacked row by accident — the DOM pin
 * (`the_turn_meta_is_one_row`) counts the row.
 *
 * PURE BY THE MODULE-SPLIT LAW (M3-a): no Svelte, no wire, no i18n. The
 * tokens carry the FACTS (the support label, the tier word, the verdict's
 * counts, the recalled count); the page maps each to its copy and its
 * `title`, because copy is five locales deep and a pure module that
 * embedded it could not be pinned without a locale context.
 *
 * THE OMIT LAW RIDES THE TYPES: `counts` and `sources` are `null` when the
 * turn does not have them, and a null token is ABSENT from the list —
 * never rendered as a zero (falsifier F5: zero is a fact only the
 * kernel's verdict can state).
 */

import type { SupportLabel, TurnRendering } from './turn.js';
import { confidenceWord } from './provenance.js';

/** One token of the meta row. The kind is the discriminant; the page's
 * switch is exhaustive over it (`assertNever`-shaped by the type). */
export type TurnMetaToken =
	| { readonly kind: 'verdict'; readonly support: SupportLabel }
	| { readonly kind: 'tier'; readonly tier: string }
	| { readonly kind: 'counts'; readonly quoted: number; readonly paraphrase: number }
	| {
			readonly kind: 'recalled';
			readonly count: number;
			/** The confidence WORD (plan 06 §2 — never a bare number), or
			 * `null` when the kernel stated none: the same ladder and the
			 * same source the old heading read. */
			readonly confidence: 'high' | 'medium' | 'low' | null;
		  }
	| { readonly kind: 'sources'; readonly count: number };

/**
 * Build the token list for a turn's meta row.
 *
 * ORDER IS THE READING ORDER: the verdict first (the reader asks "can I
 * trust this?" before "where did it come from?"), then the tier word,
 * then the verdict's counts, then the recalled count, then the sources
 * chip. `sources` is the evidence block's count when the turn carries
 * one, `null` when it does not — the chip is a token like the others so
 * the row keeps ONE compiled body for every arm.
 */
export function buildTurnMeta(
	rendering: TurnRendering,
	sources: number | null
): TurnMetaToken[] {
	const tokens: TurnMetaToken[] = [];
	if (rendering.kind === 'generated') {
		if (rendering.support !== null) {
			tokens.push({ kind: 'verdict', support: rendering.support });
		}
		if (rendering.tier !== null) {
			tokens.push({ kind: 'tier', tier: rendering.tier });
		}
		if (rendering.quoted !== null && rendering.paraphrase !== null) {
			tokens.push({ kind: 'counts', quoted: rendering.quoted, paraphrase: rendering.paraphrase });
		}
	} else if (rendering.kind === 'answered') {
		const confidence = confidenceWord(rendering.confidence);
		tokens.push({
			kind: 'recalled',
			count: rendering.passages.length,
			confidence: confidence.kind === 'word' ? confidence.word : null
		});
	}
	if (sources !== null) {
		tokens.push({ kind: 'sources', count: sources });
	}
	return tokens;
}
