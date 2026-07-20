import { ChevronRight } from "lucide-react";
import { useAppStore, type RouteId } from "../../stores/appStore";

const LABELS: Record<RouteId, string> = {
  dashboard: "Dashboard",
  network: "Network",
  diagnostics: "Diagnostics",
};

/** Breadcrumb trail driven by the store's active route. */
export default function Breadcrumb() {
  const activeRoute = useAppStore((s) => s.activeRoute);

  return (
    <header className="flex h-12 shrink-0 items-center gap-2 border-b border-white/5 px-6 text-sm">
      <span className="text-ink-muted">Player Club</span>
      <ChevronRight size={14} className="text-ink-muted/60" />
      <span data-testid="breadcrumb-current" className="font-medium text-ink">
        {LABELS[activeRoute]}
      </span>
    </header>
  );
}
