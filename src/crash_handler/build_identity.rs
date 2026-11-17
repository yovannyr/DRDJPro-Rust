//! Safe, additive report identity. Native symbol matching additionally requires
//! the module's debug ID; a commit/version alone never proves symbol compatibility.
use serde::{Deserialize, Serialize};

/// Checkout and build configuration for locating candidate symbol bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawIdentity")]
pub struct BuildIdentity {
    commit: Option<String>,
    target: String,
    profile: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIdentity {
    commit: Option<String>,
    target: String,
    profile: String,
}
impl TryFrom<RawIdentity> for BuildIdentity {
    type Error = &'static str;
    fn try_from(raw: RawIdentity) -> Result<Self, Self::Error> {
        let token = |s: &str| {
            !s.is_empty()
                && s.len() <= 80
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        };
        if !token(&raw.target)
            || !token(&raw.profile)
            || raw
                .commit
                .as_ref()
                .is_some_and(|s| s.len() != 40 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("Invalid build identity");
        }
        Ok(Self {
            commit: raw.commit,
            target: raw.target,
            profile: raw.profile,
        })
    }
}
impl BuildIdentity {
    /// Identity embedded by Cargo; missing Git metadata is explicitly None.
    pub fn current() -> Self {
        Self {
            commit: (env!("DRDJ_BUILD_COMMIT") != "unknown")
                .then(|| env!("DRDJ_BUILD_COMMIT").into()),
            target: env!("DRDJ_BUILD_TARGET").into(),
            profile: env!("DRDJ_BUILD_PROFILE").into(),
        }
    }
}
