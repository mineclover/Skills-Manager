import { ProjectBinding } from "@/types";
import { useTranslation } from "@/i18n";

interface ScopeSelectorProps {
  projects: ProjectBinding[];
  value: string | null;
  onChange: (projectId: string | null) => void;
  label?: string;
  disabled?: boolean;
  className?: string;
}

/**
 * The scope selector is deliberately explicit: an empty value always means
 * the global scope, while a project is selected by its stable binding id.
 * Pages should not silently follow AppConfig.active_project_id.
 */
export function ScopeSelector({
  projects,
  value,
  onChange,
  label,
  disabled = false,
  className = "",
}: ScopeSelectorProps) {
  const { t } = useTranslation();

  return (
    <label className={`inline-flex items-center gap-2 min-w-0 ${className}`}>
      {label && (
        <span className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground whitespace-nowrap">
          {label}
        </span>
      )}
      <select
        aria-label={label ?? t("scope.selectScope")}
        value={value ?? ""}
        onChange={(event) => onChange(event.target.value || null)}
        disabled={disabled}
        className="h-8 min-w-[180px] max-w-[300px] rounded-md border border-border bg-background px-2 text-[11px] text-foreground outline-none transition-colors focus:border-ring disabled:cursor-not-allowed disabled:opacity-60"
      >
        <option value="">{t("skills.scopeGlobal")}</option>
        {projects.map((project) => (
          <option key={project.id} value={project.id}>
            {project.name}
          </option>
        ))}
      </select>
    </label>
  );
}
