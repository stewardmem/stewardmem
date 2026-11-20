/**
 * UI2 S11 — the session list's DATE-GROUP boundary, in a PLAIN module on
 * purpose: local midnight needs one Date instance, and the Svelte eslint
 * preset forbids mutable Date instances inside components and runes
 * modules (`svelte/prefer-svelte-reactivity`). The boundary is a pure
 * function of the clock, not reactive state — a plain .ts module is the
 * honest home, and the component stays lint-clean without a disable.
 */

/** Local calendar midnight for "now", in Unix seconds. */
export function localMidnightUnix(): number {
	const nowMs = Date.now();
	// getTimezoneOffset() is UTC minus LOCAL, in minutes — local wall time
	// is UTC minus it, so flooring local wall time to a day and converting
	// back gives the local day boundary in UTC seconds.
	const offsetMs = new Date(nowMs).getTimezoneOffset() * 60_000;
	const localNowMs = nowMs - offsetMs;
	const localMidnightMs = Math.floor(localNowMs / 86_400_000) * 86_400_000;
	return Math.floor((localMidnightMs + offsetMs) / 1000);
}
