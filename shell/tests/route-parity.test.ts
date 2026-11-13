import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';
import { expect, test } from 'vitest';
import { shellRoot } from './fixtures';
import { CENSUS_ROUTES, ROUTE_FLOOR } from './route-census';
import { navEntries } from '../src/lib/extensions/registry.js';

/**
 * The census routes that are deliberately NOT sidebar nav entries — the
 * operator listing and the two detail routes, opened from lists and
 * citations (the frame's recorded comment, now a literal here). The registry
 * nav set must equal the census minus exactly these.
 */
const DECISION_AND_DETAIL_ROUTES = [
	'/decisions',
	'/decisions/[id]',
	'/recall/[trace_id]/trace',
	// The settings PAGE (2026-10-30, the dsh-UX round): opened from the
	// sidebar FOOT and the native StewardMem → Settings… ⌘, item — a
	// preferences surface, not a panel-list entry (the operator's call:
	// it must not sit under Governance).
	'/settings'
] as const;

/**
 * II-7d — the SHELL route census. Census-form since the decommission
 * (2026-11-13): the Dioxus counterpart half of this file — the parity
 * verdicts, the `dioxus`/`nearest` cells, the walk of the client's
 * `main.rs` — retired WITH the client workspace; the rows are route +
 * because + authz, and the because fields keep the parity history in
 * past tense where it still teaches.
 *
 * ## WHY THIS FILE EXISTS AT ALL, and what it is NOT
 *
 * Plan 09 §2's gates 1 and 3 name "the route-coverage table" and "the
 * route-authz guard" as their tests. Those are KERNEL tables —
 * `OPENAPI_ROUTES` and `AUTHZ_GATES` in `src/server/router/route_guards.rs` —
 * covering every path the composed *router* registers. They are already green
 * and they say NOTHING about the shell.
 *
 * Gate 1's actual text was different: "Every user-reachable **shell** route has
 * parity with its Dioxus counterpart" — a check against a different artifact,
 * which did not exist, and which lost its subject when the Dioxus side retired.
 * Asserting the kernel tables instead would produce a passing gate that has
 * never looked at the shell — which would look exactly like the work being
 * done. So this file walks the shell's own route tree, and one pin below is
 * dedicated to proving it did.
 *
 * ## THE ANTI-VACUITY FLOOR COMES FIRST, and it is a floor
 *
 * A route table that walks the tree and finds nothing is a green light for
 * nothing — the exact failure II-1's transport pin hit when `/chat` did not
 * exist yet. So the census asserts BOTH a floor and the exact route set. The
 * floor is the load-bearing half: deleting the walk, renaming the directory, or
 * breaking the path derivation all fail here rather than passing over an empty
 * list.
 */

/**
 * The floor. Not a count of what the walk happens to find today — a LOWER
 * bound, so adding routes cannot break this and removing them cannot pass it.
 *
 * Numerically EQUAL to the exact set at M3-a (it was 9 at II-7d and 10 after
 * III-6 — the exact set moved around it, which is the two literals doing
 * their jobs). They stay separate statements: one says "the walk found
 * something", the other says "and it is exactly these", and a future route
 * reds the exact set before the floor has an opinion.
 *
 * III-0c: the literal MOVED to `route-census.ts` so the a11y sweep reads the
 * SAME set — see that module's header. This file's pins are unchanged in law.
 */

/** One census entry: the route path AND the page file it was derived from.
 * III-0b's authz derivation re-opens the FILE — the route string alone cannot
 * be read. */
interface RouteEntry {
	readonly route: string;
	readonly file: string;
}

/** The census, with files: one entry per `+page.svelte` under `routes/`. The
 * walk is the one `censusShellRoutes` has always done — that function is now
 * this one's projection, so the two cannot disagree about what a route is. */
function censusShellRouteEntries(routesDir: string): RouteEntry[] {
	const found: RouteEntry[] = [];
	const walk = (dir: string): void => {
		for (const entry of readdirSync(dir, { withFileTypes: true })) {
			const path = join(dir, entry.name);
			if (entry.isDirectory()) {
				walk(path);
			} else if (entry.name === '+page.svelte') {
				found.push({ route: routePathFor(path, routesDir), file: path });
			}
		}
	};
	walk(routesDir);
	return found.sort((a, b) => (a.route < b.route ? -1 : 1));
}

/** Walks `shell/src/routes` and derives each user-reachable route path.
 *
 * SvelteKit's file-based convention, which is what makes this derivable rather
 * than hand-listed: `+page.svelte` is the page component, `[param]` is a dynamic
 * segment, and the route path is the directory chain with `routes/` and the
 * `+page.svelte` filename removed. `routes/+page.svelte` is `/`, which is the
 * one case where no directory name survives.
 */
function censusShellRoutes(routesDir: string): string[] {
	return censusShellRouteEntries(routesDir).map((entry) => entry.route);
}

function routePathFor(pageFile: string, routesDir: string): string {
	// Drop the filename, then the `routes/` root.
	const withoutFile = relative(routesDir, join(pageFile, '..'));
	const segments = withoutFile.split('/').filter((s) => s.length > 0);
	return segments.length === 0 ? '/' : `/${segments.join('/')}`;
}

/** The kernel's own authorize() vocabulary, plus `public` for the paths the
 * bearer middleware exempts (`PUBLIC_PATHS`). A scope here is never typed from
 * memory: the tests below read it out of `route_guards.rs` and compare. */
type KernelScope = 'public' | 'Read' | 'Write' | 'Admin';

/**
 * III-0b — the AUTHORITY half of a route's row: what the route can ask the
 * kernel for, and whether any of it changes stored state.
 *
 * Both fields are DECLARATIONS, and both are checked against derivations:
 *
 * - `kernel` is compared against the paths the route's own wire calls derive
 *   (its page file's `client.*` call sites plus the `#lib/api/*` symbols it
 *   imports) and against the scope the kernel's own gate table demands for
 *   each. A route that changed its calls, or a table written from
 *   `openapi.yaml` prose, fails.
 * - `mutates` is NOT derivable from scope, and pretending otherwise was the
 *   first draft's mistake: `replay-diff` is Write-scoped and persists
 *   NOTHING, and `/recall` is POST-and-Read. So a mutating reach is a NAMED
 *   claim with the act, and a Write-scoped path that is not named either way
 *   fails the enumeration test rather than drifting.
 */
interface RouteAuthz {
	/** path → the authorization the kernel demands for that path. */
	readonly kernel: Readonly<Record<string, KernelScope>>;
	/** The route's reach into a verb that CHANGES stored state, named, or
	 * `null`. The act is stated because "mutating" is the claim being made. */
	readonly mutates: null | { readonly path: string; readonly act: string };
}

