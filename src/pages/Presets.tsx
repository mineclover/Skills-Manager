import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "@/i18n";
import { useToast } from "@/components/ui/toast";
import { PageHeader } from "@/components/ui/page-header";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { AppConfig, Tool, Skill, SkillActivationPreset, PresetActivation } from "@/types";
import { Sliders, Plus, Trash2, Play, Check, AlertTriangle, Layers, Download } from "lucide-react";

export function Presets() {
  const { t } = useTranslation();
  const { addToast } = useToast();

  const [config, setConfig] = useState<AppConfig | null>(null);
  const [tools, setTools] = useState<Tool[]>([]);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [selectedPresetId, setSelectedPresetId] = useState<string | null>(null);
  const [isNewPresetDialogOpen, setIsNewPresetDialogOpen] = useState(false);
  const [newPresetName, setNewPresetName] = useState("");
  const [newPresetDesc, setNewPresetDesc] = useState("");
  const [copyCurrentState, setCopyCurrentState] = useState(true);
  const [applyingPresetId, setApplyingPresetId] = useState<string | null>(null);

  // Load all data on mount
  useEffect(() => {
    fetchData();
  }, []);

  async function fetchData() {
    try {
      const cfg = await invoke<AppConfig>("get_config");
      setConfig(cfg);

      const tList = await invoke<Tool[]>("detect_tools");
      setTools(tList.filter((t) => t.detected && t.config.enabled));

      const sList = await invoke<Skill[]>("list_skills");
      setSkills(sList);

      // Select active preset or first preset by default
      const savedPresets = cfg.presets || [];
      if (cfg.active_preset_id) {
        setSelectedPresetId(cfg.active_preset_id);
      } else if (savedPresets.length > 0) {
        setSelectedPresetId(savedPresets[0].id);
      }
    } catch (err) {
      console.error("Failed to load data:", err);
      addToast(t("skills.loadFailed"), "error");
    }
  }

  const presetsList = config?.presets || [];
  const selectedPreset = presetsList.find((p) => p.id === selectedPresetId);

  // Create new preset
  async function handleCreatePreset() {
    if (!config) return;
    if (!newPresetName.trim()) {
      addToast(t("skills.nameRequired"), "error");
      return;
    }

    const newId = `preset-${Date.now()}`;
    const activations: PresetActivation[] = [];
    if (copyCurrentState) {
      tools.forEach((tool) => {
        const activeSkillIds = skills
          .filter((skill) => skill.enabled[tool.id] === true)
          .map((skill) => skill.instance_id);
        
        // We push even if activeSkillIds is empty, to explicitly deactivate
        activations.push({
          tool_id: tool.id,
          skill_ids: activeSkillIds,
        });
      });
    }

    const newPreset: SkillActivationPreset = {
      id: newId,
      name: newPresetName.trim(),
      description: newPresetDesc.trim() || null,
      activations,
    };

    const updatedConfig = {
      ...config,
      presets: [...presetsList, newPreset],
    };

    try {
      await invoke("save_config", { config: updatedConfig });
      setConfig(updatedConfig);
      setSelectedPresetId(newId);
      setIsNewPresetDialogOpen(false);
      setNewPresetName("");
      setNewPresetDesc("");
      addToast(
        t("presets.createSuccess").replace("{name}", newPreset.name),
        "success"
      );
    } catch (err) {
      addToast(err instanceof Error ? err.message : t("settings.saveFailed"), "error");
    }
  }

  // Delete preset
  async function handleDeletePreset(presetId: string) {
    if (!config) return;
    const target = presetsList.find((p) => p.id === presetId);
    if (!target) return;

    if (!confirm(t("presets.deleteConfirm").replace("{name}", target.name))) {
      return;
    }

    const isActive = config.active_preset_id === presetId;
    const updatedPresets = presetsList.filter((p) => p.id !== presetId);
    const updatedConfig = {
      ...config,
      presets: updatedPresets,
      active_preset_id: isActive ? null : config.active_preset_id,
    };

    try {
      await invoke("save_config", { config: updatedConfig });
      setConfig(updatedConfig);

      if (isActive) {
        await invoke("clear_active_preset");
      }

      // Switch selection
      if (updatedPresets.length > 0) {
        setSelectedPresetId(updatedPresets[0].id);
      } else {
        setSelectedPresetId(null);
      }

      addToast(
        t("presets.deleteSuccess").replace("{name}", target.name),
        "success"
      );
    } catch (err) {
      addToast(err instanceof Error ? err.message : t("settings.saveFailed"), "error");
    }
  }

  // Capture current state to selected preset
  async function handleCaptureCurrentToPreset(presetId: string) {
    if (!config) return;
    const target = presetsList.find((p) => p.id === presetId);
    if (!target) return;

    if (!confirm(t("presets.captureConfirm").replace("{name}", target.name))) {
      return;
    }

    const activations: PresetActivation[] = tools.map((tool) => {
      const activeSkillIds = skills
        .filter((skill) => skill.enabled[tool.id] === true)
        .map((skill) => skill.instance_id);
      return {
        tool_id: tool.id,
        skill_ids: activeSkillIds,
      };
    });

    const updatedPresets = presetsList.map((preset) => {
      if (preset.id === presetId) {
        return {
          ...preset,
          activations,
        };
      }
      return preset;
    });

    const updatedConfig = {
      ...config,
      presets: updatedPresets,
    };

    try {
      await invoke("save_config", { config: updatedConfig });
      setConfig(updatedConfig);
      addToast(t("presets.captureSuccess"), "success");
    } catch (err) {
      addToast(err instanceof Error ? err.message : t("settings.saveFailed"), "error");
    }
  }

  // Toggle skill in preset
  async function handleToggleSkill(toolId: string, skillId: string, enabled: boolean) {
    if (!config || !selectedPresetId) return;

    const updatedPresets = presetsList.map((preset) => {
      if (preset.id !== selectedPresetId) return preset;

      const activations = [...preset.activations];
      const toolActIndex = activations.findIndex((a) => a.tool_id === toolId);

      if (toolActIndex > -1) {
        const toolAct = activations[toolActIndex];
        let skillIds = [...toolAct.skill_ids];

        if (enabled) {
          if (!skillIds.includes(skillId)) {
            skillIds.push(skillId);
          }
        } else {
          skillIds = skillIds.filter((id) => id !== skillId);
        }

        activations[toolActIndex] = {
          ...toolAct,
          skill_ids: skillIds,
        };
      } else {
        // Create new tool activation
        if (enabled) {
          activations.push({
            tool_id: toolId,
            skill_ids: [skillId],
          });
        }
      }

      return {
        ...preset,
        activations,
      };
    });

    const updatedConfig = {
      ...config,
      presets: updatedPresets,
    };

    try {
      setConfig(updatedConfig);
      // Auto-save changes to the preset mappings
      await invoke("save_config", { config: updatedConfig });
    } catch (err) {
      addToast(err instanceof Error ? err.message : t("settings.saveFailed"), "error");
    }
  }

  // Select all skills for a tool
  async function handleSelectAllForTool(toolId: string, selectAll: boolean) {
    if (!config || !selectedPresetId) return;

    const updatedPresets = presetsList.map((preset) => {
      if (preset.id !== selectedPresetId) return preset;

      const activations = [...preset.activations];
      const toolActIndex = activations.findIndex((a) => a.tool_id === toolId);
      const allSkillIds = selectAll ? skills.map((s) => s.instance_id) : [];

      if (toolActIndex > -1) {
        activations[toolActIndex] = {
          tool_id: toolId,
          skill_ids: allSkillIds,
        };
      } else {
        activations.push({
          tool_id: toolId,
          skill_ids: allSkillIds,
        });
      }

      return {
        ...preset,
        activations,
      };
    });

    const updatedConfig = {
      ...config,
      presets: updatedPresets,
    };

    try {
      setConfig(updatedConfig);
      await invoke("save_config", { config: updatedConfig });
    } catch (err) {
      addToast(err instanceof Error ? err.message : t("settings.saveFailed"), "error");
    }
  }

  // Apply preset to system
  async function handleApplyPreset(presetId: string) {
    setApplyingPresetId(presetId);
    try {
      await invoke("apply_preset", { presetId });
      
      // Update config reference
      if (config) {
        const updatedConfig = {
          ...config,
          active_preset_id: presetId,
        };
        setConfig(updatedConfig);
      }

      addToast(
        t("presets.applySuccess").replace("{name}", selectedPreset?.name || ""),
        "success"
      );
    } catch (err) {
      console.error(err);
      addToast(t("presets.applyFailed"), "error");
    } finally {
      setApplyingPresetId(null);
    }
  }

  // Helper: check if a skill is active in a preset
  function isSkillActiveInPreset(toolId: string, skillId: string): boolean {
    if (!selectedPreset) return false;
    const toolAct = selectedPreset.activations.find((a) => a.tool_id === toolId);
    return toolAct ? toolAct.skill_ids.includes(skillId) : false;
  }

  // Helper: count selected skills in preset for a tool
  function getSelectedSkillsCountForTool(toolId: string): number {
    if (!selectedPreset) return 0;
    const toolAct = selectedPreset.activations.find((a) => a.tool_id === toolId);
    return toolAct ? toolAct.skill_ids.length : 0;
  }

  return (
    <div style={{
      flex: 1,
      display: 'flex',
      flexDirection: 'column',
      height: '100%',
      overflow: 'hidden',
      backgroundColor: 'var(--background)',
    }}>
      <PageHeader title={t("presets.title")} />

      <div className="flex flex-1 min-w-0 min-h-0 overflow-hidden">
        {/* Preset List Panel (Left) */}
      <div
        className="w-60 flex-shrink-0 border-r border-border bg-card/30 flex flex-col h-full min-h-0"
        style={{
          borderRight: "1px solid var(--border)",
        }}
      >
        <div className="p-3 border-b border-border flex justify-between items-center">
          <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            {t("presets.title")}
          </span>
          <Button
            size="sm"
            variant="ghost"
            className="h-7 w-7 p-0 rounded-md"
            onClick={() => setIsNewPresetDialogOpen(true)}
            title={t("presets.newPreset")}
          >
            <Plus size={16} />
          </Button>
        </div>

        {isNewPresetDialogOpen && (
          <div className="p-3 border-b border-border bg-muted/30 space-y-2 animate-slide-down">
            <Input
              value={newPresetName}
              onChange={(e) => setNewPresetName(e.target.value)}
              placeholder={t("presets.presetNamePlaceholder")}
              className="text-xs h-8"
            />
            <Input
              value={newPresetDesc}
              onChange={(e) => setNewPresetDesc(e.target.value)}
              placeholder={t("presets.presetDescPlaceholder")}
              className="text-xs h-8"
            />
            <div className="flex items-center justify-between py-1 px-1">
              <span className="text-[10px] text-muted-foreground font-medium">
                {t("presets.copyCurrentActive")}
              </span>
              <Switch
                checked={copyCurrentState}
                onCheckedChange={setCopyCurrentState}
              />
            </div>
            <div className="flex justify-end gap-2">
              <Button
                size="sm"
                variant="ghost"
                className="h-7 text-xs px-2"
                onClick={() => {
                  setIsNewPresetDialogOpen(false);
                  setNewPresetName("");
                  setNewPresetDesc("");
                }}
              >
                {t("common.cancel")}
              </Button>
              <Button
                size="sm"
                className="h-7 text-xs px-2"
                onClick={handleCreatePreset}
              >
                {t("common.add")}
              </Button>
            </div>
          </div>
        )}

        <ScrollArea className="flex-1 min-h-0">
          <div className="p-2 space-y-1">
            {presetsList.map((preset) => {
              const isActive = config?.active_preset_id === preset.id;
              const isSelected = selectedPresetId === preset.id;
              return (
                <button
                  key={preset.id}
                  onClick={() => setSelectedPresetId(preset.id)}
                  className={`w-full text-left p-3 rounded-md transition-colors flex flex-col gap-1 ${
                    isSelected
                      ? "bg-secondary text-foreground"
                      : "hover:bg-secondary/40 text-muted-foreground hover:text-foreground"
                  }`}
                >
                  <div className="flex items-center justify-between w-full">
                    <span className="font-semibold text-xs truncate flex items-center gap-1.5">
                      {isActive && <span className="text-ember" style={{ color: "var(--ember)" }}>✦</span>}
                      {preset.name}
                    </span>
                    {isActive && (
                      <span className="text-[10px] bg-ember/20 text-ember px-1.5 py-0.5 rounded font-mono font-medium">
                        ACTIVE
                      </span>
                    )}
                  </div>
                  {preset.description && (
                    <span className="text-[10px] truncate opacity-70">
                      {preset.description}
                    </span>
                  )}
                </button>
              );
            })}

            {presetsList.length === 0 && (
              <div className="text-center p-6 space-y-2">
                <Sliders size={24} className="mx-auto text-muted-foreground/50" />
                <p className="text-[11px] text-muted-foreground">{t("presets.noPresets")}</p>
              </div>
            )}
          </div>
        </ScrollArea>
      </div>

      {/* Preset Detail Configuration Panel (Right) */}
      <div className="flex-1 flex flex-col min-w-0 h-full min-h-0 bg-background/50">
        {selectedPreset ? (
          <div className="flex-1 flex flex-col min-h-0">
            {/* Header info */}
            <div className="p-6 border-b border-border flex justify-between items-start flex-shrink-0">
              <div className="space-y-1 flex-1 min-w-0 pr-4">
                <div className="flex items-center gap-2">
                  <h2 className="text-lg font-bold text-foreground truncate">
                    {selectedPreset.name}
                  </h2>
                  {config?.active_preset_id === selectedPreset.id && (
                    <span className="text-xs bg-ember/15 text-ember border border-ember/30 px-2 py-0.5 rounded font-mono">
                      {t("presets.activePreset")}
                    </span>
                  )}
                </div>
                <p className="text-xs text-muted-foreground truncate">
                  {selectedPreset.description || t("skills.noDescription")}
                </p>
              </div>

              <div className="flex items-center gap-2 flex-shrink-0">
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => handleCaptureCurrentToPreset(selectedPreset.id)}
                  className="h-8 text-xs"
                >
                  <Download size={14} />
                  <span>{t("presets.captureCurrent")}</span>
                </Button>

                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => handleDeletePreset(selectedPreset.id)}
                  className="h-8 text-xs border-destructive/25 text-destructive hover:bg-destructive/10 hover:text-destructive"
                >
                  <Trash2 size={14} />
                  <span>{t("common.delete")}</span>
                </Button>

                <Button
                  size="sm"
                  disabled={applyingPresetId !== null}
                  onClick={() => handleApplyPreset(selectedPreset.id)}
                  className="h-8 text-xs bg-ash text-black hover:bg-ash/90 shadow-sm"
                  style={{
                    backgroundColor: "var(--ash)",
                    color: "#2F3031",
                  }}
                >
                  {applyingPresetId === selectedPreset.id ? (
                    t("presets.applying")
                  ) : config?.active_preset_id === selectedPreset.id ? (
                    <>
                      <Check size={14} />
                      <span>{t("presets.applied")}</span>
                    </>
                  ) : (
                    <>
                      <Play size={14} />
                      <span>{t("presets.apply")}</span>
                    </>
                  )}
                </Button>
              </div>
            </div>

            {/* Description & Explicit Deactivation note */}
            <div className="px-6 py-3 bg-muted/20 border-b border-border flex items-center gap-2 text-[11px] text-muted-foreground">
              <AlertTriangle size={13} className="text-amber" />
              <span>{t("presets.description")}</span>
            </div>

            {/* Skills selection per tool */}
            <ScrollArea className="flex-1 min-h-0">
              <div className="p-6 space-y-6 w-full max-w-full overflow-hidden">
                {tools.map((tool) => {
                  const selectedCount = getSelectedSkillsCountForTool(tool.id);
                  const isAllSelected = selectedCount === skills.length && skills.length > 0;
                  return (
                    <Card
                      key={tool.id}
                      className="border border-border bg-card/40 w-full max-w-full overflow-hidden"
                    >
                      <CardHeader className="p-4 pb-2 flex flex-row items-center justify-between space-y-0">
                        <div>
                          <CardTitle className="text-sm font-semibold flex items-center gap-2">
                            {tool.name}
                          </CardTitle>
                          <CardDescription className="text-[10px]">
                            {t("presets.skillsSelected").replace("{count}", String(selectedCount))}
                          </CardDescription>
                        </div>

                        <div className="flex gap-2">
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-7 text-[10px] px-2"
                            onClick={() => handleSelectAllForTool(tool.id, !isAllSelected)}
                          >
                            {isAllSelected ? t("welcome.selectNone") : t("welcome.selectAll")}
                          </Button>
                        </div>
                      </CardHeader>
                      <CardContent className="p-4 pt-0">
                        {skills.length > 0 ? (
                          <div className="mt-3 grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
                            {skills.map((skill) => {
                              const isActive = isSkillActiveInPreset(tool.id, skill.instance_id);
                              return (
                                <div
                                  key={skill.instance_id}
                                  className="flex items-center justify-between p-3 border border-border rounded-md bg-background/30 hover:bg-secondary/20 transition-colors"
                                >
                                  <div className="flex flex-col min-w-0 pr-3">
                                    <span className="text-xs font-medium text-foreground truncate" title={skill.name}>
                                      {skill.name}
                                    </span>
                                    {skill.description && (
                                      <span className="text-[10px] text-muted-foreground truncate" title={skill.description}>
                                        {skill.description}
                                      </span>
                                    )}
                                  </div>
                                  <Switch
                                    checked={isActive}
                                    onCheckedChange={(checked) =>
                                      handleToggleSkill(tool.id, skill.instance_id, checked)
                                    }
                                    className="flex-shrink-0"
                                  />
                                </div>
                              );
                            })}
                          </div>
                        ) : (
                          <div className="text-center py-6 text-xs text-muted-foreground">
                            {t("skills.noSkills")}
                          </div>
                        )}
                      </CardContent>
                    </Card>
                  );
                })}

                {tools.length === 0 && (
                  <div className="text-center py-12 space-y-3">
                    <Layers size={36} className="mx-auto text-muted-foreground/30" />
                    <h3 className="text-sm font-semibold">{t("tools.noTools")}</h3>
                    <p className="text-xs text-muted-foreground">{t("tools.noToolsDesc")}</p>
                  </div>
                )}
              </div>
            </ScrollArea>
          </div>
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center p-12 space-y-3 text-center">
            <Sliders size={48} className="text-muted-foreground/30 animate-pulse" />
            <h3 className="text-sm font-semibold text-foreground">{t("presets.noPresets")}</h3>
            <p className="text-xs text-muted-foreground max-w-sm">
              {t("presets.noPresetsDesc")}
            </p>
            <Button
              size="sm"
              onClick={() => setIsNewPresetDialogOpen(true)}
              className="mt-2 text-xs"
            >
              <Plus size={14} />
              <span>{t("presets.newPreset")}</span>
            </Button>
          </div>
        )}
      </div>
    </div>
  </div>
  );
}
