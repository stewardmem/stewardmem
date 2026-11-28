//! The registry's closed vocabulary: every scaffold kind, status, warrant
//! and split a table column can hold, the verdict a sign re-derives, and the
//! refusal codes the surface answers with. All of it closed — a new word is
//! a vocabulary change, which is a migration and a CHECK list, never a
//! string.

/// What a scaffold changes. A-1 validates exactly one kind in SI0; the rest
/// exist so the CHECK lists and the refusals can name themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaffoldKind {
    Prompt,
    Frame,
    Retrieval,
    ToolPolicy,
    Routing,
    DiffFormat,
}

impl ScaffoldKind {
    /// The stored and wire form; the migration's CHECK list is built from
    /// these names.
    pub fn as_str(self) -> &'static str {
        match self {
            ScaffoldKind::Prompt => "prompt",
            ScaffoldKind::Frame => "frame",
            ScaffoldKind::Retrieval => "retrieval",
            ScaffoldKind::ToolPolicy => "tool_policy",
            ScaffoldKind::Routing => "routing",
            ScaffoldKind::DiffFormat => "diff_format",
        }
    }
}

impl TryFrom<&str> for ScaffoldKind {
    type Error = Refusal;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "prompt" => Ok(ScaffoldKind::Prompt),
            "frame" => Ok(ScaffoldKind::Frame),
            "retrieval" => Ok(ScaffoldKind::Retrieval),
            "tool_policy" => Ok(ScaffoldKind::ToolPolicy),
            "routing" => Ok(ScaffoldKind::Routing),
            "diff_format" => Ok(ScaffoldKind::DiffFormat),
            _ => Err(Refusal::ScaffoldKindUnvalidated),
        }
    }
}

/// Where a scaffold sits in its life: a candidate until a sign promotes it,
/// then the one active row of its kind, then retired. Nothing is ever
/// deleted — retired is the only exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaffoldStatus {
    Candidate,
    Active,
    Retired,
}

impl ScaffoldStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ScaffoldStatus::Candidate => "candidate",
            ScaffoldStatus::Active => "active",
            ScaffoldStatus::Retired => "retired",
        }
    }
}

/// The evidence a proposal carries: a cited trace, or the explicit statement
/// that none applies. Unverifiable is refused at sign.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Warrant {
    Cited,
    NotApplicable,
}

impl Warrant {
    pub fn as_str(self) -> &'static str {
        match self {
            Warrant::Cited => "cited",
            Warrant::NotApplicable => "not_applicable",
        }
    }
}

/// Which split a bench run scored. A sign needs both: a candidate measured
/// only on the dev split cannot pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Split {
    Dev,
    HeldOut,
}

impl Split {
    pub fn as_str(self) -> &'static str {
        match self {
            Split::Dev => "dev",
            Split::HeldOut => "held_out",
        }
    }
}

/// The verdict a sign re-derives from the stored runs. The caller's claim is
/// never read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    Inconclusive,
}

/// The refusal codes, all new, all closed: the exact strings the sign
/// answers with, in the order the sign's checks apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    ScaffoldKindUnvalidated,
    DiffOverComplexityCap,
    WarrantUnverified,
    HeldOutNotMeasured,
    RawOutputMismatch,
    ProposerCannotSign,
    DigestRequired,
    DigestMismatch,
    ProposalNotFound,
    GainInsideNoise,
}

impl Refusal {
    /// The wire form.
    pub fn as_str(self) -> &'static str {
        match self {
            Refusal::ScaffoldKindUnvalidated => "scaffold_kind_unvalidated",
            Refusal::DiffOverComplexityCap => "diff_over_complexity_cap",
            Refusal::WarrantUnverified => "warrant_unverified",
            Refusal::HeldOutNotMeasured => "held_out_not_measured",
            Refusal::RawOutputMismatch => "raw_output_mismatch",
            Refusal::ProposerCannotSign => "proposer_cannot_sign",
            Refusal::DigestRequired => "digest_required",
            Refusal::DigestMismatch => "digest_mismatch",
            Refusal::ProposalNotFound => "proposal_not_found",
            Refusal::GainInsideNoise => "gain_inside_noise",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