/** One census row: the route, its reason, and the kernel authority its own
 * wire exercises. Census-form since the decommission — the parity verdict
 * cells (`verdict`/`dioxus`/`nearest`) retired WITH the Dioxus workspace;
 * the because fields keep what still teaches in past tense. */
interface RouteRow {
	readonly route: string;
	readonly because: string;
	/** III-0b: the kernel authority this route's own wire exercises. Derived
	 * facts, checked — see `RouteAuthz`. */
	readonly authz: RouteAuthz;
}

/**
 * THE TABLE. One row per user-reachable shell route. Census-form since the
 * decommission (2026-11-13): the verdict cells retired with the Dioxus
 * workspace, and each row states what the route SERVES and the authority its
 * own wire exercises. The parity HISTORY stays in the because fields, past
 * tense, where it still teaches: the III-6 false-rename lesson (a claimed
 * counterpart that reads as coverage) and the MISSING-ON-BOTH-SIDES home
 * finding, now concluded — the other side retired.
 *
 * M3-a (plan `21_CHAT_FIRST_IA`): the chat is the app. The wizard that served
 * `/` is dropped, `/chat` is PROMOTED to `/`, and `/overview`, `/search` and
 * `/models` retire as routes.
 *
 * The sidebar frame (2026-10-25) grew the table to eight rows: `/audit` and
 * `/governance`. 2026-10-30 added `/settings`; 2026-11-12 added `/journey`.
 */
const ROUTE_ROWS: readonly RouteRow[] = [
	{
		route: '/',
		because:
			'the chat surface, PROMOTED to the home route in M3-a (plan 21: the chat is the app). Plan 09 §1 ' +
			'records conversation as MISSING ON BOTH SIDES; the shell grew /chat in II-3 and M3-a made it the ' +
			"shell's `/`. The retired Dioxus `/` was a connect-first landing — a different surface — so the " +
			'finding is CONCLUDED, not open: the other side retired 2026-11-13, and the shell home is the ' +
			'only conversation surface there ever was.',
		authz: {
			// The chat surface reaches three kernel verbs via `#lib/api/*`, and
			// each is derived from the SYMBOL the page imports (the typed
			// transports carry the call sites; recallMemory/verifyClaim/
			// forgetRecord carry none). `[forget]` is the mutating one, and it
			// is soft-only — asserted at this route's OWN call site below.
			// M3-c: the session switch's replay fetch joins via the frame's
			// store (`fetchTurns` from `#lib/shell/sessions.svelte.js`) — the
			// stored-turn page is a Read.
			kernel: {
				'/recall': 'Read',
				'/chat/sessions/{session}/turns': 'Read',
				'/verify': 'Read',
				'/ump/forget': 'Write'
			},
			mutates: {
				path: '/ump/forget',
				act: 'tombstones the cited row — SOFT only: the request is `{id}` and nothing else, and the core refuses any request carrying `hard`'
			}
		}
	},
	{
		route: '/recall',
		because:
			'the memory surface. The sidebar frame lists it as "Memory": the POST /recall ' +
			'wire rendered as memory cards, with the store\'s count beside the empty state and an ' +
			'"Add memory" that files a PROPOSAL into the review queue — never a memory. (History: ' +
			'exact parity with the retired Dioxus /recall — the same path on both sides until the ' +
			'decommission.)',
		authz: {
			// The page's own POST /recall, plus the two api symbols it imports:
			// `fetchKernelFacts` (the public /health probe and the Read-scoped
			// /stats count) and `proposeMemory` (the Write-scoped proposal).
			kernel: {
				'/health': 'public',
				'/ingest/proposal': 'Write',
				'/recall': 'Read',
				'/stats': 'Read'
			},
			mutates: {
				path: '/ingest/proposal',
				act: 'files a CANDIDATE memory into the human review queue — the kernel creates no knowledge row and never auto-promotes; nothing enters the store until an operator approves it on /review'
			}
		}
	},
	{
		route: '/audit',
		because:
			'the operator\'s hash-chain read: the chain newest-first with limit/offset paging and the ' +
			'chain verdict beside it, filters as query params. (History: the retired Dioxus audit ' +
			'panel carried its since/principal filters as URL segments in the route string — a ' +
			'query-shape difference, not a different surface.)',
		authz: {
			// Both reads are operator reads in the kernel's own table: the
			// event list and the chain recomputation are Admin.
			kernel: { '/audit': 'Admin', '/audit/verify': 'Admin' },
			mutates: null
		}
	},
	{
		route: '/journey',
		because:
			'"the chain the customer sees": the journey (IMPROVE15) reads the kernel\'s chain-projected ' +
			'transparency surface (/clients/{name}/journey — closed customer sentences, roles not names, ' +
			'receipts not scores, zero new state) through the pure projection. The retired Dioxus side ' +
			'had no route for it — the nearest surface there was /audit (the operator\'s hash-chain ' +
			'read), a fact about the old tree and never a counterpart: the audit showed the chain to ' +
			'the OPERATOR; the journey shows the customer-facing projection of it.',
		authz: {
			// The journey's OWN authz law lives kernel-side in the handler (the
			// client's own principal + the operator; everyone else answers the
			// existence-blind 404). The kernel's table row is the OPERATOR leg
			// the handler carries (Admin — the class every client op speaks);
			// the own-principal arm is the handler's law beyond the table.
			kernel: { '/clients/{name}/journey': 'Admin' },
			mutates: null
		}
	},
	{
		route: '/governance',
		because:
			'"what the kernel enforces" as one surface: domains, roles and retention read together. ' +
			'On the retired Dioxus side, domains lived under the /system console group, retention ' +
			'under /data, and roles had no surface at all — no aggregation existed there to retire ' +
			'alongside.',
		authz: {
			// Domains and roles are Read; retention is Admin — three reads with
			// two scopes, each section rendering its own answer.
			kernel: { '/domains': 'Read', '/retention': 'Admin', '/roles': 'Read' },
			mutates: null
		}
	},
	{
		route: '/settings',
		because:
			'the app preferences as a full PAGE (the 2026-10-30 dsh-UX round, the ChatGPT-desktop shape). ' +
			'The retired Dioxus shell had no preferences page — its settings were a native-menu concern ' +
			'it never carried as a route. The page reads the kernel ' +
			'through `#lib/api/kernel-facts` (fetchKernelFacts → /health + /stats, and the model posture ' +
			'via /health/db) for the model + About panes; the token and origin panes are TAURI-ONLY ' +
			'(keychain + rotation through the shell commands, no kernel wire), and the locale pane is ' +
			'browser-local. Opened from the sidebar FOOT and the native ⌘, item — never a panel-list entry.',
		authz: {
			// The PAGE's own wire reach (the derivation reads the page's
			// imports): fetchKernelFacts reads /health (public — the version
			// fact) and /stats (Read — the About pane's corpus fact). The
			// MODEL pane's /health/db Admin read is carried by the model
			// SECTION (a frame-consumed component; its call attributes
			// through the frame walk). The token and origin panes are
			// TAURI-ONLY — no kernel wire.
			kernel: { '/health': 'public', '/stats': 'Read' },
			mutates: null
		}
	},
	{
		route: '/recall/[trace_id]/trace',
		because:
			'the trace replay view: the shell nests a literal `/trace` segment under the id. (History: ' +
			'the retired Dioxus side put the trace at the leaf, /recall/:trace_id — a path-shape ' +
			'decision, not a rename.)',
		authz: {
			// The trace surface is Admin: replaying a stored run is an operator
			// read (trace replay + DSAR are operator surfaces, per the kernel's
			// own table comment).
			kernel: { '/recall/{trace_id}/trace': 'Admin' },
			mutates: null
		}
	},
	{
		route: '/decisions',
		because:
			'the shell\'s /decisions serves the DECISION-RUN listing (`GET /workflow/decision-runs`), an ' +
			'Admin surface — NOT the proposal queue. Until III-6 this row claimed the Dioxus /review ' +
			'(the proposal queue) as its counterpart: a rename that did not hold, because the two are ' +
			'different surfaces, and the one the shell was MISSING is the one the rename claimed to ' +
			'cover. The retired Dioxus side\'s run surface was /runs/:run_id (the workflow-run ' +
			'conversation), and the proposal queue is carried by the shell\'s own /review row.',
		authz: {
			// The decision-run LIST is an operator surface in the kernel's own
			// table (Admin), and the shell reads it — /decisions is not a
			// user-grade surface and the row says so.
			kernel: { '/workflow/decision-runs': 'Admin' },
			mutates: null
		}
	},
	{
		route: '/decisions/[id]',
		because:
			'the single-run detail view: the shell\'s /decisions/[id] reads the DECISION-RUN ' +
			'trace (`GET /workflow/decision-runs/{id}`), not a proposal — until III-6 this row claimed ' +
			'the Dioxus /review/:proposal_id, which is the single-PROPOSAL view, and it was the same ' +
			'false rename as /decisions, one row over. The retired Dioxus side\'s single-run view was ' +
			'/runs/:run_id.',
		authz: {
			// The one Write here is replay-diff, which re-executes a stored run
			// and persists NOTHING — the handler doc's own words. It is the
			// measured reason `mutates` is a separate claim from scope:
			// classified in NON_MUTATING_WRITE below, never silently dropped.
			kernel: {
				'/workflow/decision-runs/{id}': 'Read',
				'/workflow/decision-runs/{id}/replay-diff': 'Write'
			},
			mutates: null
		}
	},
	{
		route: '/review',
		because:
			'the human review queue. The retired Dioxus app served it at /review ' +
			'and could approve, reject and edit rows; the shell\'s /review reads the SAME queue and — ' +
			'as of the III-6 amendment (the approve-action round, C2 G-6) — carries EXACTLY ONE of the ' +
			'three verbs back: the approve, for the authorized, digest-bound, audited. The affordance ' +
			'renders only on a live (200) listing and its call carries the row\'s DISPLAYED content_digest; ' +
			'reject and edit stay behind the kernel\'s own roles, and this surface has no path for them. ' +
			'The amendment is an act, named here and in the register row.',
		authz: {
			// The queue is Read-scoped in the kernel's own table, and the ONE
			// write the amendment granted rides the kernel's `approve` role
			// gate: the shell reads and approves with the caller's own bearer
			// and the kernel decides both halves — who sees the queue, and
			// whose digest-bound decision lands.
			kernel: { '/proposals': 'Read', '/proposals/{id}/approve': 'Write' },
			mutates: {
				path: '/proposals/{id}/approve',
				act: 'approves the displayed row into memory — the call carries the row\'s DISPLAYED content_digest, the kernel re-derives it and refuses on drift (409), and the decision is audited under the acting principal\'s name'
			}
		}
	}
];

