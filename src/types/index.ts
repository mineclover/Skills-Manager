// TypeScript type definitions matching Rust backend models
// Note: Field names use snake_case to match Rust serde serialization

export type SkillScope = "global" | "project" | "tool";

export interface SkillMarketplaceMeta {
  marketplace_source_id?: string | null;
  marketplace_skill_id?: string | null;
  marketplace_skill_slug?: string | null;
  repo_url?: string | null;
  skill_path?: string | null;
  remote_revision?: string | null;
}

export type SkillContractStatus = "unmanaged" | "incomplete" | "managed";
export type SkillContractSource = "portable_sidecar" | "local_metadata";

export interface SkillContractPurpose {
  summary: string;
  use_when: string[];
  avoid_when: string[];
}

export interface SkillContractRequirements {
  runtimes: string[];
  project_signals: string[];
  verification: string[];
}

export interface SkillContractSuccess {
  expected_outcomes: string[];
  non_goals: string[];
  safety_rules: string[];
}

export interface SkillContractFeedback {
  codes: string[];
  required_for_completed: string[];
}

export interface SkillContractEvaluation {
  cases: string[];
  review_cycle_days?: number | null;
}

export interface SkillContract {
  schema_version?: number | null;
  purpose: SkillContractPurpose;
  requirements: SkillContractRequirements;
  success_contract: SkillContractSuccess;
  feedback: SkillContractFeedback;
  evaluation: SkillContractEvaluation;
}

export interface SkillContractSummary {
  status: SkillContractStatus;
  path?: string | null;
  contract?: SkillContract | null;
  validation_errors: string[];
  source?: SkillContractSource | null;
}

export interface PresetApplyProgress {
  preset_id: string;
  project_id?: string | null;
  tool_id: string;
  total_count: number;
  processed_count: number;
  applied_count: number;
  skipped_count: number;
  failed_count: number;
  current_skill_instance_id?: string | null;
  current_skill_name?: string | null;
  completed: boolean;
}

export interface SkillSystemPrompt {
  project_id?: string | null;
  tool_id: string;
  included_skill_instance_ids: string[];
  skipped_skill_instance_ids: string[];
  content: string;
}

export interface SkillSetMember {
  skill_id: string;
  scope_policy: "global" | "project" | "project_then_global" | "tool_local";
}

export interface SkillSetMemberSnapshot {
  skill_id: string;
  source_path: string;
  scope: SkillScope;
  contract_status: SkillContractStatus;
  contract_digest?: string | null;
  contract?: SkillContract | null;
  purpose_summary?: string | null;
  evaluation_cases: string[];
}

export interface SkillSetBlueprint {
  id: string;
  name: string;
  description: string;
  members: SkillSetMember[];
  created_at: number;
  updated_at: number;
  reviewed_at?: number | null;
}

export interface SkillSetRelease {
  id: string;
  blueprint_id: string;
  blueprint_name: string;
  label: string;
  release_notes: string;
  content_digest: string;
  members: SkillSetMember[];
  member_snapshots: SkillSetMemberSnapshot[];
  created_at: number;
}

export interface ReleaseEvaluationSummary {
  release_id: string;
  total_count: number;
  passed_count: number;
  failed_count: number;
  blocked_count: number;
  required_case_count: number;
  verified_case_count: number;
  is_verified: boolean;
  last_evaluated_at?: number | null;
}

export interface SkillSetAssignment {
  id: string;
  release_id: string;
  project_id?: string | null;
  work_scope: string;
  /** All tags must match; omitted on legacy records. */
  work_scope_tags?: string[];
  role: "default" | "recommended" | "work_scope_overlay";
  provider_ids: string[];
  priority: number;
  active: boolean;
  created_at: number;
  updated_at: number;
}

export interface SkillSetStore {
  schema_version: number;
  blueprints: SkillSetBlueprint[];
  releases: SkillSetRelease[];
  assignments: SkillSetAssignment[];
}

export type ActivationPlanAction = "enable" | "unchanged";

export interface SkillSetActivationOperation {
  skill_id: string;
  skill_instance_id: string;
  tool_id: string;
  current_enabled: boolean;
  action: ActivationPlanAction;
  reason: string;
}

