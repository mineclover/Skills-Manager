use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const SKILL_CONTRACT_FILE_NAME: &str = "skill-manager.yaml";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillContractStatus {
    Unmanaged,
    Incomplete,
    Managed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillContractSource {
    PortableSidecar,
    LocalMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SkillContractPurpose {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub use_when: Vec<String>,
    #[serde(default)]
    pub avoid_when: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SkillContractRequirements {
    #[serde(default)]
    pub runtimes: Vec<String>,
    #[serde(default)]
    pub project_signals: Vec<String>,
    #[serde(default)]
    pub verification: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SkillContractSuccess {
    #[serde(default)]
    pub expected_outcomes: Vec<String>,
    #[serde(default)]
    pub non_goals: Vec<String>,
    #[serde(default)]
    pub safety_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SkillContractFeedback {
    #[serde(default)]
    pub codes: Vec<String>,
    #[serde(default)]
    pub required_for_completed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SkillContractEvaluation {
    #[serde(default)]
    pub cases: Vec<String>,
    pub review_cycle_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SkillContract {
    pub schema_version: Option<u32>,
    #[serde(default)]
    pub purpose: SkillContractPurpose,
    #[serde(default)]
    pub requirements: SkillContractRequirements,
    #[serde(default)]
    pub success_contract: SkillContractSuccess,
    #[serde(default)]
    pub feedback: SkillContractFeedback,
    #[serde(default)]
    pub evaluation: SkillContractEvaluation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillContractSummary {
    pub status: SkillContractStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<SkillContract>,
    #[serde(default)]
    pub validation_errors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SkillContractSource>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveLocalSkillContractRequest {
    pub skill_instance_id: String,
    pub contract: SkillContract,
}

impl SkillContractSummary {
    pub fn load(skill_path: &Path) -> Self {
        let sidecar_path = skill_path.join(SKILL_CONTRACT_FILE_NAME);
        if !sidecar_path.exists() {
            return Self::unmanaged();
        }

        match fs::read_to_string(&sidecar_path) {
            Ok(contents) => match serde_yaml::from_str::<SkillContract>(&contents) {
                Ok(contract) => {
                    let validation_errors = contract.validate();
                    Self {
                        status: if validation_errors.is_empty() {
                            SkillContractStatus::Managed
                        } else {
                            SkillContractStatus::Incomplete
                        },
                        path: Some(sidecar_path),
                        contract: Some(contract),
                        validation_errors,
                        source: Some(SkillContractSource::PortableSidecar),
                    }
                }
                Err(error) => Self {
                    status: SkillContractStatus::Incomplete,
                    path: Some(sidecar_path),
                    contract: None,
                    validation_errors: vec![format!("Invalid YAML: {error}")],
                    source: Some(SkillContractSource::PortableSidecar),
                },
            },
            Err(error) => Self {
                status: SkillContractStatus::Incomplete,
                path: Some(sidecar_path),
                contract: None,
                validation_errors: vec![format!("Unable to read contract: {error}")],
                source: Some(SkillContractSource::PortableSidecar),
            },
        }
    }

    pub fn from_local_metadata(contract: SkillContract) -> Self {
        let validation_errors = contract.validate();
        Self {
            status: if validation_errors.is_empty() {
                SkillContractStatus::Managed
            } else {
                SkillContractStatus::Incomplete
            },
            path: None,
            contract: Some(contract),
            validation_errors,
            source: Some(SkillContractSource::LocalMetadata),
        }
    }

    pub fn unmanaged() -> Self {
        Self {
            status: SkillContractStatus::Unmanaged,
            path: None,
            contract: None,
            validation_errors: Vec::new(),
            source: None,
        }
    }
}

impl SkillContract {
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.schema_version != Some(1) {
            errors.push("schema_version must be 1".to_string());
        }
        if self.purpose.summary.trim().is_empty() {
            errors.push("purpose.summary is required".to_string());
        }
        if self.purpose.use_when.is_empty() {
            errors.push("purpose.use_when must include at least one case".to_string());
        }
        if self.purpose.avoid_when.is_empty() {
            errors.push("purpose.avoid_when must include at least one case".to_string());
        }
        if self.requirements.verification.is_empty() {
            errors.push("requirements.verification must include a check".to_string());
        }
        if self.success_contract.expected_outcomes.is_empty() {
            errors.push("success.expected_outcomes must include an outcome".to_string());
        }
        if self.success_contract.non_goals.is_empty() {
            errors.push("success.non_goals must include a boundary".to_string());
        }
        if self.success_contract.safety_rules.is_empty() {
            errors.push("success.safety_rules must include a rule".to_string());
        }
        if self.feedback.codes.is_empty() {
            errors.push("feedback.codes must include at least one code".to_string());
        }
        if self.feedback.required_for_completed.is_empty() {
            errors.push("feedback.required_for_completed must include evidence".to_string());
        }
        if self.evaluation.cases.is_empty() {
            errors.push("evaluation.cases must include at least one case".to_string());
        }
        if self.evaluation.review_cycle_days.unwrap_or(0) == 0 {
            errors.push("evaluation.review_cycle_days must be greater than zero".to_string());
        }
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const VALID_CONTRACT: &str = r#"
schema_version: 1
purpose:
  summary: Test contract
  use_when: ["test this behavior"]
  avoid_when: ["use another skill"]
requirements:
  runtimes: ["node"]
  project_signals: ["package.json"]
  verification: ["cargo test"]
success_contract:
  expected_outcomes: ["verified result"]
  non_goals: ["production rollout"]
  safety_rules: ["do not mutate data"]
feedback:
  codes: ["completed", "partial"]
  required_for_completed: ["test output"]
evaluation:
  cases: ["evaluations/happy-path.md"]
  review_cycle_days: 30
"#;

    #[test]
    fn missing_sidecar_is_unmanaged() {
        let directory = tempdir().unwrap();
        let summary = SkillContractSummary::load(directory.path());
        assert_eq!(summary.status, SkillContractStatus::Unmanaged);
        assert!(summary.contract.is_none());
    }

    #[test]
    fn valid_sidecar_is_managed() {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join(SKILL_CONTRACT_FILE_NAME),
            VALID_CONTRACT,
        )
        .unwrap();
        let summary = SkillContractSummary::load(directory.path());
        assert_eq!(summary.status, SkillContractStatus::Managed);
        assert_eq!(summary.contract.unwrap().purpose.summary, "Test contract");
    }

    #[test]
    fn incomplete_sidecar_reports_each_missing_convention() {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join(SKILL_CONTRACT_FILE_NAME),
            "schema_version: 1\npurpose:\n  summary: Test\n",
        )
        .unwrap();
        let summary = SkillContractSummary::load(directory.path());
        assert_eq!(summary.status, SkillContractStatus::Incomplete);
        assert!(summary
            .validation_errors
            .iter()
            .any(|error| error.contains("purpose.use_when")));
        assert!(summary
            .validation_errors
            .iter()
            .any(|error| error.contains("evaluation.cases")));
    }

    #[test]
    fn invalid_yaml_is_incomplete_without_panicking() {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join(SKILL_CONTRACT_FILE_NAME),
            "purpose: [not valid",
        )
        .unwrap();
        let summary = SkillContractSummary::load(directory.path());
        assert_eq!(summary.status, SkillContractStatus::Incomplete);
        assert!(summary.validation_errors[0].starts_with("Invalid YAML:"));
    }
}
