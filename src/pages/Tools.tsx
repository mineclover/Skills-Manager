import { useState, useEffect, useCallback, useMemo } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { confirm, open } from "@tauri-apps/plugin-dialog";

import { Skill, SkillOperationPreview, SkillOperationReport, Tool } from "@/types";
import { useTranslation } from "@/i18n";
import { usePageSearch } from "@/components/PageHeaderContext";
import { getToolIconUrl, GenericToolIcon } from "@/assets/tools";
import { FolderOpen, Pencil, Plus, Power, Trash2, X } from "lucide-react";
import { Toggle } from "@/components/ui/toggle";
import { RelationToggleDialog } from "@/components/skills/RelationToggleDialog";
import { RefreshButton } from "@/components/ui/refresh-button";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { PageHeader } from "@/components/ui/page-header";
import { ToastContainer, useToast } from "@/components/ui/toast";
import { Input } from "@/components/ui/input";
import { OperationReportCard } from "@/components/skills/OperationReportCard";

import { PageLoader } from "@/components/ui/loading";
import { sortToolsByEnabled } from "./tools/sortTools";
import {
  getBulkToggleConfirmKey,
  getBulkToggleTargets,
  getNextBulkToggleMode,
} from "./tools/bulkToggleTools";
import { orderSkillIdsForTool } from "./tools/orderSkillIdsForTool";
import {
  getToolBulkToggleConfirmKey,
  getToolBulkToggleMode,
  getToolBulkToggleTargets,
} from "./tools/bulkToggleToolSkills";
import { mergeSkillOperationReports } from "./tools/mergeSkillOperationReports";

function getSkillDisplayName(skillIdentity: string, skills: Skill[]): string {
  const skill = skills.find((item) => item.instance_id === skillIdentity) ?? skills.find((item) => item.id === skillIdentity);
  return skill?.name ?? skillIdentity;
}

// Module-level cache: survives route changes so revisiting the Tools page
// renders the last-known data immediately (no PageLoader flash) while a
// silent background refresh runs. Same pattern Marketplace uses.
interface ToolsPageCache {
  tools: Tool[];
  skills: Skill[];
}
let toolsPageCache: ToolsPageCache | null = null;

