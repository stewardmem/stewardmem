import { readFileSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';

/**
 * D9 — the five catalogs (en/de/fr/es/nl — the set the retired Dioxus
 * client also shipped, kept at five since) must
 * carry EXACTLY the same key sets: a missing key in ANY locale is a red
 * test, not a runtime fallback surprise.
 */

const i18nDir = join(dirname(fileURLToPath(import.meta.url)), '..', 'src', 'lib', 'i18n');
const LOCALES = ['en', 'de', 'fr', 'es', 'nl'] as const;

function keySet(locale: string): string[] {
	const raw = JSON.parse(readFileSync(join(i18nDir, `${locale}.json`), 'utf8')) as Record<
		string,
		unknown
	>;
	return Object.keys(raw).sort();
}

/// The comparison, extracted so the planted-miss control below can run the
/// SAME checker over synthetic catalogs — a check that cannot fail is the
/// one defect a structural pin must never have. The message names the
/// locale and the law: paraglide's fallback map serves the BASE LOCALE for
/// a missing key, so absence is silent at runtime and only this test sees
/// it.
function expectSameKeySet(enKeys: string[], locale: string, keys: string[]): void {
	expect(keys, `locale ${locale} diverges from en — paraglide would silently serve English`).toEqual(
		enKeys
	);
}

test('i18n_catalog_keys_are_parity_complete_en_de_fr_es_nl', () => {
	const files = readdirSync(i18nDir).filter((f) => f.endsWith('.json')).sort();
	expect(files).toEqual(LOCALES.map((l) => `${l}.json`).sort());
	const enKeys = keySet('en');
	expect(enKeys.length).toBeGreaterThan(5);
	for (const locale of LOCALES.slice(1)) {
		expectSameKeySet(enKeys, locale, keySet(locale));
	}
	// And no catalog carries an EMPTY string (a hole in a translation).
	for (const locale of LOCALES) {
		const raw = JSON.parse(readFileSync(join(i18nDir, `${locale}.json`), 'utf8')) as Record<
			string,
			string
		>;
		for (const [key, value] of Object.entries(raw)) {
			expect(typeof value === 'string' && value.length > 0, `${locale}:${key} empty`).toBe(true);
		}
	}
});

/**
 * AA-3 (improve12) — the AI-disclosure key set exists in EVERY locale. The
 * Art 50(1) plain-language interaction statement is a disclosure only if
 * every reader meets it in their own language: with paraglide's base-locale
 * fallback, a missing de/fr/es/nl key compiles green and silently serves
 * English — the kernel's pins read the five catalogs for exactly this key
 * (tests/ai_act_pins.rs), and THIS test is the shell-side law that keeps
 * the whole key set aligned.
 */
const AI_DISCLOSURE_KEYS = ['chat.disclosure.ai_interaction'] as const;

test('ai_disclosure_key_set_exists_in_every_locale', () => {
	for (const locale of LOCALES) {
		const keys = keySet(locale);
		for (const key of AI_DISCLOSURE_KEYS) {
			expect(
				keys.includes(key),
				`${locale} is missing the AI-disclosure key ${key} — the Art 50(1) \
				 statement must exist in all five catalogs or it silently falls back \
				 to English`
			).toBe(true);
		}
	}
});

/**
 * UI2 S3 — THE PLAIN-LANGUAGE LAW (the operator's directive, 2026-11-20):
 * no AI language in the UI, ever. Made structural here so it can never
 * drift back:
 *
 *  - no em-dash (U+2014) and no en-dash (U+2013) in ANY catalog value, any
 *    locale. They were the AI-house-style tell (the sweep reworked ~49 keys
 *    per locale with periods, commas, colons and parentheses);
 *  - no hot-air token (word-boundary, case-insensitive). The list is the
 *    operator's: seamless/robust/leverage/unlock/elevate/streamline and the
 *    rest of the marketing vocabulary. Per-locale cognate lists grow by
 *    NAMED ACT when a translation genuinely needs one (the loanword-list
 *    precedent); they start empty because the measured catalogs carry none.
 */
const HOT_AIR_TOKENS: readonly string[] = [
	'seamless',
	'seamlessly',
	'robust',
	'robustly',
	'leverage',
	'leveraging',
	'unlock',
	'unlocking',
	'elevate',
	'elevating',
	'streamline',
	'streamlining',
	'empower',
	'empowering',
	'revolutionize',
	'revolutionary',
	'game-changing',
	'cutting-edge',
	'bleeding-edge',
	'state-of-the-art',
	'next-generation',
	'world-class',
	'best-in-class',
	'synergy',
	'synergies',
	'holistic',
	'paradigm',
	'effortless',
	'effortlessly',
	'supercharge',
	'transformative',
	'disruptive',
	'disrupting',
	'delve',
	'developments'
] as const;

/** Cognates per non-en locale, EMPTY BY DESIGN: an entry joins only as a
 * named act when a translation genuinely needs the word (the measured
 * five catalogs carried none at the law's landing). */
const HOT_AIR_COGNATES: Readonly<Record<string, readonly string[]>> = {
	de: [],
	fr: [],
	es: [],
	nl: []
};

function dashFailure(locale: string, key: string, value: string): string | null {
	if (value.includes('\u2014') || value.includes('\u2013')) {
		return `${locale}:${key} carries an em/en dash — rework with periods, commas, colons or parentheses (the plain-language law, 2026-11-20)`;
	}
	return null;
}

function hotAirFailure(locale: string, key: string, value: string): string | null {
	const tokens = [...HOT_AIR_TOKENS, ...(HOT_AIR_COGNATES[locale] ?? [])];
	// WORD-BOUNDARY by tokenization (never substring): a "word" is a run of
	// letters/digits joined by inner hyphens, so "Modelverwijzingen" (nl) is
	// ONE word and never trips "delve", while "game-changing" matches its
	// own hyphenated form. Case-insensitive on both sides.
	const words = value.toLowerCase().match(/[\p{L}\p{N}]+(?:-[\p{L}\p{N}]+)*/gu) ?? [];
	const seen = new Set(words);
	for (const token of tokens) {
		if (seen.has(token.toLowerCase())) {
			return `${locale}:${key} carries the hot-air token "${token}" — plain words only (the operator's law, 2026-11-20): "${value}"`;
		}
	}
	return null;
}

test('ui2_the_plain_language_law_holds_in_all_five_catalogs', () => {
	const failures: string[] = [];
	for (const locale of LOCALES) {
		const values = JSON.parse(readFileSync(join(i18nDir, `${locale}.json`), 'utf8')) as Record<
			string,
			string
		>;
		for (const [key, value] of Object.entries(values)) {
			const dash = dashFailure(locale, key, value);
			if (dash !== null) failures.push(dash);
			const hot = hotAirFailure(locale, key, value);
			if (hot !== null) failures.push(hot);
		}
	}
	expect(
		failures,
		'the catalogs carry AI house style or hot-air vocabulary — the UI speaks plain words in short sentences (the operator\'s law)'
	).toEqual([]);
});

/**
 * The control that keeps the plain-language checker honest (the planted-miss
 * discipline the parity checker carries): a synthetic value carrying a dash
 * and one carrying a hot-air token MUST both red, named; a clean value must
 * pass. A checker that cannot fail is the one defect a structural pin must
 * never have.
 */
test('control_a_planted_dash_and_hot_air_token_red_the_plain_language_checker', () => {
	expect(hotAirFailure('en', 'demo.key', 'Seamless sync — everywhere')).toMatch(
		/hot-air token "seamless"/
	);
	expect(
		dashFailure('en', 'demo.key', 'One dash\u2014 two words'),
		'the dash arm must name the em-dash shape it refuses'
	).toMatch(/em\/en dash/);
	// And the checker discriminates rather than reds everything: a plain
	// value, and the MEASURED false-positive class (the nl substring
	// "Modelverwijzingen" contains "delve" — word boundaries must hold).
	expect(hotAirFailure('nl', 'demo.key', 'Modelverwijzingen')).toBeNull();
	expect(dashFailure('en', 'demo.key', 'Plain words. Short sentences.')).toBeNull();
});

/**
 * The control that keeps the parity checker honest: it PLANTS a missing
 * key — a synthetic de catalog that lacks a key en carries, exactly the
 * shape paraglide's fallback hides — and the checker MUST red it, named.
 * An empty result would mean the checker cannot fail: the one defect a
 * structural pin must never have.
 */
test('control_a_planted_missing_locale_key_reds_the_parity_checker', () => {
	const en = ['a', 'b', 'c'];
	const dePlanted = ['a', 'c']; // 'b' missing — silent English at runtime
	expect(() => expectSameKeySet(en, 'de', dePlanted)).toThrow(/diverges from en/);
	// And the same checker passes an aligned pair — it discriminates, it
	// does not red everything.
	expect(() => expectSameKeySet(en, 'de', ['a', 'b', 'c'])).not.toThrow();
});

/**
 * UI2 S4 — the import disclosure is AGREEMENT-FREE: two noun phrases, no
 * verb to disagree with the count. The old copy carried one ("{real} carry
 * real data" read "1 carry" — the grammar bug, in all five locales because
 * they were translations of the same shape). The fix is the plan's default:
 * noun copy, NOT a plural variant (exact-number locals need a newer message
 * format plugin than the tree pins — /generaltranslation/paraglide-js
 * docs/variants.md, fetched 2026-11-20). The pin holds the shape: at
 * real=1 the second fragment names the count and holds NO verb — the verbs
 * listed are the ones the old copy used, per locale, so restoring any of
 * them reds by name.
 */
const IMPORT_DISCLOSURE_VERBS: Readonly<Record<string, readonly string[]>> = {
	en: ['carry', 'carries'],
	de: ['tragen', 'trägt', 'sind'],
	fr: ['portent', 'porte', 'sont'],
	es: ['contienen', 'contiene', 'son'],
	nl: ['bevatten', 'bevat', 'zijn']
};

test('ui2_the_import_disclosure_is_agreement_free_at_every_count', () => {
	for (const locale of LOCALES) {
		const values = JSON.parse(readFileSync(join(i18nDir, `${locale}.json`), 'utf8')) as Record<
			string,
			string
		>;
		const template = values['settings.import.disclosure'];
		expect(template, `${locale} carries the disclosure key`).toBeDefined();
		// Compose the REAL count shape the reviewer saw (tiny artifacts outnumber
		// the real ones) with the count that reds: ONE real file.
		const composed = template
			.replace('{count}', '2')
			.replace('{real}', '1');
		const fragments = composed.split('.').map((s) => s.trim()).filter((s) => s.length > 0);
		expect(
				fragments.length,
				`${locale}: the disclosure is TWO noun phrases (a period between them) — "${composed}"`
		).toBe(2);
		const realFragment = fragments[1] ?? '';
		expect(
				realFragment.startsWith('1'),
				`${locale}: the real-data phrase names its own count first — "${composed}"`
		).toBe(true);
		for (const verb of IMPORT_DISCLOSURE_VERBS[locale] ?? []) {
			expect(
					composed.toLowerCase(),
					`${locale}: the disclosure holds no verb for the count to disagree with (found "${verb}") — "${composed}"`
			).not.toContain(verb);
		}
	}
});

/**
 * UI1 S4 — the composer's posture badge: the honest on-device sentence
 * exists in EVERY catalog, because the badge is a glyph the reader meets
 * with no visible sentence beside it — its `title`/`aria-label` IS the
 * disclosure, and a missing locale would silently serve English for a
 * privacy claim (the exact class the parity law exists for).
 */
test('ui1_the_composer_posture_badge_sentence_exists_in_all_five_catalogs', () => {
	for (const locale of LOCALES) {
		const values = JSON.parse(readFileSync(join(i18nDir, `${locale}.json`), 'utf8')) as Record<
			string,
			string
		>;
		for (const key of ['chat.badge.ondevice', 'chat.badge.ondevice.tip'] as const) {
			const value = values[key];
			expect(
				typeof value === 'string' && value.length > 0,
				`${locale}:${key} must carry the badge sentence — the glyph's tooltip is the disclosure`
			).toBe(true);
		}
		// The tooltip is a SENTENCE (a claim, not a word): it must say more
		// than the badge's own label.
		expect(values['chat.badge.ondevice.tip']!.length).toBeGreaterThan(
			values['chat.badge.ondevice']!.length
		);
	}
});
