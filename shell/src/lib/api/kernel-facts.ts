/**
 * The kernel facts the frame shows: version, corpus count, model.
 *
 * Two reads, both over the typed wire with the shell's own bearer:
 *
 * - `GET /health` is PUBLIC and answers the kernel's version. It is the one
 *   read that can tell "the kernel is there" from "the kernel is not".
 * - `GET /stats` is Read-scoped and answers the corpus count and the model
 *   id. A refusal here is NOT a fault — the kernel spoke — so the count and
 *   the model simply stay unknown and the sidebar says so by omission.
 *
 * The answer is a record of what was observed, fact by fact, with `null`
 * for each fact the kernel did not give. Nothing is guessed: a sidebar that
 * printed "0 rows" over a refused `/stats` would be reading a refusal as an
 * empty store.
 */

import { client } from '#lib/api/client.js';
import { faultOf } from '#lib/api/wire-fault.js';
import type { WireFault } from '#lib/api/client.js';

export interface KernelFacts {
	readonly version: string | null;
	readonly count: number | null;
	readonly model: string | null;
}

/** The retrieval model's boot-lazy posture (bind-first; the S-4 finding). */
export type ModelState = 'loading' | 'ready' | 'failed';

export interface ModelStateRead {
	readonly model: string | null;
	readonly state: ModelState | 'unknown';
	readonly error: string | null;
}

export type KernelFactsRead =
	| { readonly kind: 'ok'; readonly facts: KernelFacts }
	| { readonly kind: 'offline'; readonly fault: WireFault };

function stringOrNull(value: unknown): string | null {
	return typeof value === 'string' && value.length > 0 ? value : null;
}

function countOrNull(value: unknown): number | null {
	return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

/** Read the facts. Offline only when the PUBLIC probe itself cannot be reached. */
export async function fetchKernelFacts(): Promise<KernelFactsRead> {
	let version: string | null = null;
	try {
		const health = await client.GET('/health');
		if (health.data !== undefined) version = stringOrNull(health.data['version']);
	} catch (error) {
		const fault = faultOf(error);
		if (fault === 'unreachable') return { kind: 'offline', fault };
	}

	let count: number | null = null;
	let model: string | null = null;
	try {
		const stats = await client.GET('/stats');
		if (stats.response.status === 200 && stats.data !== undefined) {
			count = countOrNull(stats.data['count']);
			model = stringOrNull(stats.data['model']);
		}
	} catch {
		// A failed stats read leaves the two facts unknown; the version read
		// above already decided whether the kernel is reachable at all.
	}
	return { kind: 'ok', facts: { version, count, model } };
}

/**
 * The retrieval model's boot-lazy posture, from the app's OWN `/health/db`
 * (Admin body — the sidecar's token is the kernel's superuser; the field
 * is additive there, openapi same commit). Read by the settings model
 * section's bounded poll — ONLY while the state is `loading` (1.5 s
 * cadence, stopped at ready/failed), never a standing request.
 *
 * The state is never guessed: `unknown` means this read did not name it
 * (a refused/failed read is a tick the next poll retries), so the section
 * cannot show `ready` over a refusal.
 */
export async function fetchModelState(): Promise<ModelStateRead> {
	try {
		const { data, response } = await client.GET('/health/db');
		if (response.status === 200 && data) {
			const wire = data.model_state;
			if (wire === 'loading' || wire === 'ready' || wire === 'failed') {
				return {
					model: stringOrNull(data['model']),
					state: wire,
					error: stringOrNull(data['model_error'])
				};
			}
		}
	} catch {
		// the tick retries; nothing is guessed
	}
	return { model: null, state: 'unknown', error: null };
}

/** The rerank tier's state echo (CG-12): `{state, model, top_n}` off the
 * admin `/health/db`, the kernel's own honest snapshot of its boot/lazy load
 * decision.
 *
 * `not-reported` is the OLDER-CORE shape: a kernel predating the echo
 * carries no `rerank` field at all, and this read REFUSES to guess — the
 * section renders "not reported by this kernel" instead of inventing a
 * state. A malformed or unknown `state` value reads the same way: an echo
 * this build cannot name is not a state this build may display. */
export type RerankState = 'off' | 'loading' | 'ready' | 'failed';

export type RerankStateRead =
	| { readonly state: RerankState; readonly model: string | null; readonly topN: number }
	| { readonly state: 'not-reported' };

const RERANK_STATES: readonly RerankState[] = ['off', 'loading', 'ready', 'failed'];

export async function fetchRerankState(): Promise<RerankStateRead> {
	try {
		const { data, response } = await client.GET('/health/db');
		if (response.status === 200 && data) {
			const wire = (data as Record<string, unknown>)['rerank'];
			if (wire && typeof wire === 'object') {
				const echo = wire as Record<string, unknown>;
				const state = echo['state'];
				if (typeof state === 'string' && (RERANK_STATES as readonly string[]).includes(state)) {
					return {
						state: state as RerankState,
						model: stringOrNull(echo['model']),
						topN: countOrNull(echo['top_n']) ?? 50
					};
				}
			}
		}
	} catch {
		// the tick retries; nothing is guessed
	}
	return { state: 'not-reported' };
}
