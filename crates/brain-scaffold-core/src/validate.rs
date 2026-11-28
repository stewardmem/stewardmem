//! The body gate: which kinds are validated this round and how big a
//! candidate body may be. The cap is a CONSTRAINT, not a tie-break (C-8,
//! note 08 N-9) — over the cap is a refusal, never a score.

use crate::rule::VALIDATED_KINDS;
use crate::vocab::{Refusal, ScaffoldKind};

/// The provisional diff-size cap, in lines. SI0's law is that the cap exists
/// and refuses; SI6 sets the real one from the diffs the rounds actually
/// accept. A diff larger than this is not a single-kind candidate.
pub const DIFF_MAX_LINES: usize = 200;

/// Validate one candidate body against the round's kind gate and the cap.
/// The kind gate first: an unvalidated kind refuses even when its body is
/// small.
pub fn validate_body(kind: ScaffoldKind, body: &str) -> Result<(), Refusal> {
    if !VALIDATED_KINDS.contains(&kind) {
        return Err(Refusal::ScaffoldKindUnvalidated);
    }
    if body.lines().count() > DIFF_MAX_LINES {
        return Err(Refusal::DiffOverComplexityCap);
    }
    Ok(())
}