export function Tools() {
  const { t } = useTranslation();
  const { query: searchQuery } = usePageSearch(t("tools.searchPlaceholder"));
  const [tools, setTools] = useState<Tool[]>(() => toolsPageCache?.tools ?? []);
  const [skills, setSkills] = useState<Skill[]>(() => toolsPageCache?.skills ?? []);
  const [error, setError] = useState<string | null>(null);
  const [initialLoading, setInitialLoading] = useState(() => toolsPageCache === null);
  const [refreshing, setRefreshing] = useState(false);
  const [bulkToggling, setBulkToggling] = useState(false);
  const { toasts, addToast, removeToast } = useToast();
  const [formOpen, setFormOpen] = useState(false);
  const [editingToolId, setEditingToolId] = useState<string | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const [idManuallyEdited, setIdManuallyEdited] = useState(false);
  const [toolEditorToolId, setToolEditorToolId] = useState<string | null>(null);
  const [toolEditorQuery, setToolEditorQuery] = useState("");
  const [toolEditorEnabledOnly, setToolEditorEnabledOnly] = useState(false);
  const [togglingSkill, setTogglingSkill] = useState<string | null>(null);
  const [bulkTogglingToolId, setBulkTogglingToolId] = useState<string | null>(null);
  const [lastOperationReport, setLastOperationReport] = useState<SkillOperationReport | null>(null);
  const [iconFallbackStage, setIconFallbackStage] = useState<Record<string, "asset" | "file" | "none">>({});
  const [form, setForm] = useState({
    name: "",
    id: "",
    configPath: "",
    skillsPath: "",
    iconPath: "",
  });
  const formInputStyle: React.CSSProperties = {
    height: '40px',
    backgroundColor: 'var(--background)',
    borderRadius: '8px',
    border: '1px solid var(--border)',
    padding: '0 12px',
    color: 'var(--foreground)',
    caretColor: 'var(--foreground)',
    flex: 1,
    fontSize: '13px',
  };
  const fieldLabelStyle: React.CSSProperties = {
    display: 'block',
    fontSize: '12px',
    fontWeight: 500,
    color: 'var(--muted-foreground)',
    marginBottom: '6px',
    letterSpacing: '0.01em',
  };
  const pickerButtonStyle: React.CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: '40px',
    height: '40px',
    borderRadius: '8px',
    border: '1px solid var(--border)',
    background: 'var(--secondary)',
    color: 'var(--muted-foreground)',
    cursor: 'pointer',
    flexShrink: 0,
    transition: 'background-color 0.15s, color 0.15s, border-color 0.15s',
  };

  const fetchToolsAndSkills = useCallback(async (
    toolsCommand: "detect_tools" | "refresh_tools",
  ) => {
    setError(null);
    const [toolsResult, skillsResult] = await Promise.all([
      invoke<Tool[]>(toolsCommand),
      // Tools manages the global/direct agent view. Project-scoped reads are
      // explicit on Skills and Presets, so this page must not follow the
      // persisted active project implicitly.
      invoke<Skill[]>("scan_skills_for_scope", { projectId: null }),
    ]);
    setTools(toolsResult);
    setSkills(skillsResult);
    setIconFallbackStage({});
  }, []);

  // Initial load - uses cached data
  const loadTools = useCallback(async () => {
    try {
      // Directly installed skills may not exist in the manager cache yet.
      await fetchToolsAndSkills("detect_tools");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setInitialLoading(false);
    }
  }, [fetchToolsAndSkills]);

  // Manual refresh - forces re-detection
  const handleRefresh = useCallback(async () => {
    setRefreshing(true);
    try {
      await fetchToolsAndSkills("refresh_tools");
      addToast(t("common.refreshSuccess"), "success");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setRefreshing(false);
    }
  }, [addToast, fetchToolsAndSkills, t]);

  // Reload after operations - force re-detection to avoid stale cached list
  const reloadTools = useCallback(async () => {
    try {
      await fetchToolsAndSkills("refresh_tools");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [fetchToolsAndSkills]);
  const toggleToolEnabled = useCallback(async (tool: Tool, enabled: boolean) => {
    // Undetected tools cannot be enabled, but still allow disabling if already enabled.
    if (enabled && !tool.detected) {
      setError(t("skills.toolNotDetected"));
      return;
    }

    const previousEnabled = tool.config.enabled;

    // Optimistic update
    setTools(prev => prev.map(t =>
      t.id === tool.id ? { ...t, config: { ...t.config, enabled } } : t
    ));
    setError(null);

    try {
      await invoke("set_tool_enabled", { toolId: tool.id, enabled });
    } catch (err) {
      // Rollback on error
      setTools(prev => prev.map(t =>
        t.id === tool.id ? { ...t, config: { ...t.config, enabled: previousEnabled } } : t
      ));
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [t]);

  const openToolSkillEditor = useCallback((toolId: string) => {
    setToolEditorToolId(toolId);
    setToolEditorQuery("");
    setToolEditorEnabledOnly(false);
  }, []);

  const closeToolSkillEditor = useCallback(() => {
    setToolEditorToolId(null);
    setToolEditorQuery("");
    setToolEditorEnabledOnly(false);
  }, []);

  const confirmSharedPreviews = useCallback(async (previews: SkillOperationPreview[]) => {
    const sharedPreviews = previews.filter((preview) => preview.requires_confirmation);
    if (sharedPreviews.length === 0) {
      return true;
    }

    const impactNames = Array.from(
      new Set(sharedPreviews.flatMap((preview) => preview.impacts.map((impact) => impact.display_name))),
    ).join(", ");
    const warning = sharedPreviews.find((preview) => preview.warning)?.warning;
    return confirm(
      `${warning ?? t("skills.sharedImpactConfirm")}${impactNames ? `\n\n${impactNames}` : ""}`,
      { title: t("skills.sharedImpactConfirmTitle"), kind: "warning" },
    );
  }, [t]);

  const handleToggleSkillForTool = useCallback(async (tool: Tool, instanceId: string, enabled: boolean) => {
    const toggleKey = `${tool.id}:${instanceId}`;
    setTogglingSkill(toggleKey);

    try {
      const skill = skills.find((item) => item.instance_id === instanceId);
      const preview = await invoke<SkillOperationPreview>("preview_skill_operation", {
        projectId: skill?.project_id ?? null,
        skillInstanceId: instanceId,
        providerId: tool.id,
        enabled,
      });
      if (!(await confirmSharedPreviews([preview]))) {
        return;
      }

      const command = enabled ? "enable_skill" : "disable_skill";
      const report = await invoke<SkillOperationReport>(command, { instanceId, toolId: tool.id });
      setLastOperationReport(report);
      if (report.failed_count > 0) {
        throw new Error(report.failures[0]?.message || t("skills.toggleFailed"));
      }
      addToast(
        (enabled ? t("skills.enableSuccess") : t("skills.disableSuccess"))
          .replace("{skill}", getSkillDisplayName(instanceId, skills))
          .replace("{tool}", tool.name),
        "success",
      );
      await reloadTools();
    } catch (err) {
      addToast(err instanceof Error ? err.message : String(err), "error");
    } finally {
      setTogglingSkill(null);
    }
  }, [addToast, confirmSharedPreviews, reloadTools, skills, t]);

  const handleBulkToggleToolSkills = useCallback(async (tool: Tool, visibleSkillIds: string[]) => {
    const enabledMap: Record<string, boolean> = {};
    skills.forEach((skill) => {
      enabledMap[skill.instance_id] = Boolean(skill.enabled[tool.id]);
    });

    const bulkMode = getToolBulkToggleMode(visibleSkillIds, enabledMap);
    const targetSkillIds = getToolBulkToggleTargets(visibleSkillIds, enabledMap, bulkMode);

    if (targetSkillIds.length === 0) {
      return;
    }

    const enabled = bulkMode === "enable";
    const confirmed = await confirm(
      t(getToolBulkToggleConfirmKey(bulkMode)).replace("{count}", String(targetSkillIds.length)),
      {
        title: t("tools.bulkConfirmTitle"),
        kind: "warning",
      },
    );
    if (!confirmed) {
      return;
    }

    let previews: SkillOperationPreview[];
    try {
      previews = await Promise.all(
        targetSkillIds.map((instanceId) => {
          const skill = skills.find((item) => item.instance_id === instanceId);
          return invoke<SkillOperationPreview>("preview_skill_operation", {
            projectId: skill?.project_id ?? null,
            skillInstanceId: instanceId,
            providerId: tool.id,
            enabled,
          });
        }),
      );
      if (!(await confirmSharedPreviews(previews))) {
        return;
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      addToast(message, "error");
      setError(message);
      return;
    }

    setBulkTogglingToolId(tool.id);
    setError(null);

    // Optimistic update for immediate feedback.
    setSkills((prev) =>
      prev.map((skill) => {
        if (!targetSkillIds.includes(skill.instance_id)) {
          return skill;
        }
        return {
          ...skill,
          enabled: {
            ...skill.enabled,
            [tool.id]: enabled,
          },
        };
      }),
    );

    try {
      const command = enabled ? "enable_skill" : "disable_skill";
      const results = await Promise.allSettled(
        targetSkillIds.map((instanceId) => invoke<SkillOperationReport>(command, { instanceId, toolId: tool.id })),
      );
      const reports = results
        .filter((result): result is PromiseFulfilledResult<SkillOperationReport> => result.status === "fulfilled")
        .map((result) => result.value);
      const mergedReport = mergeSkillOperationReports(reports);
      if (mergedReport) {
        setLastOperationReport(mergedReport);
      }

      const failedCount = results.filter((result) => result.status === "rejected").length
        + (mergedReport?.failed_count ?? 0);
      const changedCount = mergedReport?.applied_count ?? 0;

      if (changedCount > 0) {
        const successMessage = enabled
          ? t("tools.bulkEnableSkillsSuccess")
          : t("tools.bulkDisableSkillsSuccess");
        addToast(successMessage.replace("{count}", String(changedCount)), "success");
      }

      if (failedCount > 0) {
        const failedMessage = t("tools.bulkToggleSkillsPartialFailed").replace("{count}", String(failedCount));
        addToast(failedMessage, "error");
        setError(failedMessage);
      }

      await reloadTools();
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      addToast(message, "error");
      setError(message);
      await reloadTools();
    } finally {
      setBulkTogglingToolId(null);
    }
  }, [addToast, confirmSharedPreviews, reloadTools, skills, t]);

  const bulkToggleMode = useMemo(
    () => getNextBulkToggleMode(tools),
    [tools]
  );
  const bulkToggleTargets = useMemo(
    () => getBulkToggleTargets(tools, bulkToggleMode),
    [tools, bulkToggleMode]
  );

  const handleBulkToggleTools = useCallback(async () => {
    if (bulkToggleTargets.length === 0) {
      return;
    }

    const enabled = bulkToggleMode === "enable";
    const confirmed = await confirm(
      t(getBulkToggleConfirmKey(bulkToggleMode)).replace("{count}", String(bulkToggleTargets.length)),
      {
        title: t("tools.bulkConfirmTitle"),
        kind: "warning",
      }
    );
    if (!confirmed) {
      return;
    }

    const targetIds = new Set(bulkToggleTargets.map((tool) => tool.id));

    setBulkToggling(true);
    setError(null);

    try {
      // Optimistic update for immediate feedback.
      setTools((prev) =>
        prev.map((tool) =>
          targetIds.has(tool.id)
            ? { ...tool, config: { ...tool.config, enabled } }
            : tool
        )
      );

      const results = await Promise.allSettled(
        bulkToggleTargets.map((tool) =>
          invoke("set_tool_enabled", { toolId: tool.id, enabled })
        )
      );

      const failedCount = results.filter((result) => result.status === "rejected").length;
      const changedCount = bulkToggleTargets.length - failedCount;

      if (changedCount > 0) {
        const successMessage = enabled
          ? t("tools.bulkEnableSuccess")
          : t("tools.bulkDisableSuccess");
        addToast(successMessage.replace("{count}", String(changedCount)), "success");
      }

      if (failedCount > 0) {
        const failedMessage = t("tools.bulkTogglePartialFailed").replace("{count}", String(failedCount));
        addToast(failedMessage, "error");
        setError(failedMessage);
      }

      await reloadTools();
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      addToast(message, "error");
      setError(message);
      await reloadTools();
    } finally {
      setBulkToggling(false);
    }
  }, [addToast, bulkToggleMode, bulkToggleTargets, reloadTools, t]);

  // 修改配置路径（默认定位到当前路径）
  const handleEditConfigPath = useCallback(async (toolId: string, currentPath?: string) => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: t("tools.selectConfigPath"),
      defaultPath: currentPath || undefined,
    });

    if (selected && typeof selected === "string") {
      try {
        await invoke("update_tool_paths", {
          toolId,
          configPath: selected,
        });
        await reloadTools();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    }
  }, [reloadTools, t]);

  // 修改技能路径（默认定位到当前路径）
  const handleEditSkillsPath = useCallback(async (toolId: string, currentPath?: string) => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: t("tools.selectSkillsPath"),
      defaultPath: currentPath || undefined,
    });

    if (selected && typeof selected === "string") {
      try {
        await invoke("update_tool_paths", {
          toolId,
          skillsPath: selected,
        });
        await reloadTools();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    }
  }, [reloadTools, t]);


  const slugify = (value: string) =>
    value
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "");

  const startCreateCustomTool = useCallback(() => {
    setEditingToolId(null);
    setForm({
      name: "",
      id: "",
      configPath: "",
      skillsPath: "",
      iconPath: "",
    });
    setIdManuallyEdited(false);
    setFormError(null);
    setFormOpen(true);
  }, []);

  const startEditCustomTool = useCallback((tool: Tool) => {
    setEditingToolId(tool.id);
    setForm({
      name: tool.name,
      id: tool.id,
      configPath: tool.config.config_path,
      skillsPath: tool.config.skills_path,
      iconPath: tool.icon_path || "",
    });
    setIdManuallyEdited(true);
    setFormError(null);
    setFormOpen(true);
  }, []);

  const handleCustomNameChange = useCallback((value: string) => {
    setForm(prev => ({
      ...prev,
      name: value,
      id: idManuallyEdited ? prev.id : slugify(value),
    }));
  }, [idManuallyEdited]);

  const handleCustomIdChange = useCallback((value: string) => {
    setIdManuallyEdited(true);
    setForm(prev => ({ ...prev, id: value }));
  }, []);

  const handleSelectCustomConfigPath = useCallback(async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: t("tools.selectConfigPath"),
    });
    if (selected && typeof selected === "string") {
      setForm(prev => ({
        ...prev,
        configPath: selected,
        skillsPath: `${selected}/skills`,
      }));
    }
  }, [t]);

  const handleSelectCustomSkillsPath = useCallback(async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: t("tools.selectSkillsPath"),
    });
    if (selected && typeof selected === "string") {
      setForm(prev => ({ ...prev, skillsPath: selected }));
    }
  }, [t]);

  const handleSelectCustomIconPath = useCallback(async () => {
    const selected = await open({
      directory: false,
      multiple: false,
      title: t("tools.selectIconPath"),
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "svg", "ico"] }],
    });
    if (selected && typeof selected === "string") {
      setForm(prev => ({ ...prev, iconPath: selected }));
    }
  }, [t]);

  const handleSaveCustomTool = useCallback(async () => {
    const trimmedName = form.name.trim();
    const trimmedId = form.id.trim();
    const trimmedConfig = form.configPath.trim();
    const trimmedSkills = form.skillsPath.trim();

    if (!trimmedName || !trimmedId || !trimmedConfig || !trimmedSkills) {
      setFormError(t("tools.customErrorRequired"));
      return;
    }

    if (!editingToolId) {
      const existingIds = new Set(tools.map(tool => tool.id));
      if (existingIds.has(trimmedId)) {
        setFormError(t("tools.customErrorConflict"));
        return;
      }
    }

    setFormError(null);
    setError(null);

    try {
      if (editingToolId) {
        const currentTool = tools.find(tool => tool.id === editingToolId);
        await invoke("update_custom_tool", {
          toolId: editingToolId,
          name: trimmedName,
          configPath: trimmedConfig,
          skillsPath: trimmedSkills,
          iconPath: form.iconPath.trim() ? form.iconPath.trim() : null,
          enabled: currentTool?.config.enabled ?? false,
        });
      } else {
        await invoke("create_custom_tool", {
          toolId: trimmedId,
          name: trimmedName,
          configPath: trimmedConfig,
          skillsPath: trimmedSkills,
          iconPath: form.iconPath.trim() ? form.iconPath.trim() : null,
        });
      }

      await reloadTools();
      setFormOpen(false);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    }
  }, [editingToolId, form, tools, reloadTools, t]);

  const handleDeleteCustomTool = useCallback(async (tool: Tool) => {
    const confirmed = await confirm(
      t("tools.customDeleteConfirm").replace("{name}", tool.name),
      { title: t("tools.customDelete") }
    );
    if (!confirmed) {
      return;
    }

    setError(null);
    try {
      await invoke("delete_custom_tool", { toolId: tool.id });
      await reloadTools();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [reloadTools, t]);

  // Render cached data immediately, then refresh it in the background so
  // skills installed outside the manager are not hidden until a manual click.
  useEffect(() => {
    loadTools();
  }, [loadTools]);

  // Keep the module-level cache in sync with the latest loaded data so the
  // next mount can render immediately without a PageLoader flash.
  useEffect(() => {
    if (initialLoading) return;
    toolsPageCache = { tools, skills };
  }, [initialLoading, tools, skills]);

  useEffect(() => {
    if (!formOpen) {
      return;
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setFormOpen(false);
      }
    };
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [formOpen]);

  const builtinTools = useMemo(
    () => sortToolsByEnabled(tools.filter((tool) => tool.source !== "custom")),
    [tools]
  );
  const customTools = useMemo(
    () => sortToolsByEnabled(tools.filter((tool) => tool.source === "custom")),
    [tools]
  );

  const normalizedSearchQuery = searchQuery.trim().toLowerCase();
  const visibleBuiltinTools = useMemo(() => {
    if (!normalizedSearchQuery) return builtinTools;
    return builtinTools.filter((tool) =>
      tool.name.toLowerCase().includes(normalizedSearchQuery) ||
      tool.id.toLowerCase().includes(normalizedSearchQuery)
    );
  }, [builtinTools, normalizedSearchQuery]);
  const visibleCustomTools = useMemo(() => {
    if (!normalizedSearchQuery) return customTools;
    return customTools.filter((tool) =>
      tool.name.toLowerCase().includes(normalizedSearchQuery) ||
      tool.id.toLowerCase().includes(normalizedSearchQuery)
    );
  }, [customTools, normalizedSearchQuery]);
  const bulkToggleLabel = bulkToggling
    ? t("tools.bulkUpdating")
    : bulkToggleMode === "enable"
      ? t("tools.bulkEnable")
      : t("tools.bulkDisable");

  const toolEditorTool = useMemo(
    () => tools.find((tool) => tool.id === toolEditorToolId) ?? null,
    [tools, toolEditorToolId],
  );

  const skillIds = useMemo(
    () => {
      if (!toolEditorTool || toolEditorTool.detected) {
        return skills.map((skill) => skill.instance_id);
      }

      // A disabled/undetected manager integration can still own directly
      // installed skills. Keep those rows actionable without exposing every
      // global skill as if the provider were available.
      return skills
        .filter((skill) => skill.scope === "tool" && skill.tool_id === toolEditorTool.id)
        .map((skill) => skill.instance_id);
    },
    [skills, toolEditorTool],
  );

  const toolSkillEnabledMap = useMemo(() => {
    if (!toolEditorTool) {
      return {};
    }
    const enabledMap: Record<string, boolean> = {};
    skills.forEach((skill) => {
      enabledMap[skill.instance_id] = Boolean(skill.enabled[toolEditorTool.id]);
    });
    return enabledMap;
  }, [skills, toolEditorTool]);

  const toolEditorOrderedSkillIds = useMemo(() => {
    if (!toolEditorTool) {
      return [];
    }
    return orderSkillIdsForTool(skillIds, toolSkillEnabledMap);
  }, [skillIds, toolEditorTool, toolSkillEnabledMap]);

  const toolEditorFilteredSkillIds = useMemo(() => {
    if (!toolEditorTool) {
      return [];
    }

    const normalizedQuery = toolEditorQuery.trim().toLowerCase();
    return toolEditorOrderedSkillIds.filter((skillId) => {
      if (toolEditorEnabledOnly && !toolSkillEnabledMap[skillId]) {
        return false;
      }

      if (!normalizedQuery) {
        return true;
      }

      const displayName = getSkillDisplayName(skillId, skills).toLowerCase();
      return displayName.includes(normalizedQuery) || skillId.toLowerCase().includes(normalizedQuery);
    });
  }, [
    skills,
    toolEditorEnabledOnly,
    toolEditorOrderedSkillIds,
    toolEditorQuery,
    toolEditorTool,
    toolSkillEnabledMap,
  ]);

  const toolEditorEnabledCount = useMemo(() => {
    if (!toolEditorTool) {
      return 0;
    }
    return toolEditorOrderedSkillIds.filter((skillId) => toolSkillEnabledMap[skillId]).length;
  }, [toolEditorOrderedSkillIds, toolEditorTool, toolSkillEnabledMap]);

  const toolEditorBulkToggleMode = useMemo(() => {
    if (!toolEditorTool) {
      return "enable";
    }
    return getToolBulkToggleMode(toolEditorFilteredSkillIds, toolSkillEnabledMap);
  }, [toolEditorFilteredSkillIds, toolEditorTool, toolSkillEnabledMap]);

  const toolEditorBulkToggleTargets = useMemo(() => {
    if (!toolEditorTool) {
      return [];
    }
    return getToolBulkToggleTargets(toolEditorFilteredSkillIds, toolSkillEnabledMap, toolEditorBulkToggleMode);
  }, [toolEditorFilteredSkillIds, toolEditorTool, toolEditorBulkToggleMode, toolSkillEnabledMap]);

  const toolEditorIsBulkToggling = toolEditorTool ? bulkTogglingToolId === toolEditorTool.id : false;
  const toolEditorHasPendingSingleToggle = toolEditorTool
    ? Boolean(togglingSkill?.startsWith(`${toolEditorTool.id}:`))
    : false;
  const toolEditorBulkToggleDisabled =
    toolEditorIsBulkToggling || toolEditorHasPendingSingleToggle || toolEditorBulkToggleTargets.length === 0;
  const toolEditorBulkToggleLabel = toolEditorIsBulkToggling
    ? t("tools.bulkUpdatingSkills")
    : toolEditorBulkToggleMode === "enable"
      ? t("tools.bulkEnableSkills")
      : t("tools.bulkDisableSkills");

  const toolEditorItems = useMemo(() => {
    if (!toolEditorTool) {
      return [];
    }

    // The manager toggle controls global sync behavior, not whether the
    // tool's own installed Skills can be managed.
    const shouldDisable = !toolEditorTool.detected && toolEditorOrderedSkillIds.length === 0;

    return toolEditorFilteredSkillIds.map((skillId) => {
      const isEnabled = toolSkillEnabledMap[skillId] ?? false;
      const toggleKey = `${toolEditorTool.id}:${skillId}`;
      const isToggling = togglingSkill === toggleKey;
      const isDisabled = toolEditorIsBulkToggling || isToggling || shouldDisable;
      const tooltip = !toolEditorTool.detected ? t("skills.toolNotDetected") : undefined;

      return {
        id: skillId,
        label: getSkillDisplayName(skillId, skills),
        enabled: isEnabled,
        disabled: isDisabled,
        tooltip,
        dimmed: !toolEditorTool.config.enabled && !isEnabled,
      };
    });
  }, [
    skills,
    t,
    togglingSkill,
    toolEditorFilteredSkillIds,
    toolEditorIsBulkToggling,
    toolEditorTool,
    toolSkillEnabledMap,
  ]);

  const handleIconError = useCallback((toolId: string) => {
    setIconFallbackStage(prev => {
      const current = prev[toolId] ?? "asset";
      const nextStage = current === "asset" ? "file" : "none";
      return { ...prev, [toolId]: nextStage };
    });
  }, []);

  // Show skeleton while initial loading
  if (initialLoading) {
    return (
      <div style={{
        flex: 1,
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        overflow: 'hidden',
        backgroundColor: 'var(--background)',
      }}>
        <PageHeader title={t("tools.title")} />
        <main className="page-main" style={{ flex: 1, minHeight: 0, overflow: 'auto' }}>
          <PageLoader />
        </main>
      </div>
    );
  }

  const renderToolCard = (tool: Tool) => {
    const isCustom = tool.source === "custom";
    const iconStage = iconFallbackStage[tool.id] ?? "asset";
    const customIconSrc = tool.icon_path
      ? iconStage === "asset"
        ? convertFileSrc(tool.icon_path)
        : iconStage === "file"
          ? `file://${tool.icon_path}`
          : null
      : null;
    const iconUrl = customIconSrc || getToolIconUrl(tool.id);
    const hasDirectSkills = skills.some(
      (skill) => skill.scope === "tool" && skill.tool_id === tool.id,
    );
    const manageSkillsDisabled = !tool.detected && !hasDirectSkills;
    const manageSkillsTitle = !tool.detected
      ? hasDirectSkills ? t("tools.manageDirectSkills") : t("skills.toolNotDetected")
      : t("tools.manageSkills");

    return (
      <div
        key={tool.id}
        style={{
          display: 'flex',
          flexDirection: 'column',
          padding: '18px 20px',
          backgroundColor: 'var(--secondary)',
          borderRadius: "var(--radius)",
          border: '1px solid var(--border)',
          transition: 'border-color 0.2s, box-shadow 0.2s, transform 0.2s',
          cursor: 'pointer',
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.borderColor = 'var(--ring)';
          e.currentTarget.style.boxShadow = '0 4px 12px var(--surface-hover)';
          e.currentTarget.style.transform = 'translateY(-2px)';
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.borderColor = 'var(--border)';
          e.currentTarget.style.boxShadow = 'none';
          e.currentTarget.style.transform = 'translateY(0)';
        }}
      >
        {/* Top: Icon + Title + Status + Toggle */}
        <div style={{ display: 'flex', gap: '14px', marginBottom: '16px', alignItems: 'flex-start' }}>
          {/* Icon */}
          {iconUrl ? (
            <img
              src={iconUrl}
              alt={tool.name}
              style={{
                width: 44,
                height: 44,
                borderRadius: 12,
                flexShrink: 0,
                objectFit: 'cover',
              }}
              onError={() => {
                if (tool.icon_path) {
                  handleIconError(tool.id);
                }
              }}
            />
          ) : (
            <GenericToolIcon />
          )}

          {/* Title + Status */}
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{
              display: 'flex',
              alignItems: 'center',
              gap: '8px',
              marginBottom: '4px',
              flexWrap: 'wrap',
            }}>
              <span style={{
                fontSize: '15px',
                fontWeight: 600,
                color: 'var(--foreground)',
                lineHeight: 1.3,
              }}>
                {tool.name}
              </span>
              <span style={{
                fontSize: '11px',
                fontWeight: 500,
                padding: '2px 8px',
                borderRadius: '6px',
                backgroundColor: tool.detected ? 'var(--color-success-bg)' : 'var(--secondary)',
                color: tool.detected ? 'var(--color-success)' : 'var(--muted-foreground)',
                border: tool.detected ? '1px solid var(--color-success-border)' : '1px solid var(--border)',
              }}>
                {tool.detected ? t("tools.detectedStatus") : t("tools.notDetected")}
              </span>
              {tool.cli_available && (
                <span style={{
                  fontSize: '11px',
                  fontWeight: 500,
                  padding: '2px 8px',
                  borderRadius: '6px',
                  backgroundColor: 'var(--background)',
                  color: 'var(--muted-foreground)',
                  border: '1px solid var(--border)',
                }}>
                  CLI
                </span>
              )}
              {hasDirectSkills && (
                <span style={{
                  fontSize: '11px',
                  fontWeight: 500,
                  padding: '2px 8px',
                  borderRadius: '6px',
                  backgroundColor: 'var(--primary-tint)',
                  color: 'var(--primary)',
                  border: '1px solid var(--primary-tint-border)',
                }}>
                  {t("tools.directSkills")}
                </span>
              )}
              {isCustom && (
                <div style={{ display: 'flex', gap: '6px' }}>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      startEditCustomTool(tool);
                    }}
                    title={t("tools.customEdit")}
                    style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      width: '24px',
                      height: '24px',
                      borderRadius: '6px',
                      border: '1px solid var(--border)',
                      background: 'var(--background)',
                      color: 'var(--muted-foreground)',
                      cursor: 'pointer',
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.backgroundColor = 'var(--muted)';
                      e.currentTarget.style.color = 'var(--foreground)';
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.backgroundColor = 'var(--background)';
                      e.currentTarget.style.color = 'var(--muted-foreground)';
                    }}
                  >
                    <Pencil style={{ width: '12px', height: '12px' }} />
                  </button>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      handleDeleteCustomTool(tool);
                    }}
                    title={t("tools.customDelete")}
                    style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      width: '24px',
                      height: '24px',
                      borderRadius: '6px',
                      border: '1px solid var(--border)',
                      background: 'var(--background)',
                      color: 'var(--color-danger)',
                      cursor: 'pointer',
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.backgroundColor = 'var(--muted)';
                      e.currentTarget.style.color = 'var(--color-danger)';
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.backgroundColor = 'var(--background)';
                    }}
                  >
                    <Trash2 style={{ width: '12px', height: '12px' }} />
                  </button>
                </div>
              )}
            </div>
            <p style={{
              fontSize: '13px',
              color: 'var(--muted-foreground)',
              margin: 0,
              lineHeight: 1.5,
            }}>
              ID: {tool.id}
            </p>
          </div>

          {/* Skill Dialog + Toggle */}
          <div
            onClick={(e) => e.stopPropagation()}
            style={{ marginTop: '2px', display: 'flex', alignItems: 'center', gap: '8px' }}
          >
            <button
              onClick={(e) => {
                e.stopPropagation();
                if (!manageSkillsDisabled) {
                  openToolSkillEditor(tool.id);
                }
              }}
              title={manageSkillsTitle}
              disabled={manageSkillsDisabled}
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                justifyContent: 'center',
                width: '28px',
                height: '28px',
                borderRadius: '8px',
                border: '1px solid var(--border)',
                background: 'var(--background)',
                color: 'var(--muted-foreground)',
                cursor: manageSkillsDisabled ? 'not-allowed' : 'pointer',
                opacity: manageSkillsDisabled ? 0.5 : 1,
                transition: 'background-color 0.15s, color 0.15s, border-color 0.15s',
                flexShrink: 0,
              }}
              onMouseEnter={(e) => {
                if (manageSkillsDisabled) {
                  return;
                }
                e.currentTarget.style.backgroundColor = 'var(--muted)';
                e.currentTarget.style.color = 'var(--foreground)';
                e.currentTarget.style.borderColor = 'var(--ring)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = 'var(--background)';
                e.currentTarget.style.color = 'var(--muted-foreground)';
                e.currentTarget.style.borderColor = 'var(--border)';
              }}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M12 3L13.5 8.5L19 10L13.5 11.5L12 17L10.5 11.5L5 10L10.5 8.5L12 3Z"/>
              </svg>
            </button>
            <Toggle
              checked={tool.config.enabled}
              disabled={!tool.detected && !tool.config.enabled}
              onChange={(enabled) => toggleToolEnabled(tool, enabled)}
              title={!tool.detected && !tool.config.enabled
                ? t("tools.notDetectedTooltip").replace("{tool}", tool.name)
                : undefined}
            />
          </div>
        </div>

        {/* Bottom: Config Info */}
        <div style={{
          paddingTop: '14px',
          borderTop: '1px solid var(--border)',
          display: 'flex',
          flexDirection: 'column',
          gap: '8px',
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span style={{
              fontSize: '12px',
              color: 'var(--muted-foreground)',
              flexShrink: 0,
              width: '80px',
            }}>
              {t("tools.configPath")}
            </span>
            <code style={{
              flex: 1,
              fontSize: '11px',
              color: 'var(--foreground)',
              backgroundColor: 'var(--background)',
              padding: '2px 6px',
              borderRadius: '4px',
              wordBreak: 'break-all',
            }}>
              {tool.config.config_path || t("tools.notSet")}
            </code>
            {/* 打开路径选择器修改配置路径，默认定位到当前路径，仅启用的工具可点击 */}
            {!isCustom && (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  if (tool.config.enabled) {
                    handleEditConfigPath(tool.id, tool.config.config_path);
                  }
                }}
                title={tool.config.enabled ? t("tools.editPath") : t("skills.toolNotDetected")}
                disabled={!tool.config.enabled}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  width: '24px',
                  height: '24px',
                  borderRadius: '6px',
                  border: 'none',
                  background: 'transparent',
                  color: 'var(--muted-foreground)',
                  cursor: tool.config.enabled ? 'pointer' : 'not-allowed',
                  flexShrink: 0,
                  opacity: tool.config.enabled ? 1 : 0.4,
                }}
                onMouseEnter={(e) => {
                  if (tool.config.enabled) {
                    e.currentTarget.style.backgroundColor = 'var(--muted)';
                    e.currentTarget.style.color = 'var(--foreground)';
                  }
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.backgroundColor = 'transparent';
                  e.currentTarget.style.color = 'var(--muted-foreground)';
                }}
              >
                <FolderOpen style={{ width: '14px', height: '14px' }} />
              </button>
            )}
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span style={{
              fontSize: '12px',
              color: 'var(--muted-foreground)',
              flexShrink: 0,
              width: '80px',
            }}>
              {t("tools.skillsPath")}
            </span>
            <code style={{
              flex: 1,
              fontSize: '11px',
              color: 'var(--foreground)',
              backgroundColor: 'var(--background)',
              padding: '2px 6px',
              borderRadius: '4px',
              wordBreak: 'break-all',
            }}>
              {tool.config.skills_path || t("tools.notSet")}
            </code>
            {/* 打开路径选择器修改技能路径，默认定位到当前路径，仅启用的工具可点击 */}
            {!isCustom && (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  if (tool.config.enabled) {
                    handleEditSkillsPath(tool.id, tool.config.skills_path);
                  }
                }}
                title={tool.config.enabled ? t("tools.editPath") : t("skills.toolNotDetected")}
                disabled={!tool.config.enabled}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  width: '24px',
                  height: '24px',
                  borderRadius: '6px',
                  border: 'none',
                  background: 'transparent',
                  color: 'var(--muted-foreground)',
                  cursor: tool.config.enabled ? 'pointer' : 'not-allowed',
                  flexShrink: 0,
                  opacity: tool.config.enabled ? 1 : 0.4,
                }}
                onMouseEnter={(e) => {
                  if (tool.config.enabled) {
                    e.currentTarget.style.backgroundColor = 'var(--muted)';
                    e.currentTarget.style.color = 'var(--foreground)';
                  }
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.backgroundColor = 'transparent';
                  e.currentTarget.style.color = 'var(--muted-foreground)';
                }}
              >
                <FolderOpen style={{ width: '14px', height: '14px' }} />
              </button>
            )}
          </div>
        </div>

        {isCustom && tool.icon_path && iconFallbackStage[tool.id] === "none" && (
          <p style={{
            marginTop: '10px',
            fontSize: '11px',
            color: 'var(--muted-foreground)',
          }}>
            {t("tools.customIconLoadFailed")}
          </p>
        )}
      </div>
    );
  };

  return (
    <div style={{
      flex: 1,
      display: 'flex',
      flexDirection: 'column',
      height: '100%',
      overflow: 'hidden',
      backgroundColor: 'var(--background)',
    }}>
      <PageHeader
        title={t("tools.title")}
        actions={
          <>
            <RefreshButton onClick={handleRefresh} loading={refreshing} iconOnly />
            <button
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                justifyContent: 'center',
                width: 32,
                height: 32,
                padding: 0,
                color: bulkToggling || refreshing || bulkToggleTargets.length === 0
                  ? 'var(--muted-foreground)'
                  : 'var(--muted-foreground)',
                backgroundColor: 'transparent',
                border: '1px solid transparent',
                borderRadius: '6px',
                cursor: bulkToggling || refreshing || bulkToggleTargets.length === 0 ? 'not-allowed' : 'pointer',
                opacity: bulkToggling || refreshing || bulkToggleTargets.length === 0 ? 0.5 : 1,
                transition: 'color 0.15s, background-color 0.15s',
              }}
              onMouseEnter={(e) => {
                if (bulkToggling || refreshing || bulkToggleTargets.length === 0) {
                  return;
                }
                e.currentTarget.style.color = 'var(--foreground)';
                e.currentTarget.style.backgroundColor = 'var(--secondary)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.color = 'var(--muted-foreground)';
                e.currentTarget.style.backgroundColor = 'transparent';
              }}
              onClick={handleBulkToggleTools}
              disabled={bulkToggling || refreshing || bulkToggleTargets.length === 0}
              title={bulkToggleTargets.length === 0 ? t("tools.bulkNoTarget") : bulkToggleLabel}
              aria-label={bulkToggleTargets.length === 0 ? t("tools.bulkNoTarget") : bulkToggleLabel}
            >
              <Power
                style={{
                  width: '14px',
                  height: '14px',
                  animation: bulkToggling ? 'spin 1s linear infinite' : 'none',
                }}
              />
            </button>
            <button
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                justifyContent: 'center',
                width: 32,
                height: 32,
                padding: 0,
                color: 'var(--muted-foreground)',
                backgroundColor: 'transparent',
                border: '1px solid transparent',
                borderRadius: '6px',
                cursor: 'pointer',
                transition: 'color 0.15s, background-color 0.15s',
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.color = 'var(--foreground)';
                e.currentTarget.style.backgroundColor = 'var(--secondary)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.color = 'var(--muted-foreground)';
                e.currentTarget.style.backgroundColor = 'transparent';
              }}
              onClick={startCreateCustomTool}
              title={t("tools.customAdd")}
              aria-label={t("tools.customAdd")}
            >
              <Plus style={{ width: '14px', height: '14px' }} />
            </button>
          </>
        }
      />

      {/* Content */}
      <main className="page-main" style={{
        flex: 1,
        minHeight: 0,
        overflow: 'auto',
      }}>
        <div className="page-container" style={{ maxWidth: '1200px' }}>
          {/* Error */}
          {error && (
            <div className="mb-6">
              <Alert variant="destructive">
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            </div>
          )}

          {lastOperationReport && (
            <OperationReportCard
              report={lastOperationReport}
              scopeLabel={t("skills.scopeGlobal")}
              providerLabel={
                lastOperationReport.provider_id
                  ? tools.find((tool) => tool.id === lastOperationReport.provider_id)?.name
                    ?? lastOperationReport.provider_id
                  : t("tools.title")
              }
            />
          )}

          {/* Section: Detected Tools */}
          <section>
            <h2 style={{
              fontSize: '13px',
              fontWeight: 500,
              color: 'var(--muted-foreground)',
              margin: '0 0 16px 0',
            }}>
              {t("tools.detected")}
            </h2>

            {builtinTools.length === 0 ? (
              <div style={{
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                justifyContent: 'center',
                padding: '64px 24px',
                textAlign: 'center',
                gap: 12,
              }}>
                <div style={{ fontSize: 32, color: 'var(--ember)', opacity: 0.5 }}>✦</div>
                <div style={{ fontSize: 16, fontWeight: 600, color: 'var(--foreground)' }}>{t("tools.noTools")}</div>
                <div style={{ fontSize: 13, color: 'var(--muted-foreground)', maxWidth: 360 }}>
                  {t("tools.noToolsDesc")}
                </div>
              </div>
            ) : visibleBuiltinTools.length === 0 ? (
              <div style={{
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                justifyContent: 'center',
                padding: '48px 24px',
                textAlign: 'center',
                gap: 8,
              }}>
                <div style={{ fontSize: 24, color: 'var(--ember)', opacity: 0.4 }}>✦</div>
                <div style={{ fontSize: 14, fontWeight: 500, color: 'var(--foreground)' }}>{t("tools.noMatch")}</div>
              </div>
            ) : (
              <div className="card-grid">
                {visibleBuiltinTools.map(renderToolCard)}
              </div>
            )}
          </section>

          {/* Section: Custom Tools */}
          <section style={{ marginTop: '32px' }}>
            <div style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              marginBottom: '16px',
            }}>
              <h2 style={{
                fontSize: '13px',
                fontWeight: 500,
                color: 'var(--muted-foreground)',
                margin: 0,
              }}>
                {t("tools.customTitle")}
              </h2>
              <div />
            </div>

            {customTools.length === 0 ? (
              <div style={{
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                justifyContent: 'center',
                padding: '64px 24px',
                textAlign: 'center',
                gap: 12,
              }}>
                <div style={{ fontSize: 32, color: 'var(--ember)', opacity: 0.5 }}>✦</div>
                <div style={{ fontSize: 16, fontWeight: 600, color: 'var(--foreground)' }}>{t("tools.customEmpty")}</div>
                <div style={{ fontSize: 13, color: 'var(--muted-foreground)', maxWidth: 360 }}>
                  {t("tools.customEmptyDesc")}
                </div>
                <button
                  onClick={startCreateCustomTool}
                  style={{
                    width: '42px',
                    height: '42px',
                    borderRadius: '50%',
                    border: '1px dashed var(--border)',
                    background: 'var(--background)',
                    color: 'var(--muted-foreground)',
                    display: 'inline-flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    cursor: 'pointer',
                    marginTop: 8,
                  }}
                >
                  <Plus style={{ width: '18px', height: '18px' }} />
                </button>
              </div>
            ) : visibleCustomTools.length === 0 ? (
              <div style={{
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                justifyContent: 'center',
                padding: '48px 24px',
                textAlign: 'center',
                gap: 8,
              }}>
                <div style={{ fontSize: 24, color: 'var(--ember)', opacity: 0.4 }}>✦</div>
                <div style={{ fontSize: 14, fontWeight: 500, color: 'var(--foreground)' }}>{t("tools.noMatch")}</div>
              </div>
            ) : (
              <div className="card-grid">
                {visibleCustomTools.map(renderToolCard)}
              </div>
            )}
          </section>
        </div>
      </main>

      {toolEditorTool && (
        <RelationToggleDialog
          title={t("tools.configureSkillsTitle")}
          description={t("tools.configureSkillsDesc")
            .replace("{tool}", toolEditorTool.name)
            .replace("{enabled}", String(toolEditorEnabledCount))
            .replace("{total}", String(toolEditorOrderedSkillIds.length))}
          query={toolEditorQuery}
          enabledOnly={toolEditorEnabledOnly}
          searchPlaceholder={t("tools.searchSkillsPlaceholder")}
          enabledOnlyLabel={t("tools.enabledOnlySkills")}
          bulkToggleLabel={toolEditorBulkToggleLabel}
          bulkToggleDisabled={toolEditorBulkToggleDisabled}
          bulkToggleTitle={toolEditorBulkToggleTargets.length === 0 ? t("tools.bulkNoSkillTarget") : undefined}
          items={toolEditorItems}
          emptyLabel={t("tools.noSkillsInFilter")}
          doneLabel={t("common.done")}
          onQueryChange={setToolEditorQuery}
          onEnabledOnlyChange={setToolEditorEnabledOnly}
          onToggle={(skillId, enabled) => handleToggleSkillForTool(toolEditorTool, skillId, enabled)}
          onBulkToggle={() => handleBulkToggleToolSkills(toolEditorTool, toolEditorFilteredSkillIds)}
          onClose={closeToolSkillEditor}
        />
      )}

      {formOpen && (
        <div
          style={{
            position: 'fixed',
            inset: 0,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: 'rgba(0, 0, 0, 0.45)',
            zIndex: 50,
            padding: '24px',
          }}
          onClick={(event) => {
            if (event.target === event.currentTarget) {
              setFormOpen(false);
            }
          }}
        >
          <div
            className="animate-modal"
            style={{
              width: 'min(520px, 92vw)',
              maxHeight: '88vh',
              display: 'flex',
              flexDirection: 'column',
              overflow: 'hidden',
              background: 'var(--background)',
              border: '1px solid var(--border)',
              borderRadius: 'var(--radius-xl)',
              boxShadow: '0 18px 60px rgba(0,0,0,0.25)',
            }}
            onClick={(event) => event.stopPropagation()}
          >
            {/* Header */}
            <div
              style={{
                display: 'flex',
                alignItems: 'flex-start',
                justifyContent: 'space-between',
                gap: '12px',
                padding: '16px 18px 12px',
                borderBottom: '1px solid var(--border)',
              }}
            >
              <div style={{ minWidth: 0, flex: 1 }}>
                <h3
                  style={{
                    margin: 0,
                    fontSize: '14px',
                    fontWeight: 600,
                    color: 'var(--foreground)',
                    letterSpacing: '-0.01em',
                  }}
                >
                  {editingToolId ? t("tools.customEditTitle") : t("tools.customCreateTitle")}
                </h3>
                <p
                  style={{
                    margin: '4px 0 0 0',
                    fontSize: '12px',
                    color: 'var(--muted-foreground)',
                    lineHeight: 1.45,
                  }}
                >
                  {editingToolId ? t("tools.customIdLocked") : t("tools.customEmptyDesc")}
                </p>
              </div>
              <button
                onClick={() => setFormOpen(false)}
                title={t("common.cancel")}
                style={{
                  width: '26px',
                  height: '26px',
                  borderRadius: 'var(--radius-sm)',
                  border: '1px solid var(--border)',
                  backgroundColor: 'var(--secondary)',
                  color: 'var(--muted-foreground)',
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  padding: 0,
                  flexShrink: 0,
                  transition: 'background-color 0.15s, color 0.15s, border-color 0.15s',
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.backgroundColor = 'var(--muted)';
                  e.currentTarget.style.color = 'var(--foreground)';
                  e.currentTarget.style.borderColor = 'var(--ring)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.backgroundColor = 'var(--secondary)';
                  e.currentTarget.style.color = 'var(--muted-foreground)';
                  e.currentTarget.style.borderColor = 'var(--border)';
                }}
              >
                <X style={{ width: '14px', height: '14px' }} />
              </button>
            </div>

            {/* Body */}
            <div style={{ padding: '16px 18px', overflow: 'auto' }}>
              {formError && (
                <div style={{ marginBottom: '16px' }}>
                  <Alert variant="destructive">
                    <AlertDescription>{formError}</AlertDescription>
                  </Alert>
                </div>
              )}

              <div style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
                <div>
                  <label style={fieldLabelStyle}>{t("tools.customNameLabel")}</label>
                  <Input
                    value={form.name}
                    onChange={(e) => handleCustomNameChange(e.target.value)}
                    placeholder={t("tools.customNamePlaceholder")}
                    style={formInputStyle}
                  />
                </div>
                <div>
                  <label style={fieldLabelStyle}>{t("tools.customIdLabel")}</label>
                  <Input
                    value={form.id}
                    onChange={(e) => handleCustomIdChange(e.target.value)}
                    placeholder={t("tools.customIdPlaceholder")}
                    disabled={!!editingToolId}
                    style={{
                      ...formInputStyle,
                      opacity: editingToolId ? 0.7 : 1,
                      cursor: editingToolId ? 'not-allowed' : 'text',
                      color: editingToolId ? 'var(--muted-foreground)' : 'var(--foreground)',
                      WebkitTextFillColor: editingToolId ? 'var(--muted-foreground)' : 'var(--foreground)',
                    }}
                  />
                  {!editingToolId && (
                    <span style={{ fontSize: '11px', color: 'var(--muted-foreground)' }}>
                      {t("tools.customIdHint")}
                    </span>
                  )}
                </div>
                <div>
                  <label style={fieldLabelStyle}>{t("tools.customConfigPathLabel")}</label>
                  <div style={{ display: 'flex', gap: '8px' }}>
                    <Input
                      value={form.configPath}
                      onChange={(e) => setForm(prev => ({ ...prev, configPath: e.target.value }))}
                      placeholder={t("tools.customConfigPathPlaceholder")}
                      style={formInputStyle}
                    />
                    <button
                      onClick={handleSelectCustomConfigPath}
                      title={t("tools.selectConfigPath")}
                      style={pickerButtonStyle}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.backgroundColor = 'var(--muted)';
                        e.currentTarget.style.color = 'var(--foreground)';
                        e.currentTarget.style.borderColor = 'var(--ring)';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.backgroundColor = 'var(--background)';
                        e.currentTarget.style.color = 'var(--muted-foreground)';
                        e.currentTarget.style.borderColor = 'var(--border)';
                      }}
                    >
                      <FolderOpen style={{ width: '16px', height: '16px' }} />
                    </button>
                  </div>
                </div>
                <div>
                  <label style={fieldLabelStyle}>{t("tools.customSkillsPathLabel")}</label>
                  <div style={{ display: 'flex', gap: '8px' }}>
                    <Input
                      value={form.skillsPath}
                      onChange={(e) => setForm(prev => ({ ...prev, skillsPath: e.target.value }))}
                      placeholder={t("tools.customSkillsPathPlaceholder")}
                      style={formInputStyle}
                    />
                    <button
                      onClick={handleSelectCustomSkillsPath}
                      title={t("tools.selectSkillsPath")}
                      style={pickerButtonStyle}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.backgroundColor = 'var(--muted)';
                        e.currentTarget.style.color = 'var(--foreground)';
                        e.currentTarget.style.borderColor = 'var(--ring)';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.backgroundColor = 'var(--background)';
                        e.currentTarget.style.color = 'var(--muted-foreground)';
                        e.currentTarget.style.borderColor = 'var(--border)';
                      }}
                    >
                      <FolderOpen style={{ width: '16px', height: '16px' }} />
                    </button>
                  </div>
                </div>
                <div>
                  <label style={fieldLabelStyle}>{t("tools.customIconPathLabel")}</label>
                  <div style={{ display: 'flex', gap: '8px' }}>
                    <Input
                      value={form.iconPath}
                      onChange={(e) => setForm(prev => ({ ...prev, iconPath: e.target.value }))}
                      placeholder={t("tools.customIconPathPlaceholder")}
                      style={formInputStyle}
                    />
                    <button
                      onClick={handleSelectCustomIconPath}
                      title={t("tools.selectIconPath")}
                      style={pickerButtonStyle}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.backgroundColor = 'var(--muted)';
                        e.currentTarget.style.color = 'var(--foreground)';
                        e.currentTarget.style.borderColor = 'var(--ring)';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.backgroundColor = 'var(--background)';
                        e.currentTarget.style.color = 'var(--muted-foreground)';
                        e.currentTarget.style.borderColor = 'var(--border)';
                      }}
                    >
                      <FolderOpen style={{ width: '16px', height: '16px' }} />
                    </button>
                  </div>
                </div>
              </div>
            </div>

            {/* Footer */}
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'flex-end',
                gap: '10px',
                padding: '12px 18px 14px',
                borderTop: '1px solid var(--border)',
              }}
            >
              <button
                onClick={() => setFormOpen(false)}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '6px',
                  padding: '7px 14px',
                  borderRadius: 'var(--radius-sm)',
                  border: '1px solid var(--border)',
                  background: 'var(--background)',
                  color: 'var(--foreground)',
                  cursor: 'pointer',
                  fontSize: '12px',
                  fontWeight: 500,
                  transition: 'background-color 0.15s, border-color 0.15s',
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.backgroundColor = 'var(--muted)';
                  e.currentTarget.style.borderColor = 'var(--ring)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.backgroundColor = 'var(--background)';
                  e.currentTarget.style.borderColor = 'var(--border)';
                }}
              >
                {t("common.cancel")}
              </button>
              <button
                onClick={handleSaveCustomTool}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '6px',
                  padding: '7px 16px',
                  borderRadius: 'var(--radius-sm)',
                  border: 'none',
                  background: 'var(--foreground)',
                  color: 'var(--primary-foreground)',
                  cursor: 'pointer',
                  fontSize: '12px',
                  fontWeight: 500,
                  transition: 'opacity 0.15s',
                }}
                onMouseEnter={(e) => { e.currentTarget.style.opacity = '0.85'; }}
                onMouseLeave={(e) => { e.currentTarget.style.opacity = '1'; }}
              >
                {t("common.save")}
              </button>
            </div>
          </div>
        </div>
      )}
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
