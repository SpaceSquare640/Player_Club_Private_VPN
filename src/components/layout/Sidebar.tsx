import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import {
  Activity,
  LayoutDashboard,
  Network as NetworkIcon,
  Settings,
  type LucideIcon,
} from "lucide-react";
import { useAppStore, type RouteId } from "../../stores/appStore";
import { cn } from "../../lib/cn";

interface NavItem {
  id: RouteId;
  labelKey: string;
  path: string;
  Icon: LucideIcon;
}

const NAV_ITEMS: NavItem[] = [
  { id: "dashboard", labelKey: "nav.dashboard", path: "/", Icon: LayoutDashboard },
  { id: "network", labelKey: "nav.network", path: "/network", Icon: NetworkIcon },
  { id: "diagnostics", labelKey: "nav.diagnostics", path: "/diagnostics", Icon: Activity },
];

/** Fixed 60px icon rail. Active item glows violet (semantic "active"). */
export default function Sidebar() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const activeRoute = useAppStore((s) => s.activeRoute);
  const toggleSettings = useAppStore((s) => s.toggleSettings);

  return (
    <nav className="flex h-full w-[60px] flex-col items-center gap-1 border-r border-white/5 bg-surface-2 py-3">
      <div className="mb-3 flex h-9 w-9 items-center justify-center rounded-xl bg-brand-violet/20 text-xs font-bold text-brand-violet">
        PC
      </div>

      {NAV_ITEMS.map(({ id, labelKey, path, Icon }) => {
        const active = activeRoute === id;
        const label = t(labelKey);
        return (
          <button
            key={id}
            type="button"
            title={label}
            aria-label={label}
            aria-current={active ? "page" : undefined}
            data-testid={`nav-${id}`}
            data-active={active}
            onClick={() => navigate(path)}
            className={cn(
              "relative flex h-10 w-10 items-center justify-center rounded-xl transition-colors",
              active
                ? "bg-brand-violet/15 text-brand-violet"
                : "text-ink-muted hover:bg-white/5 hover:text-ink",
            )}
          >
            {active && (
              <span className="absolute left-[-10px] h-6 w-1 rounded-r bg-brand-violet" />
            )}
            <Icon size={20} />
          </button>
        );
      })}

      <button
        type="button"
        title={t("nav.settings")}
        aria-label={t("nav.settings")}
        data-testid="nav-settings"
        onClick={() => toggleSettings()}
        className="mt-auto flex h-10 w-10 items-center justify-center rounded-xl text-ink-muted transition-colors hover:bg-white/5 hover:text-ink"
      >
        <Settings size={20} />
      </button>
    </nav>
  );
}