export interface SkillSetActivationPlan {
  assignment_id: string;
  release_id: string;
  project_id?: string | null;
  work_scope: string;
  /** All tags must match; omitted on legacy records. */
  work_scope_tags?: string[];
  operations: SkillSetActivationOperation[];
  missing_skill_ids: string[];
  requires_shared_root_confirmation: boolean;
  shared_impacts: SkillBindingImpact[];
  generated_at: number;
}

export interface SkillSetActivationApplyResult {
  plan: SkillSetActivationPlan;
  activation_run_id: string;
  applied_count: number;
  skipped_count: number;
  failed_count: number;
  failures: string[];
  provider_outcomes: ActivationProviderOutcome[];
}

export interface ActivationProviderOutcome {
  provider_id: string;
  applied_count: number;
  skipped_count: number;
  failed_count: number;
}

export interface SkillSetDriftReport {
  assignment_id: string;
  release_id: string;
  project_id?: string | null;
  work_scope: string;
  /** All tags must match; omitted on legacy records. */
  work_scope_tags?: string[];
  disabled_operations: SkillSetActivationOperation[];
  missing_skill_ids: string[];
  compliant: boolean;
  generated_at: number;
}

export interface ActivationRun {
  id: string;
  assignment_id: string;
  release_id: string;
  project_id?: string | null;
  work_scope: string;
  applied_count: number;
  skipped_count: number;
  failed_count: number;
  provider_outcomes: ActivationProviderOutcome[];
  created_at: number;
}

export interface EffectiveSkillSetMember {
  skill_id: string;
  scope_policy: "global" | "project" | "project_then_global" | "tool_local";
  skill_instance_id?: string | null;
  included_by_release_ids: string[];
}

export interface EffectiveSkillSet {
  project_id?: string | null;
  work_scope: string;
  /** All tags must match; omitted on legacy records. */
  work_scope_tags?: string[];
  assignment_ids: string[];
  release_ids: string[];
  members: EffectiveSkillSetMember[];
  unresolved_skill_ids: string[];
  generated_at: number;
}

export type StudioFeedbackTargetKind = "skill" | "skill_set_release" | "activation_run";
export type StudioFeedbackCode = "completed" | "partial" | "failed" | "wrong_scope" | "instruction_gap" | "dependency_gap" | "safety_concern";
export type StudioEvidenceType = "command_result" | "evaluation_assertion" | "human_confirmation";
export type StudioHealthStatus = "unknown" | "healthy" | "needs_review";
export type ReviewReason =
  | "insufficient_evidence"
  | "threshold_breach"
  | "safety_concern"
  | "stale_evaluation"
  | "unresolved_required_entry"
  | "repeated_feedback_gap"
  | "provider_drift"
  | "contract_incomplete";

export interface ReleaseHealth {
  release_id: string;
  status: StudioHealthStatus;
  evaluated_count: number;
  usage_count: number;
  verified_success_rate?: number | null;
  correction_rate?: number | null;
  scope_mismatch_rate?: number | null;
  safety_incidents: number;
  last_success_at?: number | null;
  freshness_days?: number | null;
}

export interface ReleaseHealthContextRequest {
  release_id: string;
  project_id?: string | null;
  work_scope?: string | null;
  provider_id?: string | null;
}

export interface ReviewQueueItem { release_id: string; reason: ReviewReason; detail: string; }

export interface ReleaseImprovementSuggestion {
  release_id: string;
  code: StudioFeedbackCode;
  occurrence_count: number;
  title: string;
  rationale: string;
  suggested_action: string;
}

export type EvaluationStatus = "passed" | "failed" | "blocked";

export interface EvaluationRecord {
  id: string;
  release_id: string;
  case_id: string;
  status: EvaluationStatus;
  evidence_type: StudioEvidenceType;
  evidence_summary: string;
  project_id?: string | null;
  work_scope?: string | null;
  provider_id?: string | null;
  created_at: number;
}

