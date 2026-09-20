import type { Tool } from "@/types";

export interface ProjectToolGroup {
  relativePath: string;
  tools: Tool[];
}

export function groupProjectToolsByDirectory(tools: Tool[]): ProjectToolGroup[] {
  const groups = new Map<string, Tool[]>();
  for (const tool of tools) {
    const relativePath = tool.project_skills_dir?.replace(/^[/\\]+/, "");
    if (!tool.config.enabled || !relativePath) continue;
    groups.set(relativePath, [...(groups.get(relativePath) ?? []), tool]);
  }
  return Array.from(groups, ([relativePath, groupedTools]) => ({
    relativePath,
    tools: groupedTools,
  }));
}

export function toggleProjectToolGroupSelection(
  selectedToolIds: string[],
  groupToolIds: string[],
): string[] {
  const selected = new Set(selectedToolIds);
  const groupSelected = groupToolIds.some((toolId) => selected.has(toolId));
  for (const toolId of groupToolIds) {
    if (groupSelected) {
      selected.delete(toolId);
    } else {
      selected.add(toolId);
    }
  }
  return Array.from(selected);
}
