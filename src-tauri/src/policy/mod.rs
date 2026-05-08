//! T13 L1 — executable policy primitives.

pub mod pack;
pub mod review;
pub mod runtime_profile;
pub mod sync;

use serde::{Deserialize, Serialize};

use crate::orchestrator::state::{Flow, Lane};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrimaryRole {
    #[default]
    Claude,
    Codex,
}

impl PrimaryRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    pub fn lane(self) -> Lane {
        match self {
            Self::Claude => Lane::Claude,
            Self::Codex => Lane::Codex,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    pub primary_role: PrimaryRole,
    pub synthesizer: Lane,
    pub default_reviewer: Lane,
    pub label: String,
}

impl ExecutionPolicy {
    pub fn for_role(primary_role: PrimaryRole) -> Self {
        let lane = primary_role.lane();
        Self {
            primary_role,
            synthesizer: lane,
            default_reviewer: Lane::Codex,
            label: format!("primary-role-{}", primary_role.as_str()),
        }
    }

    pub fn default_reviewer(&self) -> Lane {
        self.default_reviewer
    }

    pub fn mutation_owner_default(&self, flow: Flow) -> Option<Lane> {
        match (self.primary_role, flow) {
            (_, Flow::D) => None,
            (PrimaryRole::Claude, Flow::B) => Some(Lane::Codex),
            (PrimaryRole::Claude, Flow::A | Flow::C) => Some(Lane::Claude),
            (PrimaryRole::Codex, Flow::A | Flow::B | Flow::C) => Some(Lane::Codex),
        }
    }
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self::for_role(PrimaryRole::Claude)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_policy_maps_flows_for_both_primary_roles() {
        let cases = [
            (
                PrimaryRole::Claude,
                Flow::A,
                Some(Lane::Claude),
                Lane::Claude,
                Lane::Codex,
            ),
            (
                PrimaryRole::Claude,
                Flow::B,
                Some(Lane::Codex),
                Lane::Claude,
                Lane::Codex,
            ),
            (
                PrimaryRole::Claude,
                Flow::C,
                Some(Lane::Claude),
                Lane::Claude,
                Lane::Codex,
            ),
            (
                PrimaryRole::Claude,
                Flow::D,
                None,
                Lane::Claude,
                Lane::Codex,
            ),
            (
                PrimaryRole::Codex,
                Flow::A,
                Some(Lane::Codex),
                Lane::Codex,
                Lane::Codex,
            ),
            (
                PrimaryRole::Codex,
                Flow::B,
                Some(Lane::Codex),
                Lane::Codex,
                Lane::Codex,
            ),
            (
                PrimaryRole::Codex,
                Flow::C,
                Some(Lane::Codex),
                Lane::Codex,
                Lane::Codex,
            ),
            (PrimaryRole::Codex, Flow::D, None, Lane::Codex, Lane::Codex),
        ];

        for (role, flow, owner, synthesizer, reviewer) in cases {
            let policy = ExecutionPolicy::for_role(role);
            assert_eq!(policy.mutation_owner_default(flow), owner);
            assert_eq!(policy.synthesizer, synthesizer);
            assert_eq!(policy.default_reviewer(), reviewer);
        }
    }
}