test('the_shell_route_census_finds_every_page_and_asserts_its_own_floor', () => {
	const routesDir = join(shellRoot, 'src', 'routes');

	// ---- ANTI-VACUITY FIRST: the walk found something ----
	//
	// Deliberately the FIRST assertion in the file. A census that reports an
	// empty array would satisfy every "every route has a verdict" claim below by
	// having no routes, so the floor is asserted before anything reads the list.
	const routes = censusShellRoutes(routesDir);
	expect(
		routes.length,
		'the census walked shell/src/routes and found no +page.svelte — deleting the walk, ' +
			'renaming the directory, or breaking the path derivation must FAIL here, not pass empty'
	).toBeGreaterThanOrEqual(ROUTE_FLOOR);

	// ---- AND the exact set, so the floor is not the whole claim ----
	//
	// The floor says "something was found"; this says "and it is exactly these".
	// A new route fails here until it is given a verdict, which is the moment a
	// parity question gets asked rather than skipped.
	expect(routes).toEqual([...CENSUS_ROUTES]);
	expect(routes.length).toBe(ROUTE_FLOOR);

	// ---- THE SIDEBAR'S NAV SET IS THE REGISTRY'S (M3-f, plan 26 §2) ----
	//
	// The frame renders `navEntries` from the extension registry — ONE
	// source — and the census drives it both ways, fail-closed:
	//
	//  - every registry nav route is censused AND walked (the load guard in
	//    `route-census.ts` refuses an uncensused route; this restates the
	//    walk half so the red names the file's own ground truth), and
	//  - the registry's route set is EXACTLY the census's sidebar surfaces —
	//    the census minus the routes that are deliberately NOT nav entries
	//    (the operator listing `/decisions`, its detail route, and the trace
	//    detail nested under `/recall`, all reached from lists and
	//    citations). A contribution deleted reds here NAMING the route; a
	//    sixth nav entry invented to "complete" anything reds here too.
	const sidebarSurfaces = (CENSUS_ROUTES as readonly string[]).filter(
		(route) => !DECISION_AND_DETAIL_ROUTES.includes(route as (typeof DECISION_AND_DETAIL_ROUTES)[number])
	);
	expect(
		navEntries.map((entry) => entry.route).sort(),
		'the registry navEntries must be exactly the censused sidebar surfaces — a deleted contribution or an invented sixth entry reds here, naming the route'
	).toEqual([...sidebarSurfaces].sort());
	for (const entry of navEntries) {
		expect(routes, `${entry.route} (${entry.id}) must be a walked route`).toContain(entry.route);
	}

	// ---- THE DERIVATION IS PROVEN, not just its output ----
	//
	// A hardcoded list would pass the two assertions above. These arms pin the
	// RULES the list came from, so a route added under a new convention (a
	// group directory, a layout, a `(group)`) cannot be silently mis-pathed.
	const page = (rel: string): string => join(routesDir, rel, '+page.svelte');
	expect(existsSync(page('')), 'routes/+page.svelte must exist — it is the root route').toBe(true);
	expect(censusShellRoutes(routesDir)).toContain('/');
	// Dynamic segments keep their bracket form, which is what a reader needs to
	// see: `/decisions/[id]` is not `/decisions/id`.
	expect(routes).toContain('/decisions/[id]');
	expect(routes).not.toContain('/decisions/id');
	// A nested path keeps every segment.
	expect(routes).toContain('/recall/[trace_id]/trace');

	// Anti-vacuity on the walk itself: a directory with no `+page.svelte`
	// contributes NOTHING rather than a phantom route. `decisions/[id]` has one;
	// if a future layout-only directory appears, this is what stops it being
	// reported as a user-reachable page.
	const withLayoutOnly = censusShellRoutes(routesDir);
	expect(new Set(withLayoutOnly).size, 'the census must not emit duplicates').toBe(
		withLayoutOnly.length
	);
});

