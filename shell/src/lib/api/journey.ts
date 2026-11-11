/**
 * THE CUSTOMER JOURNEY — the chain-projected transparency read (IMPROVE15).
 *
 * ## THE LAW THIS MODULE IS BUILT FOR
 *
 * **The journey is a PROJECTION, not a second source.** The kernel's
 * `/clients/{name}/journey` renders the client's cases from the chain
 * through the pure projection (same chain state → byte-identical journey);
 * this module is a typed READ over that wire and adds nothing: no retry
 * that could re-read a moved chain and render two timelines as one truth,
 * no local merge, no derived display state beyond the kernel's own words.
 *
 * **The kernel's words are the ONLY words.** Each event's `text` is the
 * closed customer sentence the kernel rendered (roles, not names; receipts,
 * not scores). The module renders it verbatim — a surface that paraphrased
 * the kernel's customer sentences would be a second vocabulary drifting
 * from the first, which is exactly the false-coverage defect the projection
 * law forbids.
 *
 * ## THE FOUR READS THAT MATTER HERE
 *
 * - `ok` — the kernel answered with the journey (the only state with event
 *   text).
 * - a refusal (401/403/… — the journey's own authz law answers everyone
 *   else with the SAME existence-blind 404) — named, never swallowed.
 * - `offline` — the kernel could not be reached; nothing was read. The
 *   caller renders the named offline state and holds METADATA ONLY (the
 *   III-6 law: metadata never becomes content — no cached event text).
 * - anything else — an answer this module cannot prove. `malformed`.
 */

import { client } from '#lib/api/client.js';
import { faultOf } from '#lib/api/wire-fault.js';
import type { WireFault } from '#lib/api/client.js';

/** One projected journey event, exactly as the kernel rendered it. The
 * fields are the kernel's closed output; `unknown` values make the read
 * malformed rather than guessed at. */
export interface JourneyEvent {
	readonly kind: string;
	readonly case_id: number;
	readonly at: number;
	readonly seq: number;
	readonly text: string;
	readonly actor: string;
	readonly digest: string | null;
	readonly count: number | null;
	readonly phase: string | null;
}

/** One case's journey block. */
export interface JourneyCase {
	readonly case_id: number;
	readonly status: string;
	readonly opened_at: number;
	readonly events: readonly JourneyEvent[];
}

/** The journey response: the client's identity block + one entry per case. */
export interface JourneyResponse {
	readonly client: string;
	readonly domain: string;
	readonly cases: readonly JourneyCase[];
}

export type JourneyRead =
	| { readonly kind: 'ok'; readonly journey: JourneyResponse }
	| { readonly kind: 'refused'; readonly status: number | null; readonly fault: WireFault }
	| { readonly kind: 'offline'; readonly fault: WireFault }
	| { readonly kind: 'malformed' };

const isRecord = (value: unknown): value is Record<string, unknown> =>
	typeof value === 'object' && value !== null;

const isEvent = (value: unknown): value is JourneyEvent => {
	if (!isRecord(value)) return false;
	return (
		typeof value['kind'] === 'string' &&
		typeof value['case_id'] === 'number' &&
		typeof value['at'] === 'number' &&
		typeof value['seq'] === 'number' &&
		typeof value['text'] === 'string' &&
		typeof value['actor'] === 'string' &&
		(value['digest'] === null || typeof value['digest'] === 'string') &&
		(value['count'] === null || typeof value['count'] === 'number') &&
		(value['phase'] === null || typeof value['phase'] === 'string')
	);
};

const isCase = (value: unknown): value is JourneyCase => {
	if (!isRecord(value)) return false;
	return (
		typeof value['case_id'] === 'number' &&
		typeof value['status'] === 'string' &&
		typeof value['opened_at'] === 'number' &&
		Array.isArray(value['events']) &&
		value['events'].every(isEvent)
	);
};

const isJourney = (value: unknown): value is JourneyResponse => {
	if (!isRecord(value)) return false;
	return (
		typeof value['client'] === 'string' &&
		typeof value['domain'] === 'string' &&
		Array.isArray(value['cases']) &&
		value['cases'].every(isCase)
	);
};

/**
 * Read the client's journey. `caseId` narrows to one case (the kernel
 * refuses a foreign or absent one with the same existence-blind 404).
 */
export async function fetchClientJourney(
	name: string,
	caseId?: number
): Promise<JourneyRead> {
	try {
		const result = await client.GET('/clients/{name}/journey', {
			params: { path: { name }, query: caseId === undefined ? {} : { case: caseId } }
		});
		if (result.response.status !== 200) {
			return { kind: 'refused', status: result.response.status, fault: 'unknown' };
		}
		const data: unknown = result.data;
		if (!isJourney(data)) return { kind: 'malformed' };
		return { kind: 'ok', journey: data };
	} catch (error) {
		const fault = faultOf(error);
		if (fault === 'unreachable') return { kind: 'offline', fault };
		return { kind: 'refused', status: null, fault };
	}
}

/**
 * The offline cache's METADATA — the III-6 law's customer-side reuse: what
 * survives offline is counts and ages, never event text. The type carries
 * no field that could hold a sentence.
 */
export interface JourneyCache {
	readonly cachedAt: number;
	readonly cases: readonly { readonly case_id: number; readonly eventCount: number }[];
}

export function toJourneyMetadata(journey: JourneyResponse, nowSeconds: number): JourneyCache {
	return {
		cachedAt: nowSeconds,
		cases: journey.cases.map((c) => ({ case_id: c.case_id, eventCount: c.events.length }))
	};
}
