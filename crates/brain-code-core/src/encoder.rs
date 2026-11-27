//! The encoder port (P-5). The crate never loads a model and makes no
//! network call: the kernel fills this port with the active profile's
//! embedder, and the index records the port's dimension and profile name in
//! `code_repo`. A profile change is a rebuild, never a silent mix.
//!
//! Vectors are deterministic inputs to the index — the crate cannot tell a
//! real embedder from a fake one, and must not try. Tests use a
//! hash-derived fake so every vector behaviour is reproducible.

use crate::error::CodeIndexError;

pub trait Encoder {
    /// The profile name recorded in `code_repo.embed_profile`.
    fn profile(&self) -> &str;
    /// The vector width recorded in `code_repo.embed_dim` and baked into the
    /// vec0 DDL.
    fn dim(&self) -> usize;
    /// Embed each text, in order, one vector per input.
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, CodeIndexError>;
}

#[cfg(test)]
pub(crate) mod fake {
    use super::*;
    use sha2::{Digest, Sha256};

    /// A deterministic stand-in: each text hashes to a fixed-width vector of
    /// values in [-1, 1). Same text in, same vector out, forever.
    pub(crate) struct FakeEncoder {
        profile: String,
        dim: usize,
    }

    impl FakeEncoder {
        pub(crate) fn new(profile: &str, dim: usize) -> Self {
            Self {
                profile: profile.to_string(),
                dim,
            }
        }
    }

    impl Encoder for FakeEncoder {
        fn profile(&self) -> &str {
            &self.profile
        }

        fn dim(&self) -> usize {
            self.dim
        }

        fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, CodeIndexError> {
            Ok(texts
                .iter()
                .map(|text| {
                    let digest = Sha256::digest(text.as_bytes());
                    (0..self.dim)
                        .map(|i| {
                            let byte = digest[(i * 5) % digest.len()] as f32
                                + digest[(i * 7 + 1) % digest.len()] as f32 / 256.0;
                            (byte / 255.0) * 2.0 - 1.0
                        })
                        .collect::<Vec<f32>>()
                })
                .collect())
        }
    }
}