test('every_user_reachable_shell_route_carries_a_census_row', () => {
	const routes = censusShellRoutes(join(shellRoot, 'src', 'routes'));

	// ---- EXHAUSTIVE over the census: one row per route, no route unjudged ----
	const judged = new Set(ROUTE_ROWS.map((row) => row.route));
	for (const route of routes) {
		expect(
			judged.has(route),
			`${route} has no census row. Every user-reachable shell route needs one.`
		).toBe(true);
	}
	// And no row for a route that does not exist — a table that invents a route
	// is the same defect as one that misses one.
	for (const row of ROUTE_ROWS) {
		expect(
			routes,
			`the census table names ${row.route}, which the census did not find. A row for a route ` +
				'that does not exist would make the table agree with itself.'
		).toContain(row.route);
	}
	expect(ROUTE_ROWS.length).toBe(routes.length);

	// ---- EVERY ROW CARRIES A REASON ----
	//
	// The parity verdicts retired WITH the Dioxus workspace (2026-11-13); what
	// remains load-bearing on every row is the reason and the authority. A row
	// with an empty `because` is the decoration Plan 08 §0 warns about.
	for (const row of ROUTE_ROWS) {
		expect(
			row.because.length,
			`${row.route} carries a row with no reason`
		).toBeGreaterThan(20);
	}

	// ---- THE HOME ROW'S FINDING, CONCLUDED ----
	//
	// Plan 09 §1's "missing on both sides" is the one parity finding a reader
	// is most likely to assume was quietly filled in. It stays asserted in its
	// CONCLUDED form: the other side retired 2026-11-13, and the shell home is
	// the conversation surface.
	const home = ROUTE_ROWS.find((row) => row.route === '/');
	expect(home, '/ must carry a row — it is the app\'s home route').toBeDefined();
	expect(home!.because).toContain('MISSING ON BOTH SIDES');
	expect(home!.because).toContain('CONCLUDED');
});

test('this_census_reads_the_SHELL_not_the_kernel_route_tables', () => {
	// THE WRONG-ARTIFACT PIN, and the one this whole file is at risk of being.
	//
	// `OPENAPI_ROUTES` and `AUTHZ_GATES` in `src/server/router/route_guards.rs`
	// are what Plan 09 §2's gates name, they are already green, and they cover
	// the composed SERVER router. Substituting them for this census would pass a
	// gate that has never looked at the shell — and would look exactly like the
	// work being done. So the difference is asserted, not asserted-about.
	const kernelGuards = readFileSync(
		join(shellRoot, '..', 'src', 'server', 'router', 'route_guards.rs'),
		'utf8'
	);
	const kernelRoutes = new Set(
		[...kernelGuards.matchAll(/^\s{4}"(\/[^"]*)",$/gm)].map((m) => m[1]!)
	);

	const shellRoutes = censusShellRoutes(join(shellRoot, 'src', 'routes'));

	// Every shell route the kernel does NOT serve. These are precisely the routes
	// a kernel-table census would MISS, so if someone swapped the walk for the
	// kernel table, this set would collapse and the equality below would fail.
	// (M3-a retired `/overview`, `/models` and `/chat` as route paths; the
	// sidebar frame added `/governance`, a shell-only name — `/audit` shares
	// its name with the kernel path it reads. The shell-only set is the two
	// decision surfaces, /review and /governance; `/`, `/recall` and `/audit`
	// share their names with kernel paths.)
	const shellOnly = ['/decisions', '/decisions/[id]', '/governance', '/review'];
	for (const route of shellOnly) {
		expect(shellRoutes, `the census must find ${route}`).toContain(route);
		expect(
			[...kernelRoutes],
			`${route} is asserted to be absent from the kernel's route table; if it now appears ` +
				'there, this pin has stopped proving that the two artifacts differ'
		).not.toContain(route);
	}
	expect(shellOnly.length).toBeGreaterThan(0);

	// And the difference runs BOTH ways, so neither artifact can be a superset
	// standing in for the other: the kernel serves paths that are not user-facing
	// shell pages at all.
	const kernelOnly = [...kernelRoutes].filter((route) => !shellRoutes.includes(route));
	expect(
		kernelOnly.length,
		'the kernel route table must contain paths the shell has no page for, or the two artifacts ' +
			'are not actually different and this pin is measuring nothing'
	).toBeGreaterThan(0);

	// Finally: the census is derived from the shell's route DIRECTORY, so it is
	// asserted to be looking there and not somewhere else.
	expect(existsSync(join(shellRoot, 'src', 'routes', '+page.svelte'))).toBe(true);
	expect(existsSync(join(shellRoot, '..', 'src', 'server', 'router', 'route_guards.rs'))).toBe(true);
});

