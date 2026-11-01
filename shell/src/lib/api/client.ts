/**
 * D5 — the typed wire is the ONLY wire. The client is generated from the
 * kernel's openapi.yaml (`pnpm gen:api` → schema.d.ts, drift-gated by the
 * named test); every call is typed against THAT contract — no hand-written
 * endpoint shapes exist anywhere in the shell.
 *
 * Base URL: the kernel origin, default the pinned 127.0.0.1:8765.
 * Overridable (env for the e2e build, the settings panel for the session),
 * but LOOPBACK-ONLY is enforced client-side on EVERY path: a non-loopback
 * override silently falls back to the default (fail-safe, not fail-open).
 */

import createClient, { type Middleware } from 'openapi-fetch';
import type { paths } from './schema';
import { authHeaders } from './token';
import { bootstrapToken } from './bootstrap';
import { noteUnauthorizedResponse } from '#lib/chat/wire-status.svelte.js';

export const DEFAULT_BASE = 'http://127.0.0.1:8765';

/** The wire faults the shell can NAME. A webview failure is opaque — the
 * browser engine reports a blocked response, a rejected bearer and a dead
 * port all as one network failure — so the Rust core asks the kernel what
 * the webview would experience, and the renderer is told which fault it is.
 */
export type WireFault =
	| 'unreachable'
	| 'origin_not_allowed'
	| 'no_credential'
	| 'unauthorized'
	| 'unexpected'
	| 'unknown';

/** A wire failure that carries its NAME. Never a guess: `unknown` is the
 * honest answer when the core cannot be asked. */
export class WireError extends Error {
	readonly fault: WireFault;

	constructor(fault: WireFault) {
		super('wire_error');
		this.name = 'WireError';
		this.fault = fault;
	}
}

const WIRE_FAULTS: readonly WireFault[] = [
	'unreachable',
	'origin_not_allowed',
	'no_credential',
	'unauthorized',
	'unexpected'
];

/** Ask the Rust core which fault the webview is actually hitting. In the PWA
 * there is no core, so the answer is `unknown` and the generic message
 * stands — the operator owns their own kernel's policy there. */
async function diagnoseWire(): Promise<WireFault> {
	try {
		const { invoke } = await import('@tauri-apps/api/core');
		const dto = await invoke<{ kind: string }>('kernel_wire_status');
		return WIRE_FAULTS.includes(dto.kind as WireFault) ? (dto.kind as WireFault) : 'unknown';
	} catch {
		return 'unknown';
	}
}

/** The session origin override's sessionStorage key (settings panel). */
export const SESSION_BASE_KEY = 'shell.apiBase';

function isLoopbackUrl(raw: string): boolean {
	try {
		const url = new URL(raw);
		if (url.protocol !== 'http:' && url.protocol !== 'https:') return false;
		const host = url.hostname;
		return host === '127.0.0.1' || host === 'localhost' || host === '[::1]' || host === '::1';
	} catch {
		return false;
	}
}

function resolveBase(): string {
	const envOverride = import.meta.env['VITE_BRAIN_API_BASE'];
	if (typeof envOverride === 'string' && envOverride.length > 0 && isLoopbackUrl(envOverride)) {
		return envOverride.replace(/\/$/, '');
	}
	try {
		const stored = sessionStorage.getItem(SESSION_BASE_KEY);
		if (stored && isLoopbackUrl(stored)) return stored.replace(/\/$/, '');
	} catch {
		// no sessionStorage (private mode): the env/default posture stands
	}
	return DEFAULT_BASE;
}

let activeBase = resolveBase();

/** The origin the wire currently targets (for display + the settings panel). */
export function apiBase(): string {
	return activeBase;
}

/** Injects the runtime-held bearer (D7) — the token lives in token.ts and
 * nowhere else. */
const bearerMiddleware: Middleware = {
	async onRequest({ request }) {
		// The FIRST read must not race the credential. The layout kicks the
		// bootstrap off, but the keychain read is async and a route's mount
		// read used to beat it — the request went out with NO `Authorization`
		// header, and (the mount effect runs once) nothing retried it.
		// Awaiting the shared, memoized promise makes the credential a
		// PRECONDITION of the first request.
		await bootstrapToken();
		for (const [name, value] of Object.entries(authHeaders())) {
			request.headers.set(name, value);
		}
		return request;
	}
};

/** Names a wire failure instead of collapsing it into one opaque error. The
 * browser engine reports a blocked response, a rejected bearer and a dead
 * port identically; the core asks the kernel which one it is. */
const wireFaultMiddleware: Middleware = {
	async onError() {
		const fault = await diagnoseWire();
		// A credential fault named by the diagnosis is the 401 the response
		// choke point below can never see: the kernel's 401 answer carries
		// no `access-control-allow-origin`, so the webview blocks the
		// response and fetch() REJECTS — `onResponse` never runs (measured
		// 2026-11-01, the credential drill: twelve 401s on the wire, zero
		// heal invocations, the choke point silent). The heal triggers HERE,
		// beside the diagnosis, under the store's own storm guards.
		if (fault === 'unauthorized' || fault === 'no_credential') noteUnauthorizedResponse();
		return new WireError(fault);
	}
};