export interface Skill {
  id: string;
  instance_id: string;
  scope: SkillScope;
  project_id?: string | null;
  project_name?: string | null;
  tool_id?: string | null;
  name: string;
  description: string | null;
  version: string;
  source: "local" | "imported" | "marketplace" | "vault";
  contract: SkillContractSummary;
  enabled: Record<string, boolean>;
  package_meta?: SkillPackageMeta | null;
  marketplace_meta?: SkillMarketplaceMeta | null;
  path: string;
}

export interface ProjectBinding {
  id: string;
  name: string;
  root_path?: string | null;
  skills_dir: string;
  root_path?: string | null;
}

export interface SkillPackageMeta {
  package_id: string;
  package_name?: string | null;
  package_member_id: string;
  package_version?: string | null;
}

export interface InstalledSkillPackage {
  package_id: string;
  name: string;
  version: string;
  installed_members: string[];
  selected_members: string[];
  path?: string | null;
  manifest_hash?: string | null;
  installed_at: number;
  updated_at: number;
}

export interface SkillMetadata {
  tags: string[];
  note?: string | null;
  favorited_at?: number | null;
  /** 最近一次成功发布到 ClawHub 的记录；未发布过时缺省。 */
  publish?: SkillPublishRecord | null;
  local_contract?: SkillContract | null;
}

export type SkillMetadataMap = Record<string, SkillMetadata>;

export interface MarketplaceFavoriteMeta {
  favorited_at: number;
  name: string;
  description?: string | null;
  source_id: string;
  source_name: string;
  repo_url?: string | null;
  skill_path?: string | null;
  external_url?: string | null;
  install_count?: number | null;
  tags: string[];
  clawhub_slug?: string | null;
  clawhub_owner?: string | null;
  clawhub_version?: string | null;
}

export type MarketplaceFavoriteMap = Record<string, MarketplaceFavoriteMeta>;

export interface ToolConfig {
  enabled: boolean;
  detected: boolean;
  skills_path: string;
  config_path: string;
}

export interface Tool {
  id: string;
  name: string;
  detected: boolean;
  cli_available: boolean;
  config: ToolConfig;
  source: "builtin" | "custom";
  icon_path?: string | null;
  project_skills_dir?: string | null;
}

// Risk scan
export type RiskScanMode = "off" | "basic" | "deep";
export type RiskLevel = "safe" | "low" | "medium" | "high" | "critical";
export type RiskCategory = "destructive" | "network" | "privilege" | "payload";

export interface RiskLocation {
  file: string;
  line: number;
}

export interface RiskFinding {
  rule_id: string;
  category: RiskCategory;
  level: RiskLevel;
  confidence: number;
  message: string;
  evidence: string;
  location: RiskLocation;
  source: "rule" | "llm";
}

export interface SkillRiskReport {
  instance_id: string;
  level: RiskLevel;
  findings: RiskFinding[];
  scanned_at: number;
  scanner_version: string;
  mode: RiskScanMode;
  llm_reviewed: boolean;
}

// User preferences for the application
export interface UserPreferences {
  // Appearance
  theme: "light" | "dark" | "system";
  font_family: "default" | "serif";
  language: "zh" | "en" | "ko";

  // Sync behavior
  auto_sync: boolean;

  // Editor settings
  default_editor: string;
  tab_size: 2 | 4;

  // Notifications
  show_sync_notifications: boolean;
  remove_links_when_disabling_tool: boolean;
  skill_usage_monitor: boolean;
  risk_scan_mode: RiskScanMode;

  // Marketplace auth
  github_token?: string | null;
  clawhub_token?: string | null;
}

export interface SkillUsageStats {
  total: number;
  by_tool: Record<string, number>;
  last_called_at: number | null;
}

export interface AuthProfile {
  username: string;
  avatar_url?: string | null;
}

export interface AuthSession {
  provider: string;
  access_token?: string | null;
  refresh_token?: string | null;
  profile: AuthProfile;
}

export interface AuthStartResult {
  auth_url: string;
  state: string;
}

export interface AuthMeResponse {
  user_id: string;
  provider?: string | null;
  username?: string | null;
  avatar_url?: string | null;
  email?: string | null;
}