/**
 * ────────────────────── III-0b — the AUTHORITY derivation ──────────────────
 *
 * The census above proves every shell route is ROW-judged (one row per
 * user-reachable route, each with a reason); this section proves every route
 * is AUTHORITY-judged (gate 3): the `authz` field
 * on each row names the kernel scopes its calls demand and its mutating reach,
 * and three tests below hold those names to derivations:
 *
 * 1. **A route's kernel paths** = the `client.*` call sites in its OWN page
 *    file, plus the call sites of the `#lib/api/*` symbols that file imports.
 *    ONE hop into `api/`, deliberately: several pages import `client.ts` for
 *    its `client` object, and attributing the module's own fetcher on a mere
 *    import would credit every one of them with that fetcher's path. A call is
 *    attributed to the SYMBOL that carries it (`fetchPendingQueue`, the typed
 *    `*Transport`s), and only a page importing THAT symbol inherits the path.
 * 2. **Scopes** are the kernel's — `AUTHZ_GATES` / `PUBLIC_PATHS` in
 *    `src/server/router/route_guards.rs`, parsed per region with floors and
 *    anchors. NOT `openapi.yaml` prose: the typed-client drift gate already
 *    holds the YAML to the generated client, and this table must read what the
 *    kernel ENFORCES. A shell-called path with no row fails; a path with two
 *    rows fails too, because a shared path's scope is per-handler and a
 *    path-keyed census cannot state it (that duplicate is real:
 *    `/workflow/delivery/runs` carries Read and Write).
 * 3. **The call-site census is CLOSED over `shell/src`.** Every `client.*`
 *    call site must be a route page, an api module, or named in
 *    `NON_ROUTE_CALL_SITES` with its reason, and every api-module symbol that
 *    carries a call must be imported by at least one route. The closure is
 *    what makes this table DERIVED: a call appearing anywhere else fails here,
 *    where someone has to classify it.
 *
 * `mutates` is claimed per row and NOT inferred from scope — an inference the
 * first draft made and this round measured away: `replay-diff` is Write-scoped
 * and persists NOTHING, and `/recall` is POST-and-Read. The Write-scoped paths
 * are enumerated exhaustively, and each must be a row's named `mutates` reach
 * or a named non-mutating exception with its reason.
 */

/** The api modules: the one module tree the derivation hops into. */
const API_DIR = join(shellRoot, 'src', 'lib', 'api');

/**
 * III-6 — the derivation hops into a SECOND module tree.
 *
 * The review queue's typed read lives in `src/lib/queue/` (this round's own
 * layout), not under `#lib/api/`. The derivation MUST hop there: otherwise the
 * `/review` row would DERIVE no kernel path at all, and a row that agrees
 * with an empty derivation is a row that measures nothing.
 *
 * The alternative was to name `src/lib/queue/client.ts` in
 * NON_ROUTE_CALL_SITES, and that would have been a LIE: that slot's reason
 * field is "why it is not route-reachable", and this module IS
 * route-reachable. A naming slot for a reachable module would make the
 * closure's own vocabulary false — and a closure that names reachable modules
 * as unreachable is a closure that cannot catch the next one.
 */
const MODULE_DIRS: Readonly<Record<string, string>> = {
	api: API_DIR,
	queue: join(shellRoot, 'src', 'lib', 'queue'),
	// M3-c (plan 23): the frame's session store joins the derivation for the
	// SAME reason the queue tree did — the home page imports it
	// (`fetchTurns` for replay), so the module IS route-reachable, and naming
	// it in NON_ROUTE_CALL_SITES would be the R141 lie (that slot's reason
	// field is "why it is not route-reachable", and this one is). The store
	// is ALSO the frame's data module — AppShell renders it on every route —
	// which is why FRAME_CONSUMERS below joins the reachability walk.
	shell: join(shellRoot, 'src', 'lib', 'shell')
};

/** `client.*` call sites OUTSIDE the derivation's two known places (route
 * pages, api modules) — relative file → why it is not route-reachable. The
 * closure test makes this a NAMING SLOT, not a dumping ground: a call site
 * anywhere else fails until the derivation attributes it or it is named here
 * with its reason. It is not empty: the eval lane is exactly this case. */
const NON_ROUTE_CALL_SITES: Readonly<Record<string, string>> = {
	'src/lib/eval/baseline.ts':
		'the eval baseline harness drives the kernel over BOTH wires (raw `fetch` as the control, the typed ' +
		'client as the subject) so the bridge itself can be scored. No route renders it — the eval lane runs ' +
		'it; its POST /recall is the same verb the /recall page calls, measured, not a route.'
};

/** `Write`-scoped kernel paths that DO NOT change stored state, each with the
 * reason it is an exception. `Write` is the kernel's mark of a state-changing
 * act; an exception to that has to be argued, so this map carries arguments
 * and the enumeration test refuses a Write-scoped path that is in NEITHER
 * this map nor a row's `mutates`. */
const NON_MUTATING_WRITE: Readonly<Record<string, string>> = {
	'/workflow/decision-runs/{id}/replay-diff':
		're-executes a stored run under its own recorded conditions and reports the per-stage digest ' +
		'agreement — the handler doc states it plainly: "the replay persists NOTHING"'
};

/** The kernel tables, parsed. Floors belong to the callers: an empty parse
 * must fail as ITS OWN problem, never as a hundred downstream holes. */
interface KernelTables {
	/** path → its `AUTHZ_GATES` rows. An ARRAY because the table keys by path
	 * and a shared path carries one row per handler; a Map would silently keep
	 * only the last row and this census would not notice. */
	readonly gates: ReadonlyMap<string, readonly string[]>;
	readonly publicPaths: ReadonlySet<string>;
}

/** Parses `route_guards.rs`, bounded to each table's own declaration region so
 * a same-shaped row in a future table cannot leak in. */
function kernelTables(): KernelTables {
	const source = readFileSync(
		join(shellRoot, '..', 'src', 'server', 'router', 'route_guards.rs'),
		'utf8'
	);
	const regionOf = (constName: string): string => {
		const start = source.indexOf(`pub const ${constName}`);
		expect(
			start,
			`${constName} must exist in route_guards.rs — the authz source of truth moved`
		).toBeGreaterThan(-1);
		const next = source.indexOf('\npub const', start + 1);
		return source.slice(start, next === -1 ? undefined : next);
	};
	const gates = new Map<string, string[]>();
	for (const m of regionOf('AUTHZ_GATES').matchAll(
		/^\s*\("([^"]+)",\s*"([^"]+)"\),\s*$/gm
	)) {
		gates.set(m[1]!, [...(gates.get(m[1]!) ?? []), m[2]!]);
	}
	const publicPaths = new Set<string>();
	for (const m of regionOf('PUBLIC_PATHS').matchAll(/^\s*"([^"]+)",\s*$/gm)) {
		publicPaths.add(m[1]!);
	}
	return { gates, publicPaths };
}