/**
 * The 401 choke point — the credential self-heal's SECOND trigger. A 401
 * answer is the kernel SPEAKING (a refusal, never a banner fault), but it is
 * also the exact symptom the heal repairs: the keychain bearer went stale
 * against the running kernel. MEASURED CAVEAT (2026-11-01 drill): on the
 * tauri origin this arm is a FALLBACK, not the main path — the kernel's 401
 * carries no CORS header, the webview blocks the response, and this
 * onResponse never runs (the heal fires in the error diagnosis above). It
 * stays for the origin-less PWA context and for a kernel that one day CORSes
 * its refusals. The store guards the storm: one heal in flight, at most one
 * attempt per window. On success the store also invalidates the memoized
 * bearer, so the NEXT request on this client carries the healed credential
 * (surfaces re-read on their existing triggers — Try again, focus,
 * remount).
 */
const healMiddleware: Middleware = {
	onResponse({ response }) {
		if (response.status === 401) noteUnauthorizedResponse();
		return response;
	}
};

/** REQUEST/RESPONSE calls are BOUNDED (20 s) — a kernel that accepts the
 * connection and then stalls must surface as a named abort, not an unbounded
 * spinner. Local-only wire, so the bound is generous; nothing legitimate on
 * this client approaches it (the SSE STREAM is deliberately NOT here: it
 * rides `chat/spine.ts`, where keep-alives are liveness and Stop is the only
 * abort — a named law this middleware must not quietly reach). */
const REQUEST_TIMEOUT_MS = 20_000;

const timeoutMiddleware: Middleware = {
	async onRequest({ request }) {
		return new Request(request, { signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS) });
	}
};

/**
 * The R25 byte-parity capture (R25_PREREG §2): the trace export law binds
 * the downloaded evidence to the EXACT `GET /recall/{trace_id}/trace`
 * response bytes — not a re-serialization. The typed wire stays the ONLY
 * wire (D5): this middleware rides it, cloning the undisturbed response
 * to capture the raw text of the next response TO THE NAMED PATH (opt-in
 * per call site). Bound to the path, not to "the next response": the frame
 * reads the kernel at its own glance moments, and a capture that took
 * whatever answered next handed the export another read's bytes when that
 * landed first — measured on the lane's trace replay. Capture is
 * best-effort: if it cannot run, the caller falls back to the typed parse's
 * own stringify (disclosed) — never an error surface.
 */
let rawTextCapture: { readonly path: string; readonly sink: (text: string) => void } | null = null;

/** Arms the raw-text capture for the next response to `path` on this client. */
export function captureNextRawText(path: string, sink: (text: string) => void): void {
	rawTextCapture = { path, sink };
}

const rawCaptureMiddleware: Middleware = {
	async onResponse({ request, response }) {
		const capture = rawTextCapture;
		if (capture === null) return response;
		if (new URL(request.url).pathname !== capture.path) return response;
		rawTextCapture = null;
		try {
			capture.sink(await response.clone().text());
		} catch {
			// capture unavailable → the caller's disclosed fallback stands
		}
		return response;
	}
};

/** The live typed client. Rebuilt by setApiBase/resetApiBase — ESM live
 * bindings propagate the reassignment to every importer. */
export let client = createClient<paths>({ baseUrl: activeBase });
// Middleware registers imperatively in this openapi-fetch line (0.17).
client.use(bearerMiddleware);
client.use(timeoutMiddleware);
client.use(wireFaultMiddleware);
client.use(healMiddleware);
client.use(rawCaptureMiddleware);

function rebuildClient(base: string): void {
	activeBase = base;
	client = createClient<paths>({ baseUrl: base });
	client.use(bearerMiddleware);
	client.use(timeoutMiddleware);
	client.use(wireFaultMiddleware);
	client.use(healMiddleware);
	client.use(rawCaptureMiddleware);
}

/**
 * The settings panel's origin override. Loopback-enforced (the same rule
 * as resolveBase): a non-loopback URL is REFUSED (returns false) — never
 * silently rewritten, never accepted. Persists to sessionStorage
 * (session-local by design; a fresh app start re-resolves from env/default).
 */
export function setApiBase(raw: string): boolean {
	const base = raw.trim().replace(/\/$/, '');
	if (!isLoopbackUrl(base)) return false;
	if (base !== activeBase) {
		try {
			sessionStorage.setItem(SESSION_BASE_KEY, base);
		} catch {
			// private mode: the override stays live for this session only
		}
		rebuildClient(base);
	}
	return true;
}

/** Drops the session override (back to env/default). */
export function resetApiBase(): void {
	try {
		sessionStorage.removeItem(SESSION_BASE_KEY);
	} catch {
		// nothing stored
	}
	const base = resolveBase();
	if (base !== activeBase) rebuildClient(base);
}
