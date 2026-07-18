import type { Tool } from "../../types/index.ts";

type ToolLike = Pick<Tool, "id" | "config">;

export function getDetectedToolIds(tools: ToolLike[]): string[] {
  return tools
    .filter((tool) => tool.config.detected)
    .map((tool) => tool.id)
    .sort();
}