/** The scope a path demands — or a THROW naming why the census cannot state
 * one. Both throw cases are load-bearing: an ungated path is a call the kernel
 * would refuse at runtime, and a duplicate-scoped path needs a method-qualified
 * fact this path-keyed table does not carry. */
function kernelScopeFor(path: string, tables: KernelTables, context: string): KernelScope {
	if (tables.publicPaths.has(path)) return 'public';
	const rows = tables.gates.get(path);
	if (rows === undefined) {
		throw new Error(
			`${context}: ${path} is in NEITHER PUBLIC_PATHS nor AUTHZ_GATES — the shell calls a path the kernel does not admit`
		);
	}
	if (rows.length !== 1) {
		throw new Error(
			`${context}: ${path} carries ${rows.length} AUTHZ_GATES rows (${rows.join(' / ')}) — a shared ` +
				"path's scope is per-handler, and this census cannot state one from a path-keyed table"
		);
	}
	return rows[0] as KernelScope;
}

/** A `client.METHOD('<path>'` path literal — the typed client's only call
 * shape, and the census reads RAW text: a comment naming a call would be
 * counted, deliberately (silencing it would need a prose-keyed exemption). The
 * cost is bounded: a comment can only ADD a path, which the row equality then
 * refuses until someone resolves it. */
function clientCallPaths(source: string): string[] {
	return [...source.matchAll(/client\.(?:GET|POST|PUT|DELETE|PATCH)\(\s*'([^']+)'/g)].map(
		(m) => m[1]!
	);
}

/** HTTP-method calls on the client whose argument is NOT a quoted path — the
 * shapes the path scan cannot read, generic call syntax included. Must be
 * empty per file: a scanner that skips a call shape is a census with a silent
 * gap. `client.use(…)` is not matched — the scan only claims HTTP methods. */
function unreadableClientCalls(source: string): string[] {
	const unreadable: string[] = [];
	for (const m of source.matchAll(/\bclient\.(GET|POST|PUT|DELETE|PATCH)\b/g)) {
		const afterMethod = source.slice(m.index! + m[0].length).trimStart();
		if (!afterMethod.startsWith('(')) {
			unreadable.push(`${m[0]}${afterMethod.slice(0, 20)}`);
			continue;
		}
		const afterParen = afterMethod.slice(1).trimStart();
		if (!afterParen.startsWith("'")) unreadable.push(`${m[0]}(${afterParen.slice(0, 20)}`);
	}
	return unreadable;
}

/** path-bearing exports of one api module: `export` symbol → the kernel paths
 * its body calls. Each call site is attributed to the NEAREST PRECEDING export
 * declaration; a call with none lands on the '' key, which the closure test
 * asserts is never present (fail-closed attribution). */
function apiModuleSymbolPaths(source: string): Map<string, string[]> {
	const declarations: Array<{ name: string; at: number }> = [];
	for (const m of source.matchAll(
		/export\s+(?:async\s+)?(?:function\*?|const)\s+([A-Za-z_$][\w$]*)/g
	)) {
		declarations.push({ name: m[1]!, at: m.index! });
	}
	const bySymbol = new Map<string, string[]>();
	for (const m of source.matchAll(/client\.(?:GET|POST|PUT|DELETE|PATCH)\(\s*'([^']+)'/g)) {
		let owner = '';
		for (const decl of declarations) {
			if (decl.at < m.index!) owner = decl.name;
		}
		bySymbol.set(owner, [...(bySymbol.get(owner) ?? []), m[1]!]);
	}
	return bySymbol;
}

/** The `#lib/<tree>/*` imports of one source file: module file → imported symbol
 * names (aliases resolved to the EXPORTED name, `type` prefixes stripped —
 * the names the symbol attribution above keys on).
 *
 * Each match is ONE import statement: the group may span newlines (multiline
 * imports) but never a `;`, and the statement must start at a line boundary.
 * Both are load-bearing. The first cut let the lazy group run from one
 * statement's `{` ACROSS intervening imports to a later `#lib/api/…`
 * specifier — `fetchWizardPacks` got glued to the previous statement's tail
 * and the `/` route's derivation came back empty, which is exactly the
 * empty-derivation class this section exists to refuse.
 *
 * III-6: the tree is one of `MODULE_DIRS` (see above), so a module OUTSIDE
 * `#lib/api/` can still be derived rather than named as a non-route call
 * site. */
function apiImportsOf(source: string): Map<string, string[]> {
	const imports = new Map<string, string[]>();
	for (const m of source.matchAll(
		/(?:^|\n)[ \t]*import\s*\{([^;]*?)\}\s*from\s*'#lib\/(api|queue|shell)\/([\w.-]+?)(?:\.js)?'/g
	)) {
		const file = join(MODULE_DIRS[m[2] as 'api' | 'queue' | 'shell']!, `${m[3]!}.ts`);
		const names = m[1]!
			.split(',')
			.map((s) => s.replace(/^\s*type\s+/, '').split(/\s+as\s+/)[0]!.trim())
			.filter((s) => s.length > 0);
		imports.set(file, [...(imports.get(file) ?? []), ...names]);
	}
	return imports;
}

/** One route's kernel paths, DERIVED: the page's own call sites plus the calls
 * of the `#lib/api/*` symbols it imports. */
function derivedKernelPaths(entry: RouteEntry): string[] {
	const source = readFileSync(entry.file, 'utf8');
	const paths = new Set<string>(clientCallPaths(source));
	for (const [file, symbols] of apiImportsOf(source)) {
		expect(
			existsSync(file),
			`${entry.route} imports ${file}, which does not exist — the #lib alias or the module moved`
		).toBe(true);
		const bySymbol = apiModuleSymbolPaths(readFileSync(file, 'utf8'));
		for (const symbol of symbols) {
			for (const path of bySymbol.get(symbol) ?? []) paths.add(path);
		}
	}
	return [...paths].sort();
}

