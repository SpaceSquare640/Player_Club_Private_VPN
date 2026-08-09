import { useNavigate } from "react-router";
import { useTranslation } from "react-i18next";
import {
  Activity,
  Gamepad2,
  LayoutDashboard,
  Network as NetworkIcon,
  Router,
  Settings,
  type LucideIcon,
} from "lucide-react";
import { useAppStore, type RouteId } from "../../stores/appStore";
import IconButton from "../ui/IconButton";

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
  // Neutral placeholder icon (lucide's Gamepad2) — pending resolution of the
  // Minecraft-branded artwork's licensing before it can ship in this repo.
  { id: "minecraft", labelKey: "nav.minecraft", path: "/minecraft", Icon: Gamepad2 },
  { id: "relayServer", labelKey: "nav.relayServer", path: "/relay-server", Icon: Router },
];

/** Fixed 60px icon rail. Active item marked by a left accent bar + tinted background. */
export default function Sidebar() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const activeRoute = useAppStore((s) => s.activeRoute);
  const toggleSettings = useAppStore((s) => s.toggleSettings);

  return (
    <nav className="flex h-full w-[60px] flex-col items-center gap-1.5 border-r border-white/5 bg-surface-2 py-4">
      <div className="mb-4 flex size-9 items-center justify-center rounded-xl bg-brand-violet/15 text-xs font-bold text-brand-violet ring-1 ring-brand-violet/20">
        PC
      </div>

      {NAV_ITEMS.map(({ id, labelKey, path, Icon }) => {
        const active = activeRoute === id;
        return (
          <IconButton
            key={id}
            label={t(labelKey)}
            active={active}
            data-testid={`nav-${id}`}
            data-active={active}
            aria-current={active ? "page" : undefined}
            onClick={() => navigate(path)}
            className="relative"
          >
            {active && (
              <span className="absolute left-[-12px] h-6 w-1 rounded-r bg-brand-violet" />
            )}
            <Icon size={20} />
          </IconButton>
        );
      })}

      <IconButton
        label={t("nav.settings")}
        data-testid="nav-settings"
        onClick={() => toggleSettings()}
        className="mt-auto"
      >
        <Settings size={20} />
      </IconButton>
    </nav>
  );
}
