use crate::models::{
    EvaluationRecord, RecordEvaluationRequest, RecordStudioFeedbackRequest, ReleaseHealth,
    ReviewQueueItem, StudioFeedbackEvent,
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