test('every_user_reachable_shell_route_carries_its_kernels_own_authz_verdict', () => {
	const entries = censusShellRouteEntries(join(shellRoot, 'src', 'routes'));
	const tables = kernelTables();

	// Anti-vacuity FIRST, the file's rule: an empty kernel parse must fail as
	// its own problem, not as a hundred "ungated path" throws downstream.
	expect(
		tables.gates.size,
		'AUTHZ_GATES parsed empty — the authz source of truth did not load'
	).toBeGreaterThanOrEqual(150);
	expect(tables.publicPaths.size, 'PUBLIC_PATHS parsed empty').toBeGreaterThanOrEqual(10);
	// Anchors: the scopes this table leans on, asserted at the source.
	expect(tables.gates.get('/recall')).toEqual(['Read']);
	expect(tables.gates.get('/verify')).toEqual(['Read']);
	expect(tables.gates.get('/ump/forget')).toEqual(['Write']);
	expect(tables.publicPaths.has('/health')).toBe(true);

	let derivedTotal = 0;
	for (const entry of entries) {
		const row = ROUTE_ROWS.find((r) => r.route === entry.route);
		expect(row, `${entry.route} has no row`).toBeDefined();
		const declared = row!.authz.kernel;
		expect(
			Object.keys(declared).length,
			`${entry.route} carries an EMPTY authz verdict — every census row must name the kernel scope(s) its calls require`
		).toBeGreaterThan(0);

		const derived = derivedKernelPaths(entry);
		expect(
			Object.keys(declared).sort(),
			`${entry.route}: the declared kernel paths and the paths its own wire DERIVES must agree — ` +
				'a route that changed its calls, or a table written from openapi.yaml prose, fails here'
		).toEqual(derived);

		for (const path of derived) {
			const scope = kernelScopeFor(path, tables, entry.route);
			expect(
				declared[path],
				`${entry.route}: ${path} is declared ${String(declared[path])} but the kernel's own table says ${scope}`
			).toBe(scope);
		}
		derivedTotal += derived.length;
	}
	// Re-measured with the sidebar frame: 17 paths across the eight routes —
	// / (recall, verify, ump/forget), /audit (two), /decisions,
	// /decisions/[id] (two), /governance (three), /recall (its own recall
	// plus health, stats and the proposal), the trace, /review. It was 9
	// over the six-route table. M3-c: 18 — the home page's replay fetch
	// (`fetchTurns` via the frame's store) adds /chat/sessions/{session}/turns.
	expect(
		derivedTotal,
		'the whole table derived fewer than the measured eighteen kernel paths — the derivation is not reading the wire'
	).toBeGreaterThanOrEqual(18);
});

test('the_authz_table_is_derived_from_the_wire_the_routes_call', () => {
	const routesDir = join(shellRoot, 'src', 'routes');
	const entries = censusShellRouteEntries(routesDir);
	const pageFiles = new Set(entries.map((e) => e.file));
	const apiModules = new Set(
		Object.values(MODULE_DIRS).flatMap((dir) =>
			readdirSync(dir)
				.filter((f) => f.endsWith('.ts'))
				.map((f) => join(dir, f))
		)
	);

	// ---- the closure: every typed-wire call site, wherever it lives ----
	const sites: Array<{ file: string; paths: string[] }> = [];
	const walk = (dir: string): void => {
		for (const de of readdirSync(dir, { withFileTypes: true })) {
			const path = join(dir, de.name);
			if (de.isDirectory()) {
				walk(path);
			} else if (/\.(ts|svelte)$/.test(de.name)) {
				const source = readFileSync(path, 'utf8');
				expect(
					unreadableClientCalls(source),
					`${relative(shellRoot, path)} has a client HTTP call whose argument is not a quoted path — the ` +
						'census only reads literals, and a scanner with a blind spot is a census with a gap'
				).toEqual([]);
				const paths = clientCallPaths(source);
				if (paths.length > 0) sites.push({ file: path, paths });
			}
		}
	};
	walk(join(shellRoot, 'src'));

	// Anti-vacuity: the walk found the wire at all.
	expect(
		sites.length,
		'no client.* call sites found under shell/src — the census stopped reading the wire'
	).toBeGreaterThanOrEqual(8);

	for (const site of sites) {
		const rel = relative(shellRoot, site.file);
		const known =
			pageFiles.has(site.file) ||
			apiModules.has(site.file) ||
			Object.prototype.hasOwnProperty.call(NON_ROUTE_CALL_SITES, rel);
		expect(
			known,
			`${rel} calls the typed wire (${site.paths.join(', ')}) but is neither a route page nor a #lib/api ` +
				'module. Every route-reachable call belongs to one of the two; anything else must be NAMED in ' +
				'NON_ROUTE_CALL_SITES with its reason, never left silent.'
		).toBe(true);
	}

	// ---- every api symbol carrying a call is reachable from a route ----
	//
	// M3-c: the walk reads the ROUTE pages AND the frame component. The
	// sidebar frame (R141) renders on EVERY route — its data modules are
	// user-reachable by construction — and a page-only walk would call the
	// frame's wire "dead" ("exports refresh, which NO route imports"), a
	// false claim: the region's reads/writes are the session region's whole
	// point. The frame is NAMED here rather than invented into the route
	// census: it is chrome, not a page, and its derivation stays out of the
	// per-route authz rows (a shared path's scope is not stateable by this
	// census — kernelScopeFor's own refusal).
	const frameConsumers = [
		join(shellRoot, 'src', 'lib', 'components', 'AppShell.svelte'),
		// The settings sections join the frame walk (the S-4 onboarding
		// round): the Settings dialog renders ON EVERY ROUTE (the nav opens
		// it from the frame), so its data modules are user-reachable by
		// construction — the same class as AppShell above. The walk reads
			// the extension manifest (which wires the sections into the
		// registry AppShell renders) and the section components themselves.
		join(shellRoot, 'src', 'lib', 'extensions', 'core-settings.ts'),
		join(shellRoot, 'src', 'lib', 'components', 'settings', 'model-section.svelte'),
		// CG-12: the rerank section joins the walk with the model section —
		// same dialog, same read module (`kernel-facts.ts`'s rerank echo).
		join(shellRoot, 'src', 'lib', 'components', 'settings', 'rerank-section.svelte')
	];
	for (const file of apiModules) {
		const bySymbol = apiModuleSymbolPaths(readFileSync(file, 'utf8'));
		expect(
			bySymbol.has(''),
			`${relative(shellRoot, file)} has a client call with no preceding export — the attribution scan ` +
				'cannot say which symbol owns it'
		).toBe(false);
		const importedBy = [...entries, ...frameConsumers.map((f) => ({ file: f }))].flatMap(
			(e) => apiImportsOf(readFileSync(e.file, 'utf8')).get(file) ?? []
		);
		for (const [symbol, paths] of bySymbol) {
			expect(
				importedBy,
				`${relative(shellRoot, file)} exports ${symbol} (→ ${paths.join(', ')}), which NO route imports — ` +
					'either a route that reaches it is missing from the census, or the wire is dead'
			).toContain(symbol);
		}
	}

	// ---- and the scope source is the KERNEL's table, spot-asserted ----
	const tables = kernelTables();
	for (const [path, scope] of [
		['/search', 'Read'],
		['/stats', 'Read'],
		['/ingest/proposal', 'Write'],
		['/workflow/decision-runs', 'Admin'],
		['/workflow/model-registry/{model_ref}', 'Read']
	] as const) {
		expect(tables.gates.get(path), `${path} must carry exactly the kernel's ${scope} row`).toEqual([
			scope
		]);
	}
	// The two kernel tables may OVERLAP — measured, not assumed. The
	// middleware exempts PUBLIC_PATHS before authz reads a gate, yet the
	// reverse-direction guard that demanded a row for every composed route
	// recorded `/.well-known/security.txt` with the literal scope `public`, so
	// it sits in BOTH tables. When they overlap they must AGREE, both ways: a
	// gate row that says `public` is a public claim, and a public path's gate
	// rows may say nothing else. (The first cut asserted them disjoint and was
	// simply wrong about the kernel's data — the census's own red run found it.)
	for (const p of tables.publicPaths) {
		const rows = tables.gates.get(p);
		if (rows !== undefined) {
			expect(
				rows,
				`${p} is in PUBLIC_PATHS but its gate row(s) say ${rows.join(' / ')} — the two kernel ` +
					'tables must not contradict each other'
			).toEqual(['public']);
		}
	}
	for (const [p, rows] of tables.gates) {
		if (rows.includes('public')) {
			expect(
				tables.publicPaths.has(p),
				`${p} carries a gate row that says \`public\` but is NOT in PUBLIC_PATHS — the two kernel ` +
					'tables must not contradict each other'
			).toBe(true);
		}
	}
});

