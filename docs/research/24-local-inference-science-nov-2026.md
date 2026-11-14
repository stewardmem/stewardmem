# 24 · The Local-Inference Science Round (November 2026): quantized KV, speculative decoding on hybrids, reasoning budgets, and the split that made thinking a channel

*The problem.* The local tier serves chat from a 16 GB machine, and three
questions decide whether that tier is fast: how much memory the attention
cache costs, how much of the decode work can be speculated instead of
computed, and how much of the generation is scratchpad the user never asked
for. The engine beneath the tier (mlx-swift-lm) moved ~95 commits past its
last tag, carrying new surfaces for all three — and one of those surfaces
changed a contract our integration code depended on. This file records the
science behind the round: what the literature says, what the engine actually
ships, and what our instruments measured when both were put in contact.

*The method.* Every claim here entered with at least two independent
sources: the engine's documented surface plus the in-tree source at the
pinned snapshot, or a paper's landing page plus a measurement of our own.
Literature is cited as literature — nothing below claims a paper was
implemented. The full measurement tables live in the round records and
[blog 13](../blog/13-adopting-the-unreleased-engine.md) and
[blog 14](../blog/14-every-binding-gets-the-seam.md) carry the public
narratives.

## The KV-cache asymmetry is the science

The load-bearing fact of quantized caching: attention quality is far more
sensitive to **key** error than to value error, because softmax amplifies
key error while value error only averages linearly. The engine's TurboQuant
schemes are built on that asymmetry — rotate vectors with a Walsh-Hadamard
transform, quantize against Lloyd-Max codebooks — and upstream's own
family-sensitivity table
([PR #232](https://github.com/ml-explore/mlx-swift-lm/pull/232)) states it
operationally: *"Qwen3 / qk-norm families and small models: most sensitive to
key compression; asymmetric schemes stay healthy."* For qk-norm families,
"fastest" and "healthy for your models" are different columns, and the
healthy column is the one with near-lossless keys.

The documented decode costs (Qwen3-1.7B-class, M5 Max, fp16 baseline 150
tok/s): turbo8v3 **114**, turbo4 **122**, turbo0v4 **102** — prefill
unaffected (the cache stores raw fp16 during prompt processing and compresses
at the first decode step). Kernels are JIT-compiled Metal, built once per
model shape. `quantizedKVStart` makes quantization **length-scoped**: the
cache stays exact fp16 for the first N tokens and converts dynamically inside
the token iterator — short-context turns keep exact precision while long
sessions compress from a threshold, which is the shape long-context
measurement ladders want anyway.

The guard we run alongside: symmetric key compression is refused *by name,
before any run*, on the sensitivity table's own evidence. A refusal that
cites upstream's data costs zero GPU-hours and prevents the silent-quality
failure mode entirely.

## Speculative decoding on hybrid-attention models: rollback is the crux

Hybrid tiers (recurrent linear-attention layers interleaved with
full-attention layers) make speculative decoding harder in one specific way:
a recurrent layer updates a fixed-size state per token, so "reject the last
k tokens" is not the free rewind a KV cache allows. The 2026 literature
converges on that seam from several directions:

- **ReplaySSM** ([2610.13778](https://arxiv.org/abs/2610.13778)) — unroll
  the recurrence so rollback becomes a buffer-pointer operation rather than
  state reconstruction; also addresses state I/O intensity.
- **STree** ([2505.14969](https://arxiv.org/abs/2505.14969)) — speculative
  *tree* verification for hybrid state-space models: verification-shape
  selection, not just block length.
- **SpecMamba** ([2509.19873](https://arxiv.org/abs/2509.19873)) — the
  constrained-hardware end of the same problem.
- **RAD** ([2505.22135](https://arxiv.org/abs/2505.22135)) — self-speculative
  distillation as a redundancy diagnostic for hybrids.
- **HybridSpec** ([2611.04055](https://arxiv.org/abs/2611.04055)),
  **SpecOPD** ([2611.02846](https://arxiv.org/abs/2611.02846)),
  **H-Spec** ([2609.24197](https://arxiv.org/abs/2609.24197)) —
  verification-length selection for GDN-MoE decoding, target positioning
  toward GDN-sparse deployments, and drafter-side KV-free speculation
  respectively.

The engine ships the multi-token-prediction path
([#351](https://github.com/ml-explore/mlx-swift-lm/pull/351)) with
sliding-window-aware block clamping: the speculative block is clamped to
`max(2, narrowest + 1)` across the caches actually in use — the narrowest
sliding window bounds the round, floor 2 — and acceptance-rate telemetry
arrives on the completion info, so "is speculation actually accepting on MY
model?" is a measurement, not a hope. One prerequisite is a **weights fact**:
the path gates on the model carrying the MTP head, and whether a community
quant ships those weights is answered by loading and probing, never assumed.

## The reasoning split: thinking became a channel

The generation stream now carries a `.reasoning` case — for framed-protocol
models (the Qwen `<think>` families via a reasoning configuration, the
framed token protocols), thinking arrives **separately from the answer, with
the delimiters already removed**, and the delimiters are matched by token id
so prose that discusses `<think>` cannot misroute. Unframed models stream
exactly as before.

Two consequences integrations need to know about, both measured here:

1. **The stale-filter hazard.** A hand-rolled think-block filter that primes
   on the template's opening tag and waits for `</think>` in the *text*
   stream will wait forever — the scratchpad no longer arrives there — and
   suppress the whole answer. The first `.reasoning` event is the engine
   announcing it owns the scratchpad; the filter stands down. Our witness
   measured the shape directly: 141 reasoning chunks / 583 characters on the
   reasoning channel, the answer streaming clean in chunks, zero closing
   tags in the text stream.
2. **The prefix trade.** The session keeps the finished thought beside the
   assistant message and the Qwen templates replay it as `reasoning_content`
   — which means turn 2's prompt render carries content turn 1's prompt
   never had, and the prefix diverges exactly at the assistant turn. Our
   witness measured both sides: cache reuse **works** after a tool result
   (317 tokens cached) and **rebuilds** after an assistant turn (0 cached).
   This also explains the prefix instability we measured at the previous
   pin months earlier: the mechanism is the replay, and the rebuild is the
   engine's *safety law* — the rewind-to-common-prefix machinery explicitly
   does not acquire on recurrent layers, so a trim across the assistant turn
   is not safe on hybrid models. The design choice (suppress the replay for
   prefix stability, or accept the re-prefill) is a session-shape decision
   that deserves the measured re-prefill cost next to it, not a guess.

The companion lever is the **thinking budget**: bound the scratchpad per
request, with reasoning/answer token counts reported back. The reasoning-
efficiency literature converged on external budget control as the
established inference-time intervention — *Routes of Reasoning*
([2611.05919](https://arxiv.org/abs/2611.05919)) names "token budgets or
steering" as the existing baseline; *CALM*
([2610.25908](https://arxiv.org/abs/2610.25908)) supplies the caution that
naive truncation is unsound (a model pausing between attempts has not
finished), which is why a budget's quality gate must be consistency-shaped:
the answer completes, the citations hold. Our witness measured the failure
class the budget addresses on our tiers: a short recall question whose think
block consumed the entire response clamp before the answer began — empty
answer, full clamp, ~600 characters of scratchpad. The verified engine fact:
the budget configuration exists but nothing in the session path carries it
yet, so the wiring is consumer work — scoped in the round plan, not
discovered mid-implementation.

## The shipped answer

The round's public artifacts: the adoption of the named main snapshot with a
disclosed dependency diff
([blog 13](../blog/13-adopting-the-unreleased-engine.md)); the live witness
and the scoped filter fix ([blog 14](../blog/14-every-binding-gets-the-seam.md));
the grounding path's read-seam extension, red-first (the security pin's
failure output on the unfixed tree is the before-receipt); and the
preregistration that precedes the KV-scheme measurement
([the REKRA preregistration](../REKRA_PLAN_PREREGISTRATION_2026-11-14.md) —
engine pin *and* weights pin, the refusal ledger, the decision rule). The
upstream cadence question we filed got answered directly (#797: roughly
every two months, PR-dependent), and the release itself is tracked upstream
with a public checklist (#648) — the tag lands when it lands, and the
adaptation debt is already paid.

## What we would claim, and what we don't

No technique here is ours — the asymmetry is upstream's documented design,
the rollback problem is the literature's, the budget intervention is the
field's baseline, the split is the engine's. What the round adds is the
apparatus: preregistered gates that let "nothing changed" be a valid verdict,
both-axis reproducibility pins (engine *and* weights revision), refusal
ledgers that make scope changes named rows, and witnesses that run before
behavior ships. The falsifiers are written down first; the outcomes, when
they land, get recorded whether they flatter the plan or not.