export interface AppConfig {
  version: string;
  skills_dir: string;
  tools: Record<string, ToolConfig>;
  custom_tools?: Record<string, CustomToolConfig>;
  skill_metadata?: SkillMetadataMap;
  marketplace_favorites?: MarketplaceFavoriteMap;
  preferences?: UserPreferences;
  marketplace_sources?: MarketplaceSource[];
  auth_session?: AuthSession | null;
  projects?: ProjectBinding[];
  active_project_id?: string | null;
  llm_provider?: LlmProvider | null;
  presets?: SkillActivationPreset[];
  active_preset_id?: string | null;
}

export interface LlmProvider {
  base_url: string;
  api_key: string;
  model: string;
  temperature?: number | null;
  max_tokens?: number | null;
  timeout_secs?: number | null;
}

export interface CustomToolConfig {
  name: string;
  config_path: string;
  skills_path: string;
  enabled: boolean;
  icon_path?: string | null;
}

export interface SyncReport {
  issues_count: number;
}

export interface LinkResult {
  skill_id: string;
  tool_id: string;
  message: string | null;
}

export interface LinkReport {
  success: LinkResult[];
  failed: LinkResult[];
}

export type BatchSkillToolTargetKind = "skill" | "group";
export type BatchSkillToolAction = "enable" | "disable";

export interface BatchSkillToolTarget {
  kind: BatchSkillToolTargetKind;
  id: string;
}

export interface BatchSetSkillToolsRequest {
  targets: BatchSkillToolTarget[];
  tool_ids: string[];
  action: BatchSkillToolAction;
}

export interface BatchSetSkillToolsFailure {
  target_kind: BatchSkillToolTargetKind;
  target_id: string;
  skill_id?: string | null;
  tool_id?: string | null;
  message: string;
}

export interface BatchSetSkillToolsResponse {
  requested_target_count: number;
  requested_tool_count: number;
  resolved_skill_count: number;
  attempted_operation_count: number;
  applied_count: number;
  skipped_count: number;
  failed_count: number;
  failures: BatchSetSkillToolsFailure[];
  report: SkillOperationReport;
}

// Detected editor from backend
export interface DetectedEditor {
  id: string;
  name: string;
  command: string;
  available: boolean;
  icon: string;
  icon_data?: string;  // Base64 encoded PNG from app bundle
}

// File tree node
export interface FileNode {
  name: string;
  path: string;
  is_dir: boolean;
  children?: FileNode[];
}

export interface MarketplaceSource {
  id: string;
  name: string;
  url: string;
  source_type: "github_repo" | "api" | "crawler" | "manual" | "unknown" | "clawhub_api";
  enabled: boolean;
  builtin: boolean;
  api_key?: string | null;
}

export type MarketplaceInstallStatus = "not_installed" | "installed" | "update_available";

export interface MarketplaceInstallTarget {
  scope: SkillScope;
  project_id?: string | null;
  tool_ids?: string[];
}

export interface MarketplaceInstallSelection {
  global: boolean;
  projects: MarketplaceInstallTarget[];
}

export interface MarketplaceInstallation {
  instance_id: string;
  scope: SkillScope;
  project_id?: string | null;
  project_name?: string | null;
  tool_ids: string[];
  install_status: MarketplaceInstallStatus;
}

export interface MarketplaceSkill {
  id: string;
  slug?: string | null;
  name: string;
  description: string | null;
  author: string | null;
  source_id: string;
  source_name: string;
  install_count?: number | null;
  install_url?: string | null;
  created_at?: number | null;
  repo_url: string | null;
  skill_path: string | null;
  external_url: string | null;
  remote_revision?: string | null;
  tags: string[];
  install_status: MarketplaceInstallStatus;
  installations: MarketplaceInstallation[];
  clawhub_slug?: string | null;
  clawhub_owner?: string | null;
  clawhub_version?: string | null;
}

export interface MarketplaceSkillsResponse {
  skills: MarketplaceSkill[];
  has_more: boolean;
}

export interface SkillFileNode {
  name: string;
  path: string;
  is_dir: boolean;
  download_url: string | null;
  sha?: string | null;
  children?: SkillFileNode[];
}

