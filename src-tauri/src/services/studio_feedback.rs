use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::models::{
    ActivationRun, EvaluationRecord, RecordEvaluationRequest, RecordStudioFeedbackRequest,
    ReleaseHealth, ReviewQueueItem, ReviewReason, StudioFeedbackCode, StudioFeedbackEvent,
    StudioFeedbackTargetKind, StudioHealthStatus,
};
use crate::services::SkillSetService;

const MIN_EVALUATED_SAMPLE: u64 = 5;
const REVIEW_WINDOW_DAYS: i64 = 90;
const MIN_SUCCESS_RATE: f64 = 0.80;
const MAX_CORRECTION_RATE: f64 = 0.20;
const MAX_SCOPE_MISMATCH_RATE: f64 = 0.10;

pub struct StudioFeedbackService;

impl StudioFeedbackService {
    fn db_path() -> PathBuf {
        crate::models::home_dir()
            .unwrap_or_default()
            .join(".skills-manager")
            .join("studio-feedback.db")
    }

    fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_secs() as i64)
            .unwrap_or(0)
    }

    fn open() -> Result<Connection, String> {
        let path = Self::db_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create Studio database directory: {error}"))?;
        }
        let conn = Connection::open(path)
            .map_err(|error| format!("Failed to open Studio database: {error}"))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS studio_feedback_events (
                id TEXT PRIMARY KEY, target_kind TEXT NOT NULL, target_id TEXT NOT NULL, code TEXT NOT NULL,
                evidence_type TEXT NOT NULL, evidence_summary TEXT NOT NULL, project_id TEXT, work_scope TEXT,
                provider_id TEXT, created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS studio_feedback_target_idx ON studio_feedback_events(target_kind, target_id, created_at);
            CREATE TABLE IF NOT EXISTS studio_evaluation_records (
                id TEXT PRIMARY KEY, release_id TEXT NOT NULL, case_id TEXT NOT NULL, status TEXT NOT NULL,
                evidence_type TEXT NOT NULL, evidence_summary TEXT NOT NULL, project_id TEXT, work_scope TEXT,
                provider_id TEXT, created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS studio_evaluation_release_idx ON studio_evaluation_records(release_id, created_at);
            CREATE TABLE IF NOT EXISTS studio_activation_runs (
                id TEXT PRIMARY KEY, assignment_id TEXT NOT NULL, release_id TEXT NOT NULL, project_id TEXT,
                work_scope TEXT NOT NULL, applied_count INTEGER NOT NULL, skipped_count INTEGER NOT NULL,
                failed_count INTEGER NOT NULL, created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS studio_activation_release_idx ON studio_activation_runs(release_id, created_at);"
        ).map_err(|error| format!("Failed to initialize Studio database: {error}"))?;
        Ok(conn)
    }

    fn text<T: serde::Serialize>(value: &T) -> Result<String, String> {
        serde_json::to_value(value)
            .map_err(|error| error.to_string())
            .and_then(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| "Could not serialize Studio enum".to_string())
            })
    }

    fn redact_summary(raw: &str) -> String {
        let bounded: String = raw.trim().chars().take(2000).collect();
        let redacted = regex::Regex::new(r"(?i)\b(sk-[a-z0-9_-]{16,}|ghp_[a-z0-9]{20,}|github_pat_[a-z0-9_]{20,}|AKIA[0-9A-Z]{16})\b")
            .expect("static redaction regex")
            .replace_all(&bounded, "[REDACTED]");
        redacted.into_owned()
    }

    fn optional(value: Option<String>) -> Option<String> {
        value
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
    }

    pub fn record_feedback(
        request: RecordStudioFeedbackRequest,
    ) -> Result<StudioFeedbackEvent, String> {
        let target_id = request.target_id.trim().to_string();
        if target_id.is_empty() {
            return Err("Feedback target is required".to_string());
        }
        let evidence_summary = Self::redact_summary(&request.evidence_summary);
        if request.code == StudioFeedbackCode::Completed && evidence_summary.is_empty() {
            return Err(
                "Completed feedback requires retained evidence, not an agent assertion".to_string(),
            );
        }
        let event = StudioFeedbackEvent {
            id: format!("feedback-{}", Uuid::new_v4()),
            target_kind: request.target_kind,
            target_id,
            code: request.code,
            evidence_type: request.evidence_type,
            evidence_summary,
            project_id: Self::optional(request.project_id),
            work_scope: Self::optional(request.work_scope),
            provider_id: Self::optional(request.provider_id),
            created_at: Self::now(),
        };
        let conn = Self::open()?;
        conn.execute("INSERT INTO studio_feedback_events (id,target_kind,target_id,code,evidence_type,evidence_summary,project_id,work_scope,provider_id,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![event.id, Self::text(&event.target_kind)?, event.target_id, Self::text(&event.code)?, Self::text(&event.evidence_type)?, event.evidence_summary, event.project_id, event.work_scope, event.provider_id, event.created_at]
        ).map_err(|error| format!("Failed to record Studio feedback: {error}"))?;
        Ok(event)
    }

    pub fn record_evaluation(request: RecordEvaluationRequest) -> Result<EvaluationRecord, String> {
        let release_id = request.release_id.trim().to_string();
        let case_id = request.case_id.trim().to_string();
        if release_id.is_empty() || case_id.is_empty() {
            return Err("Evaluation release and case are required".to_string());
        }
        let evidence_summary = Self::redact_summary(&request.evidence_summary);
        if evidence_summary.is_empty() {
            return Err("Evaluation records require retained evidence".to_string());
        }
        let record = EvaluationRecord {
            id: format!("evaluation-{}", Uuid::new_v4()),
            release_id,
            case_id,
            status: request.status,
            evidence_type: request.evidence_type,
            evidence_summary,
            project_id: Self::optional(request.project_id),
            work_scope: Self::optional(request.work_scope),
            provider_id: Self::optional(request.provider_id),
            created_at: Self::now(),
        };
        let conn = Self::open()?;
        conn.execute("INSERT INTO studio_evaluation_records (id,release_id,case_id,status,evidence_type,evidence_summary,project_id,work_scope,provider_id,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![record.id, record.release_id, record.case_id, Self::text(&record.status)?, Self::text(&record.evidence_type)?, record.evidence_summary, record.project_id, record.work_scope, record.provider_id, record.created_at]
        ).map_err(|error| format!("Failed to record evaluation: {error}"))?;
        Ok(record)
    }

    pub fn record_activation_run(
        assignment_id: &str,
        release_id: &str,
        project_id: Option<String>,
        work_scope: &str,
        applied_count: usize,
        skipped_count: usize,
        failed_count: usize,
    ) -> Result<ActivationRun, String> {
        let run = ActivationRun {
            id: format!("activation-{}", Uuid::new_v4()),
            assignment_id: assignment_id.to_string(),
            release_id: release_id.to_string(),
            project_id,
            work_scope: work_scope.to_string(),
            applied_count,
            skipped_count,
            failed_count,
            created_at: Self::now(),
        };
        let conn = Self::open()?;
        conn.execute(
            "INSERT INTO studio_activation_runs (id,assignment_id,release_id,project_id,work_scope,applied_count,skipped_count,failed_count,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![run.id, run.assignment_id, run.release_id, run.project_id, run.work_scope, run.applied_count, run.skipped_count, run.failed_count, run.created_at],
        ).map_err(|error| format!("Failed to record activation run: {error}"))?;
        Ok(run)
    }

    pub fn release_health(release_id: &str) -> Result<ReleaseHealth, String> {
        let conn = Self::open()?;
        let target_kind = Self::text(&StudioFeedbackTargetKind::SkillSetRelease)?;
        let mut statement = conn.prepare("SELECT code, COUNT(*), MAX(created_at) FROM studio_feedback_events WHERE target_kind=?1 AND target_id=?2 GROUP BY code")
            .map_err(|error| format!("Failed to query feedback health: {error}"))?;
        let mut completed = 0_u64;
        let mut partial = 0_u64;
        let mut wrong_scope = 0_u64;
        let mut safety = 0_u64;
        let mut evaluated = 0_u64;
        let mut last_success = None;
        let rows = statement
            .query_map(params![target_kind, release_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })
            .map_err(|error| format!("Failed to read feedback health: {error}"))?;
        for row in rows.flatten() {
            let (code, count, latest) = row;
            match code.as_str() {
                "completed" => {
                    completed += count;
                    evaluated += count;
                    last_success = latest.or(last_success);
                }
                "partial" => {
                    partial += count;
                    evaluated += count;
                }
                "wrong_scope" => {
                    wrong_scope += count;
                    evaluated += count;
                }
                "failed" | "instruction_gap" | "dependency_gap" => evaluated += count,
                "safety_concern" => {
                    safety += count;
                    evaluated += count;
                }
                _ => {}
            }
        }
        let rate = |value: u64| {
            if evaluated == 0 {
                None
            } else {
                Some(value as f64 / evaluated as f64)
            }
        };
        let now = Self::now();
        let freshness_days = last_success.map(|timestamp| (now - timestamp).max(0) / 86_400);
        let status = if evaluated < MIN_EVALUATED_SAMPLE {
            StudioHealthStatus::Unknown
        } else if safety > 0
            || rate(partial).unwrap_or(0.0) > MAX_CORRECTION_RATE
            || rate(wrong_scope).unwrap_or(0.0) > MAX_SCOPE_MISMATCH_RATE
            || rate(completed).unwrap_or(0.0) < MIN_SUCCESS_RATE
        {
            StudioHealthStatus::NeedsReview
        } else {
            StudioHealthStatus::Healthy
        };
        let usage_count = conn
            .query_row(
                "SELECT COUNT(*) FROM studio_activation_runs WHERE release_id=?1",
                params![release_id],
                |row| row.get::<_, u64>(0),
            )
            .map_err(|error| format!("Failed to query activation history: {error}"))?;
        Ok(ReleaseHealth {
            release_id: release_id.to_string(),
            status,
            evaluated_count: evaluated,
            usage_count,
            verified_success_rate: rate(completed),
            correction_rate: rate(partial),
            scope_mismatch_rate: rate(wrong_scope),
            safety_incidents: safety,
            last_success_at: last_success,
            freshness_days,
        })
    }

    pub fn review_queue() -> Result<Vec<ReviewQueueItem>, String> {
        let catalog = SkillSetService::catalog()?;
        let mut queue = Vec::new();
        for release in catalog.releases {
            let health = Self::release_health(&release.id)?;
            if health.safety_incidents > 0 {
                queue.push(ReviewQueueItem {
                    release_id: release.id,
                    reason: ReviewReason::SafetyConcern,
                    detail: format!(
                        "{} safety concern(s) require review",
                        health.safety_incidents
                    ),
                });
            } else if health.status == StudioHealthStatus::NeedsReview {
                queue.push(ReviewQueueItem {
                    release_id: release.id,
                    reason: ReviewReason::ThresholdBreach,
                    detail: "A success, correction, or scope-mismatch threshold was breached"
                        .to_string(),
                });
            } else if health.status == StudioHealthStatus::Unknown {
                queue.push(ReviewQueueItem {
                    release_id: release.id,
                    reason: ReviewReason::InsufficientEvidence,
                    detail: format!(
                        "{} evaluated outcomes; at least {} are required",
                        health.evaluated_count, MIN_EVALUATED_SAMPLE
                    ),
                });
            } else if health.freshness_days.unwrap_or(i64::MAX) > REVIEW_WINDOW_DAYS {
                queue.push(ReviewQueueItem {
                    release_id: release.id,
                    reason: ReviewReason::StaleEvaluation,
                    detail: format!("No verified success in {REVIEW_WINDOW_DAYS} days"),
                });
            }
        }
        Ok(queue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::StudioEvidenceType;
    use crate::test_support::with_temp_home;

    fn request(code: StudioFeedbackCode, evidence: &str) -> RecordStudioFeedbackRequest {
        RecordStudioFeedbackRequest {
            target_kind: StudioFeedbackTargetKind::SkillSetRelease,
            target_id: "release-a".to_string(),
            code,
            evidence_type: StudioEvidenceType::CommandResult,
            evidence_summary: evidence.to_string(),
            project_id: None,
            work_scope: None,
            provider_id: None,
        }
    }

    #[test]
    fn completed_feedback_requires_evidence_and_redacts_secrets() {
        with_temp_home(|_| {
            assert!(StudioFeedbackService::record_feedback(request(
                StudioFeedbackCode::Completed,
                ""
            ))
            .is_err());
            let event = StudioFeedbackService::record_feedback(request(
                StudioFeedbackCode::Completed,
                "token sk-abcdefghijklmnopqrstuvwxyz123456",
            ))
            .unwrap();
            assert!(event.evidence_summary.contains("[REDACTED]"));
        });
    }

    #[test]
    fn health_is_unknown_until_minimum_evidence_then_safety_needs_review() {
        with_temp_home(|_| {
            for _ in 0..5 {
                StudioFeedbackService::record_feedback(request(
                    StudioFeedbackCode::Completed,
                    "cargo test passed",
                ))
                .unwrap();
            }
            assert_eq!(
                StudioFeedbackService::release_health("release-a")
                    .unwrap()
                    .status,
                StudioHealthStatus::Healthy
            );
            StudioFeedbackService::record_feedback(request(
                StudioFeedbackCode::SafetyConcern,
                "human review requested",
            ))
            .unwrap();
            assert_eq!(
                StudioFeedbackService::release_health("release-a")
                    .unwrap()
                    .status,
                StudioHealthStatus::NeedsReview
            );
        });
    }
}