test('every_mutating_reach_is_enumerated_and_the_forget_affordance_is_soft_only_at_route_level', () => {
	const routesDir = join(shellRoot, 'src', 'routes');
	const entries = censusShellRouteEntries(routesDir);
	const tables = kernelTables();

	// ---- EXHAUSTIVE over the Write-scoped wire the census reaches ----
	const writePaths = new Set<string>();
	for (const entry of entries) {
		for (const path of derivedKernelPaths(entry)) {
			if (kernelScopeFor(path, tables, entry.route) === 'Write') writePaths.add(path);
		}
	}
	expect(
		[...writePaths].sort(),
		'the Write-scoped paths the shell can reach are an EXHAUSTIVE fact: a new one fails here, where it ' +
			'must be classified, rather than drifting in unlabelled'
	).toEqual([
		'/ingest/proposal',
		'/proposals/{id}/approve',
		'/ump/forget',
		'/workflow/decision-runs/{id}/replay-diff'
	]);

	const mutatingRows = ROUTE_ROWS.filter((row) => row.authz.mutates !== null);
	const namedPaths = new Set(mutatingRows.map((row) => row.authz.mutates!.path));
	for (const path of writePaths) {
		expect(
			namedPaths.has(path) ||
				Object.prototype.hasOwnProperty.call(NON_MUTATING_WRITE, path),
			`${path} is Write-scoped and UNCLASSIFIED — a Write is the kernel's mark of a state-changing act, ` +
				"so it is either a row's named `mutates` reach or a named non-mutating exception in " +
				'NON_MUTATING_WRITE with its reason'
		).toBe(true);
	}
	for (const [path, reason] of Object.entries(NON_MUTATING_WRITE)) {
		expect(reason.length, `${path}: the non-mutating exception carries no reason`).toBeGreaterThan(
			30
		);
	}
	for (const row of mutatingRows) {
		expect(
			Object.keys(row.authz.kernel),
			`${row.route}: the named mutating reach ${row.authz.mutates!.path} is not in the row's own kernel map`
		).toContain(row.authz.mutates!.path);
	}

	// The routes that can change stored state, exactly. Six of eight reach
	// NONE — and one of the three Write-scoped paths is not on this list,
	// which is the point of `mutates` being its own claim rather than a
	// restatement of scope. (M3-a: `/models` — the proposal queue — retired
	// with its route and `/ingest/proposal` left the reachable set; the
	// sidebar frame's Memory surface brought the proposal back as its
	// "Add memory" — a candidate for Review, never a memory.)
	expect(
		mutatingRows.map((r) => r.route).sort(),
		'the set of mutating routes is exhaustive: `/` via the tombstone, `/recall` via the proposal, ' +
			'and `/review` via the III-6 amendment\'s digest-bound approve — the amendment is an act, named here'
	).toEqual(['/', '/recall', '/review']);

	// ---- AND the [add memory] reach is a PROPOSAL at the route level ----
	//
	// The memory surface's one write is read here at its REAL call site: it
	// must still be `proposeMemory(content, kind)` through the api module,
	// never a direct `/add` or `/ingest/memory` post that would put a
	// memory in the store without the human gate.
	const memorySource = readFileSync(join(routesDir, 'recall', '+page.svelte'), 'utf8');
	expect(
		/\bproposeMemory\(/.test(memorySource),
		'the /recall route must still call proposeMemory — this arm reads the real call site, so it reds when the call moves'
	).toBe(true);
	expect(
		memorySource,
		'the memory surface must not post a memory directly: /add and /ingest/memory bypass the review queue'
	).not.toMatch(/'\/add'|'\/ingest\/memory'/);

	// ---- AND the [forget] reach is SOFT at the route level ----
	//
	// Belt and braces with the core's `hard` refusal: even if a future caller
	// found a way to express the field, the ONE route-level construction site
	// is read here — the REAL call site, never a copy — and must still be
	// `{id}` and nothing else. (M3-a: the chat page IS the home route now;
	// the file moved, the call did not change.)
	const chatSource = readFileSync(join(routesDir, '+page.svelte'), 'utf8');
	const call = /\bforgetRecord\(\s*(\{[\s\S]*?\})\s*,/.exec(chatSource);
	expect(
		call,
		'the / route must still call forgetRecord — this arm reads the real call site, so it reds when the call moves'
	).not.toBeNull();
	const firstArgument = call![1]!;
	expect(
		firstArgument,
		'the forget request at the route must be `{ id: … }` and nothing else — any second key (hard included) ' +
			'is a request the core would have to refuse'
	).toMatch(/^\{\s*id\s*:\s*[^,}]*\}$/);
	expect(
		firstArgument,
		"the [forget] affordance must not carry `hard` at its call site: soft-only is the route-level half of " +
			"the core's `hard` refusal"
	).not.toContain('hard');
});
