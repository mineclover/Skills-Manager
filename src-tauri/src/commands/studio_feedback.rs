use crate::models::{
    ActivationRun, EvaluationRecord, RecordEvaluationRequest, RecordStudioFeedbackRequest,
    ReleaseHealth, ReleaseImprovementSuggestion, ReviewQueueItem, StudioFeedbackEvent,
};
use crate::services::StudioFeedbackService;

#[tauri::command]
pub fn record_studio_feedback(
    request: RecordStudioFeedbackRequest,
) -> Result<StudioFeedbackEvent, String> {
    StudioFeedbackService::record_feedback(request)
}

#[tauri::command]
pub fn record_release_evaluation(
    request: RecordEvaluationRequest,
) -> Result<EvaluationRecord, String> {
    StudioFeedbackService::record_evaluation(request)
}

#[tauri::command]
pub fn get_release_health(release_id: String) -> Result<ReleaseHealth, String> {
    StudioFeedbackService::release_health(&release_id)
}

#[tauri::command]
pub fn get_studio_review_queue() -> Result<Vec<ReviewQueueItem>, String> {
    StudioFeedbackService::review_queue()
}

#[tauri::command]
pub fn list_activation_runs(release_id: Option<String>) -> Result<Vec<ActivationRun>, String> {
    StudioFeedbackService::activation_runs(release_id.as_deref())
}

#[tauri::command]
pub fn list_release_evaluations(release_id: String) -> Result<Vec<EvaluationRecord>, String> {
    StudioFeedbackService::evaluation_records(&release_id)
}

#[tauri::command]
pub fn get_release_improvement_suggestions(
    release_id: String,
) -> Result<Vec<ReleaseImprovementSuggestion>, String> {
    StudioFeedbackService::improvement_suggestions(&release_id)
}
