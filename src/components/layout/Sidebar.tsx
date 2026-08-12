import { NavLink } from "react-router-dom";
import { useTranslation } from "@/i18n";
import { AuthButton } from "@/components/auth/AuthButton";
import { Sparkles, Wrench, Sliders, ShoppingBag, Settings, MessageSquare, Layers3 } from "lucide-react";

export function Sidebar() {
  const { t } = useTranslation();

  const navItems = [
    {
      to: "/",
      label: t("nav.skills"),
      icon: <Sparkles size={16} />,
    },
    {
      to: "/tools",
      label: t("nav.tools"),
      icon: <Wrench size={16} />,
    },
    {
      to: "/presets",
      label: t("nav.presets"),
      icon: <Sliders size={16} />,
    },
    {
      to: "/skill-sets",
      label: "Skill Sets",
      icon: <Layers3 size={16} />,
    },
    {
      to: "/marketplace",
      label: t("nav.marketplace"),
      icon: <ShoppingBag size={16} />,
    },
    {
      to: "/settings",
      label: t("nav.settings"),
      icon: <Settings size={16} />,
    },
    {
      to: "/feedback",
      label: t("nav.feedback"),
      icon: <MessageSquare size={16} />,
    },
  ];

  return (
    <aside
      className="flex flex-col h-screen flex-shrink-0 bg-sidebar border-r border-sidebar-border"
      style={{
        width: 220,
        backgroundColor: "var(--sidebar)",
        borderRight: "1px solid var(--sidebar-border)",
      }}
    >
      {/* Header section with macOS Traffic-light area */}
      <div
        data-tauri-drag-region
        className="flex flex-col justify-end p-4 pb-2 select-none cursor-grab"
        style={{ height: 96 }}
      >
        {/* Brand: Logo & Title */}
        <div className="flex items-center gap-2 px-2 pb-2">
          <span className="text-ember font-semibold text-lg" style={{ color: "var(--ember)" }}>✦</span>
          <span
            className="font-bold text-sm tracking-tight text-foreground"
            style={{
              fontSize: 13,
              fontWeight: 600,
              letterSpacing: "-0.01em",
            }}
          >
            {t("topbar.brand")}
          </span>
        </div>
      </div>

      {/* Navigation Links */}
      <nav className="flex-1 px-3 py-2 space-y-1 overflow-y-auto">
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            className={({ isActive }) =>
              `flex items-center gap-3 px-3 py-2 text-xs font-medium rounded-md transition-colors ${
                isActive
                  ? "bg-sidebar-accent text-sidebar-accent-foreground"
                  : "text-muted-foreground hover:bg-sidebar-accent/50 hover:text-foreground"
              }`
            }
            style={({ isActive }) => ({
              backgroundColor: isActive ? "var(--sidebar-accent)" : "transparent",
              color: isActive ? "var(--foreground)" : "var(--muted-foreground)",
            })}
          >
            <span className="flex-shrink-0 opacity-80">{item.icon}</span>
            <span>{item.label}</span>
          </NavLink>
        ))}
      </nav>

      {/* Footer Profile */}
      <div className="p-4 border-t border-sidebar-border flex items-center justify-between">
        <AuthButton variant="sidebar" />
      </div>
    </aside>
  );
}
