import { UserPreferences } from "@/types";

export const defaultPreferences: UserPreferences = {
  theme: "system",
  font_family: "default",
  language: "en",
  auto_sync: true,
  default_editor: "system",
  tab_size: 2,
  show_sync_notifications: true,
  remove_links_when_disabling_tool: false,
  skill_usage_monitor: true,
  risk_scan_mode: "off",
  github_token: "",
  clawhub_token: "",
};