/** fetch_clawhub_skill_files 返回结构：文件树 + 解析出的 owner/version */
export interface ClawhubSkillFilesResponse {
  tree: SkillFileNode;
  resolved_owner?: string | null;
  resolved_version?: string | null;
}

export interface InstallResult {
  success: boolean;
  skill_id: string;
  message: string | null;
  installed_path: string | null;
}

export interface MarketplaceSyncResult {
  checked: number;
  updated: number;
  failed: string[];
}

export interface MarketplaceUpdateCheckResult {
  performed: boolean;
  checked: number;
  update_available: number;
}

export interface UpdateInfo {
  has_update: boolean;
  latest_version: string;
  download_url: string;
  release_notes?: string;
}

export interface CliInstallStatus {
  bundled: boolean;
  installed: boolean;
  target: string;
  versionMatches: boolean;
  appVersion: string;
  /** False when the install folder is not on PATH — `skm` would not resolve in a shell. */
  onPath: boolean;
}

export interface CliSkillEnableFailure {
  tool: string;
  message: string;
}

export interface CliSkillInstallReport {
  id?: string;
  path?: string;
  enabled_for?: string[];
  failed?: CliSkillEnableFailure[];
  error?: string;
}

export interface CliInstallResult {
  installed: boolean;
  target: string;
  onPath: boolean;
  cliSkill?: CliSkillInstallReport;
}

export type FeedbackContactType =
  | "wechat"
  | "email"
  | "other";

export interface FeedbackRequest {
  contact_type: FeedbackContactType;
  contact_value: string;
  content: string;
  source?: string | null;
  language?: string | null;
}

// Skill import/export (cross-device sync)
export interface ExportedSkillMeta {
  id: string;
  name: string;
  description: string | null;
  version: string;
  folder: string;
  enabled_tools: string[];
  tags: string[];
  note?: string | null;
  favorited_at: number | null;
}

export interface ExportManifest {
  format_version: number;
  exported_at: number;
  app_version: string;
  skills: ExportedSkillMeta[];
}

export interface ImportConflict {
  skill_id: string;
  skill_name: string;
  local_path: string;
}

export interface ImportPreview {
  manifest: ExportManifest;
  conflicts: ImportConflict[];
}

export type ConflictStrategy = "skip" | "overwrite" | "rename";

export interface ImportResolution {
  skill_id: string;
  strategy: ConflictStrategy;
}

export interface ImportedSkillRecord {
  original_id: string;
  final_id: string;
  name: string;
}

export interface RenamedSkillRecord {
  original_id: string;
  new_id: string;
  name: string;
}

export interface ImportFailure {
  skill_id: string;
  message: string;
}

export interface ImportResult {
  imported: ImportedSkillRecord[];
  skipped: string[];
  overwritten: string[];
  renamed: RenamedSkillRecord[];
  failed: ImportFailure[];
}

// ===== ClawHub 发布 =====

export interface ClawhubIdentity {
  handle: string | null;
  display_name: string | null;
  image: string | null;
}

export interface PublishFileEntry {
  rel_path: string;
  size: number;
}

export interface PublishPreview {
  files: PublishFileEntry[];
  total_bytes: number;
  suggested_slug: string;
  suggested_display_name: string;
  suggested_owner_handle: string | null;
  latest_version: string | null;
  suggested_version: string;
  existing_record: SkillPublishRecord | null;
  version_lookup_failed: boolean;
  warning: string | null;
}

export interface SkillPublishRecord {
  slug: string;
  owner_handle?: string | null;
  version: string;
  published_at: number;
  publication_status?: string | null;
  external_url?: string | null;
}

export interface PublishRequest {
  instance_id: string;
  slug: string;
  display_name: string;
  version: string;
  changelog: string;
  categories: string[];
  topics: string[];
  owner_handle?: string | null;
  accept_license_terms: boolean;
}

export interface PublishResult {
  ok: boolean;
  version_id?: string | null;
  publication_status?: string | null;
  external_url?: string | null;
  version: string;
}
