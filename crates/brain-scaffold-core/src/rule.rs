//! The A-6 rule as pinned constants and one pure function. The rule (note 02
//! §7): the primary is file-level recall@10 on the fused arm; a candidate
//! passes a paired sign test whose one-sided tail at `b` of `b + c` must sit
//! at or below 0.05 divided by the candidates tried; fewer than ten changed
//! entries cannot pass at any `k`; and the guards are non-inferior — a guard
//! that loses more entries than it gains is fail. The arithmetic is exact in
//! u128 or the verdict refuses as inconclusive: a verdict the crate cannot
//! compute exactly is never a pass.

use crate::vocab::{ScaffoldKind, Verdict};

/// The metric the sign test scores, one level per entry, paired against the
/// active baseline on the same held-out entries.
pub const PRIMARY_METRIC: &str = "file_level_recall_at_10_fused";

/// The selection level, as an exact fraction: 0.05 = 1/20. A float literal
/// would let the level drift below what the pins pin.
pub const ALPHA_NUMERATOR: u64 = 1;
pub const ALPHA_DENOMINATOR: u64 = 20;

/// R-4: a change that moves fewer than ten entries is noise on any set this
/// size; it cannot pass at any k.
pub const MIN_CHANGED_ENTRIES: u32 = 10;

/// The guard list (R-1, R-5): the fused arm's recall@1 and recall@5, and
/// recall@10 on both strata. [`GuardCounts`] is this list's type-level twin
/// — the four names must stay in step, and the pins hold both.
pub const GUARDS: [&str; 4] = [
    "recall_at_1",
    "recall_at_5",
    "recall_at_10_with_identifier",
    "recall_at_10_without",
];

/// A-1: the kinds validated this round. Every other kind is refused.
pub const VALIDATED_KINDS: &[ScaffoldKind] = &[ScaffoldKind::Retrieval];

/// One guard metric's paired entry counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pair {
    pub improved: u32,
    pub worsened: u32,
}

/// The four guards' paired counts, in [`GUARDS`]' order. The named fields
/// are the list's type-level enforcement: adding or renaming a guard is a
/// type change every pin re-approves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuardCounts {
    pub recall_at_1: Pair,
    pub recall_at_5: Pair,
    pub recall_at_10_with_identifier: Pair,
    pub recall_at_10_without: Pair,
}

impl GuardCounts {
    /// A guard regression: the guard loses more entries than it gains. A tie
    /// is not a regression.
    fn any_regression(self) -> bool {
        [
            self.recall_at_1,
            self.recall_at_5,
            self.recall_at_10_with_identifier,
            self.recall_at_10_without,
        ]
        .iter()
        .any(|pair| pair.worsened > pair.improved)
    }
}

/// The paired counts one verdict decides on: `improved`/`worsened` for the
/// primary metric, the candidates tried so far — read from the registry, an
/// argument here, never a constant the caller can drop — and the guards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerdictInput {
    pub improved: u32,
    pub worsened: u32,
    pub candidates_tried: u32,
    pub guards: GuardCounts,
}

/// The verdict, in the rule's order: a guard regression is fail; below the
/// floor is inconclusive; then the exact one-sided tail against the
/// selection level decides pass; anything the arithmetic cannot settle
/// exactly is inconclusive — refuse, never guess.
pub fn decide_verdict(input: &VerdictInput) -> Verdict {
    if input.guards.any_regression() {
        return Verdict::Fail;
    }
    let changed = input.improved.saturating_add(input.worsened);
    if changed < MIN_CHANGED_ENTRIES {
        return Verdict::Inconclusive;
    }
    if input.candidates_tried == 0 {
        // No candidate has been tried against this baseline, so no selection
        // level exists. Refuse rather than pass through a division by zero.
        return Verdict::Inconclusive;
    }
    match tail_at_most_selection_level(input.improved, input.worsened, input.candidates_tried) {
        Some(true) => Verdict::Pass,
        _ => Verdict::Inconclusive,
    }
}

/// `Some(true)` when the one-sided exact binomial tail at `b` of `b + c` is
/// at or below the selection level 0.05/k; `Some(false)` when it is above;
/// `None` when the exact arithmetic cannot be represented — which refuses.
///
/// The comparison is kept in integers: the tail Σ C(n, i) for i in b..=n has
/// denominator 2^n, and the level 1/(20k), so the test is
/// `20·k·Σ ≤ 2^n`. n is capped at 126 because 2^n must fit u128 exactly;
/// the pinned table stops at m = 100 and the frozen set is 46 entries, so a
/// larger set is refused rather than approximated.
fn tail_at_most_selection_level(b: u32, c: u32, k: u32) -> Option<bool> {
    let n = u64::from(b) + u64::from(c);
    if n == 0 {
        return None;
    }
    if n > 126 {
        return None;
    }
    let tail_sum = binomial_tail_sum(n, u64::from(b))?;
    let two_pow_n = 1u128.checked_shl(n as u32)?;
    let levelled = tail_sum
        .checked_mul(u128::from(ALPHA_DENOMINATOR))
        .and_then(|v| v.checked_mul(u128::from(k)));
    // overflow ⇒ the product exceeds any representable bound, so it exceeds 2^n
    Some(levelled.is_some_and(|product| product <= two_pow_n))
}

/// Σ C(n, i) for i in b..=n, exact in u128: C(n, b) by the multiplicative
/// formula, then the exact upward step C(n, i + 1) = C(n, i)·(n − i)/(i + 1).
/// `None` on any overflow — refuse rather than guess.
fn binomial_tail_sum(n: u64, b: u64) -> Option<u128> {
    if b > n {
        return None;
    }
    let mut coef = binomial(n, b)?;
    let mut sum = coef;
    let mut i = b;
    while i < n {
        coef = coef
            .checked_mul(u128::from(n - i))
            .and_then(|v| v.checked_div(u128::from(i + 1)))?;
        i += 1;
        sum = sum.checked_add(coef)?;
    }
    Some(sum)
}

/// C(n, k) exact by the multiplicative formula: after step j the running
/// product is C(n − k + j, j), so every division is exact. `None` on
/// overflow.
fn binomial(n: u64, k: u64) -> Option<u128> {
    if k > n {
        return None;
    }
    let k = k.min(n - k);
    let mut result = 1u128;
    let mut j = 1u64;
    while j <= k {
        result = result
            .checked_mul(u128::from(n - k + j))
            .and_then(|v| v.checked_div(u128::from(j)))?;
        j += 1;
    }
    Some(result)
}
