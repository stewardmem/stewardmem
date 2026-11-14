# Every binding gets the seam: a gate for what the model reads, a budget for what it thinks

*2026. Two gates go up before the volume does. The first extends our
oldest security law to the one path that forgot it — the chat grounding
path, where retrieved memory becomes model context. The second is newer
science: the reasoning-efficiency literature converged on external thinking
budgets as the established intervention, and the engine we adopted carries
the surface — we verified it is not yet a switch, which is exactly the kind
of fact you want before writing the code, not after.*

## Act I: every binding gets the seam

The kernel has a law sentence, in the source, where the retrieval clamp
lives: *"Defense-in-depth at the core: every binding of run_recall gets the
clamp even if a handler forgets it."* The request side earned that sentence
honestly — review-flags are clamped at the core, so a handler that forgets
the clamp cannot ship un-clamped flags.

This cycle we extended the identical law to the output side. The read seam —
PII redaction + invisible-Unicode stripping + markdown-exfil neutralization —
lived in exactly one place: the `/recall` handler's mapping. Every *other*
consumer of the recall outcome received whatever came out of the core.
Re-verifying the tree against our own five-day-old plan showed the chat
grounding path was the one that forgot: it mapped retrieved results into the
model's fenced context and the persisted turn record without crossing the
seam. Same core, different binding, different result — the classic shape the
law sentence exists to kill.

The fix is not a patch in the chat handler. It is the law's extension: one
`sanitize_search_result` function extracted into the gate module, applied at
the outcome boundary — *every* binding of the recall outcome gets the seam,
even if a handler forgets it, forever. The consumers that already sanitized
their own output keep doing so; a second pass over the seam is idempotent by
construction (masked forms never re-match the mask patterns), and now pinned
as a test rather than assumed.

The pins came first, red on purpose. The security pin's failure output *is*
the evidence: on the unfixed tree, the test prints the raw email flowing
into `outcome.tagged` — that failure message is the before-receipt, the
green run is the after-receipt, and the pair goes in the register. Three
leak classes ride the same seam, and each was already armed in the corpus:

- **PII into the model context** — masked deterministically (email, phone,
  Luhn-valid card). The chat case is worse than a display leak: the model
  *faithfully quotes its grounding*, so raw PII in context becomes raw PII
  in answers — and the grounding record persists it as an artifact.
- **Invisible-Unicode smuggling** — zero-width and bidi characters defeating
  naive scans (our own documented case: a card number whose digits were
  invisible). The strip runs on the emitted form, not the raw bytes.
- **Markdown-image exfiltration** (the EchoLeak class) — `![alt](url)`
  renders as an outbound fetch carrying context; the seam neutralizes it to
  its bare alt text before it reaches the model, because a model that echoes
  its grounding can carry the construct outward.

Sequencing note we recorded for ourselves: the fast-chat work (Act II)
*unlocks more chat volume on this exact path*. The seam covers the path
before the volume grows. Gates before traffic; never the reverse order.

## Act II: a budget for what the model thinks

The engine we adopted splits reasoning into its own channel — thinking
arrives separately from the answer, with the delimiters already removed.
That makes thinking *measurable* per request: chunk counts, character
counts, and completion-info token counts, all observable. Our live witness
used those counters to catch a failure class the old engine hid: a short
recall question whose think block consumed the entire response clamp before
the answer began. Empty answer, full clamp, six hundred characters of
scratchpad. On the old engine the same class looked like a silent answer
with the scratchpad smeared through the text; on the new one it has a
number.

The research literature has converged on the intervention: a recent wave of
reasoning-efficiency work (overthinking analyses, reasoning-length control,
consistency-aware early exit) treats *external token budgets* as the
established baseline intervention at inference time — and specifically warns
that naive truncation is unsound: a model pausing between attempts is not a
model that has finished. So the budget lever enters our stack the way every
constant does: preregistered gates (answer completeness, citation identity
with the unbudgeted run, the quality battery, and the latency number that is
the point), env-gated, off by default, deciding per tier from tables.

Verifying the surface before writing the code produced the kind of fact that
saves a day: **the budget is not a chat-session switch.** The configuration
type and its apply-point exist, but nothing in the session path carries it
yet — the wiring is ours to design, which is precisely the kind of work you
want scoped in a plan rather than discovered mid-implementation.

Two more verified facts rounded out the round's ground truth:

- **The prefix rebuild is a safety law, not a quirk.** When a template
  rewrites earlier prompt content (the reasoning replay does — turn 2's
  render carries turn 1's thought), the session rewinds to a verified common
  prefix *when the cache allows* — and the rewind explicitly does not apply
  to recurrent layers. Hybrid models like ours carry recurrent layers. Our
  witness measured the result: reuse works after a tool result (317 tokens
  cached) and rebuilds after an assistant turn (0 cached). The mechanism is
  named; the cost is now measurable per tier; and the design choice
  (suppress the replay vs accept the re-prefill) deserves to be made with
  that number in hand.
- **Speculative decoding via multi-token prediction is gated on the model
  actually carrying the head.** The engine's construction gates on a drafter
  the model must provide. Whether a given community quant ships those
  weights is a load-and-probe question — a weights fact, pinned like every
  weights fact, never assumed.

## The ledger

No technique here is ours: the seam pattern is classic defense-in-depth, the
leak classes are the field's documented ones, the budget literature is the
field's convergence, the engine surfaces are upstream's work. What we add is
the discipline — every gate preregistered, every red-first pin archived as
evidence, every surface verified against the source before it enters a plan,
and every "not yet a switch" recorded as a finding rather than discovered
as a surprise. The next two rounds are written down, falsifiers first: the
seam's receipt, then the budget tables, then the acceptance-rate verdict on
speculative decoding — each a measurement, none a hope.
