/**
 * M3-b — the ONE classified wire fault the chrome may show (plan 21 §1, plan
 * 22 R2).
 *
 * ## WHAT THIS MODULE IS
 *
 * The single piece of shared state behind the home page's fault banner. A
 * wire fault is classified when a surface's READ failed at the wire — the
 * transport threw (`WireError.fault`, named by the Rust core's
 * `kernel_wire_status` because the renderer cannot tell a blocked response
 * from a dead port) — and the classification is cleared when the wire is
 * observed healthy again. Nothing else lives here: no polling, no interval,
 * no watchdog. The banner is REACTIVE, and this module is how a fault found
 * by one surface's read becomes the one interrupt another surface renders.
 *
 * ## THE LAWS THIS MODULE OBEYS
 *
 * - **A refusal is not a fault.** A 403 answer means the kernel SPOKE. Only
 *   a wire-level failure (unreachable, origin_not_allowed, no_credential,
 *   …) is ever classified here — the six-state taxonomy's whole point.
 * - **Silent while green.** `null` is the resting state, and a healthy read
 *   clears a stale fault rather than leaving a banner over a live wire.
 * - **One fault, not a history.** The newest classification wins; the banner
 *   names the CURRENT fact, not the last N of them.
 *
 * ## THE SELF-HEAL
 *
 * A credential fault — `unauthorized` (the kernel rejected this app's
 * bearer) or `no_credential` (none configured) — is the one fault class the
 * shell can repair WITHOUT the operator: the Rust core adopts the kernel's
 * own token (read from the kernel's on-disk source under the mode law,
 * PROVEN against the live kernel, and only then written to the keychain —
 * the `kernel_heal_credential` command and its pins). The classification
 * sites all flow through `classifyWireFault`, so the heal lives HERE: one
 * trigger point, every surface covered. A healed wire invalidates the
 * renderer's stale memoized bearer, clears the fault, and the banner the
 * user was about to read is gone because the FACT is. Any other outcome —
 * no source found, a non-credential fault, no Tauri runtime — leaves the
 * banner standing: it still names the live truth, and Restart (the gate
 * flow) remains the manual path. One heal per classification, no loop: a
 * heal that cannot fix the wire is not retried by this module.
 */

import type { WireFault } from '#lib/api/client.js';

let classified = $state<WireFault | null>(null);
let healing = $state(false);

/** The fault the banner should name, or `null` while the wire is healthy. */
export function currentWireFault(): WireFault | null {
	return classified;
}

/** True while a credential self-heal is in flight (the banner says so). */
export function healInFlight(): boolean {
	return healing;
}

/** The two faults a CREDENTIAL can repair — the heal's whole territory. */
function isCredentialFault(fault: WireFault): boolean {
	return fault === 'unauthorized' || fault === 'no_credential';
}

/** Classify a wire-level fault — the named kind, never a guess. */
export function classifyWireFault(fault: WireFault): void {
	classified = fault;
	if (isCredentialFault(fault)) noteUnauthorizedResponse();
}

/** Clear a stale fault — called when a read observed the wire healthy. */
export function clearWireFault(): void {
	classified = null;
}

/**
 * The heal itself: ask the core to repair the credential, and on a healed
 * (or already-healthy) wire, drop the stale memoized bearer and clear the
 * classified fault.
 */
async function healCredential(): Promise<void> {
	healing = true;
	try {
		// The PWA has no core to heal with: the operator owns their kernel's
		// policy there (documented in the README), and the banner stands.
		if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) return;
		const { invoke } = await import('@tauri-apps/api/core');
		const dto = (await invoke<{ outcome: string }>('kernel_heal_credential')) as {
			outcome: string;
		};
		if (dto.outcome !== 'healed' && dto.outcome !== 'not_needed') return;
		// The keychain now holds the kernel's token; the renderer's memoized
		// bearer is still the stale one the kernel rejected. Invalidate, and
		// the next request carries the healed credential.
		const { invalidateBearer } = await import('#lib/api/bootstrap.js');
		await invalidateBearer();
		// Clear ONLY the fault the heal actually repaired: a newer fault
			// classified while the heal ran is the live fact and stays.
		if (classified !== null && isCredentialFault(classified)) classified = null;
	} catch {
		// No core, or the heal failed: the banner stays, still naming the
		// live fact.
	} finally {
		healing = false;
	}
}

/**
 * The 401 CHOKE POINT. A refused read is not a banner fault (the kernel
 * spoke — the taxonomy's law), but a 401 is the credential being stale, and
 * the heal exists precisely for that. Every typed-client read flows through
 * the client's middleware, so a 401 THERE reaches the heal without any
 * surface knowing the heal exists. Guarded: one heal in flight, and at most
 * one ATTEMPT per window — a heal that cannot fix the wire must not turn a
 * busy surface's 401 stream into an invoke storm.
 */
const HEAL_RETRY_WINDOW_MS = 30_000;
let lastHealAttempt = 0;

export function noteUnauthorizedResponse(): void {
	if (healing) return;
	const now = Date.now();
	if (now - lastHealAttempt < HEAL_RETRY_WINDOW_MS) return;
	lastHealAttempt = now;
	void healCredential();
}
