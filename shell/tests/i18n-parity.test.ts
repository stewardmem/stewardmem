import { readFileSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';

/**
 * D9 — the five catalogs (en/de/fr/es/nl, the Dioxus parity set) must
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
