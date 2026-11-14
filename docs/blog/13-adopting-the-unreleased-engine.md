# Adopting the unreleased engine: what we learned from mlx-swift-lm main, two months before the tag

*2026. We run our local models through mlx-swift-lm on tags, and the tag we
wanted was ~95 commits behind main. Instead of waiting, we adopted a named
main snapshot behind a preregistration, patched everything that broke,
witnessed the behavior changes live, and found the root cause of a bug class
we had measured months before upstream named it. This post is the science
and the method — the papers we're reading, the surfaces that changed, and
the one integration hazard almost everyone will hit.*

## The posture: measure first, adopt second

Our rule for model-serving constants is boring and strict: nothing moves —
a KV-cache scheme, a speculative-decoding block size, a thinking budget —
without a preregistered decision rule and a measured table on *our* tiers.
Every table pins the engine (tag or named commit) **and the weights revision**
(a re-quantized "same" model is a different artifact silently), and every run
row carries a content hash. "Nothing changed" is a valid, re-earned verdict;
we have preregistered refusals we are proud of, including refusing symmetric
key compression for our model family before spending a single GPU-hour,
because upstream's own family-sensitivity table says qk-norm families are the
ones key compression hurts.

When upstream's release-tracking issue (#648, "Next Release (3.33.0)") sat at
78% checked with a quarterly cadence behind it, we did the math: waiting was
cheap, *measuring twice* was not. So we adopted `main` as a named act — a
pinned snapshot, a disclosed dependency diff, a compile rehearsal that told us
exactly what would break — and paid the adaptation cost early, so the tag-era
day becomes a re-point and a re-verify.

## What broke, and what it taught us

Exactly one compile break, and it was the interesting kind. The engine's
generation stream grew two cases: `.reasoning` and `.steering`. Our stream
pump's exhaustive switch missed them. Mechanical to fix — but the semantic
decision behind the fix is where the science lives.

**Reasoning is no longer part of the text stream.** For framed-protocol
models (the Qwen `<think>` families among them), thinking arrives as
`.reasoning` events with the delimiters already removed, matched by token id
so that prose *discussing* `<think>` cannot misroute. The chat session keeps
the finished thought beside the assistant message and the Qwen templates
replay it as `reasoning_content`. This is a genuinely better contract: the
scratchpad is a named channel instead of text you have to parse out of the
answer stream.

It also breaks two things integrators had hand-rolled:

1. **The stale think-filter.** If your filter primes on the template's
   opening `<think>` and waits for `</think>` in the *text* stream — ours
   did, documented and tested at the old pin — then on the new engine it
   waits forever and suppresses the entire answer. Our live witness caught
   the shape before it could ship: thinking arrived on the `.reasoning`
   channel (141 chunks / 583 characters), the answer streamed clean in
   `chunk`, and no `</think>` ever appeared in it. The fix is one line with
   a big comment: the first `.reasoning` event is the engine announcing it
   owns the scratchpad — the filter stands down.

2. **The prefix-instability mechanism, finally named.** Months ago we
   measured a finding we filed away: on our hybrid tiers, the second turn
   of a chat reported `cachedPromptTokenCount == 0` — the whole prompt
   re-prefilled — while a standard-attention control reported a healthy
   cached count. On the new engine, with the reasoning fixes merged
   upstream, we re-ran the witness and got the split that names the cause:
   **cache reuse works after a tool result (317 tokens cached) and breaks
   after an assistant turn (0 cached).** The difference is the thought
   replay: turn 2's prompt render now carries turn 1's reasoning as
   `reasoning_content`, which turn 1's prompt never contained — so the
   prefix diverges exactly at the assistant message. That's not a bug in
   our session shape or in the engine; it's a design decision with a
   latency cost on long contexts, and it deserves to be a *choice*
   (suppress the replay for prefix stability, or accept the re-prefill)
   rather than an accident.

## What the new engine generation brings

- **Quantized KV caches with documented costs.** The asymmetric schemes
  (compressed values, near-lossless keys — softmax amplifies key error,
  values only average) run through JIT-compiled Metal kernels built once per
  model shape, prefill untouched, with `quantizedKVStart` letting the cache
  stay exact fp16 for short contexts and quantize from a threshold. The
  documented decode numbers on a 1.7B-class model: fp16 150 tok/s, turbo8v3
  114, turbo4 122, turbo0v4 102 — and the family-sensitivity table is why
  "fastest" and "healthy for your models" are different columns.
- **The MTP speculative-decoding path**, with sliding-window-aware block
  clamping (`max(2, narrowest + 1)` across the caches you're actually
  running) and acceptance-rate telemetry on the completion info — so the
  classic question "is speculative decoding actually accepting on MY model?"
  is a measurement, not a hope.
- **Thinking budgets.** Cap the scratchpad per request, with
  `reasoningTokenCount` / `answerTokenCount` reported back. Our witness
  showed the failure class this addresses on reasoning models: a short
  recall question whose think block consumed the entire token clamp before
  the answer began. Bounding the scratchpad is a latency feature *and* a
  correctness feature.
- **Per-layer cache observability** — `memoryBytes`, capacity disposition,
  evicted-token counts — so the cache's own accounting is measurable instead
  of derived.

## The papers on our desk

The hybrid-attention (linear-attention + full-attention mix) decoding
literature is having its speculative-decoding moment: ReplaySSM
(2610.13778) turns recurrent rollback into buffer-pointer ops; STree
(2505.14969) does speculative tree verification for hybrid SSMs; SpecMamba
(2509.19873) takes it to constrained hardware; RAD (2505.22135) uses
self-speculation as a redundancy diagnostic; HybridSpec (2611.04055),
SpecOPD (2611.02846) and H-Spec (2609.24197) attack verification-length
selection, target positioning, and drafter-side KV respectively. And the
older rule that still governs all of it: RULER (2404.06654) — *standard
needle-in-a-haystack overestimates long-context ability*. We measure at five
depths and record where the baseline itself stops resolving; the boundary is
the data.

## The honest ledger

We did not invent any of the techniques above; the papers are ahead of
everyone's implementation, ours included. What we hold is the apparatus that
decides which techniques survive contact with our models: preregistered
gates, both-axis pins, refusal ledgers, and witnesses that run before
behavior ships. Adopting main early cost us one compile break, one scoped
filter fix, and a root cause we can finally name. The tag will land when it
lands (the maintainers answered our cadence question: roughly every two
months, PR-dependent). When it does, the day is a re-point, a re-verify, and
the measurement — because the adaptation debt is already paid.

*The engine pins itself: every number above was taken against a named
snapshot (`63529075…`) and re-derivable from the recorded tables. The
reproducibility recipe — engine revision + weights revision + run hash — is
the point.*
