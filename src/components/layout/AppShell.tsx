import { Suspense, useEffect, useState } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import Sidebar from "./Sidebar";
import Breadcrumb from "./Breadcrumb";
import SettingsOverlay from "../settings/SettingsOverlay";
import { Skeleton } from "../common/Skeleton";
import { useAppStore, type RouteId } from "../../stores/appStore";
import { useEngineTelemetry } from "../../hooks/useEngineTelemetry";
import { useLiveConnectionSettings } from "../../hooks/useLiveConnectionSettings";
import i18n from "../../i18n";

const ROUTE_PATHS: Record<RouteId, string> = {
  dashboard: "/",
  network: "/network",
  diagnostics: "/diagnostics",
  minecraft: "/minecraft",
};

function routeFromPath(pathname: string): RouteId {
  if (pathname.startsWith("/network")) return "network";
  if (pathname.startsWith("/diagnostics")) return "diagnostics";
  if (pathname.startsWith("/minecraft")) return "minecraft";
  return "dashboard";
}

/**
 * Top-level layout: a golden-ratio CSS grid with the fixed 60px icon rail and a
 * fluid main column (breadcrumb + routed content). Syncs the router location
 * into the store so the sidebar and breadcrumb stay consistent.
 */
export default function AppShell() {
  const location = useLocation();
  const navigate = useNavigate();
  const setActiveRoute = useAppStore((s) => s.setActiveRoute);
  const language = useAppStore((s) => s.language);

  // App-wide telemetry subscription: the engine event streams (state, stats,
  // packets, notices) and identity/privilege must stay live regardless of which
  // page is showing — the Network page drives connections while Diagnostics only
  // observes, so the subscription cannot be tied to either page's lifetime.
  useEngineTelemetry();

  // Same pattern for language: sync the persisted/detected choice into i18next
  // once, here, rather than in every page that happens to render first.
  useEffect(() => {
    void i18n.changeLanguage(language);
  }, [language]);

  // Live settings push (Phase B.4) — app-wide for the same reason as the
  // telemetry subscription above: it must outlive whichever page is showing.
  useLiveConnectionSettings();

  // Captured once at first render — the persisted route, before the location
  // sync effect below reconciles the store to the URL.
  const [restoredRoute] = useState(() => useAppStore.getState().activeRoute);

  // Restore the last-visited page on launch. Only when we land on the default
  // root ("/"); an explicit deep-link to a real path always wins.
  useEffect(() => {
    if (location.pathname === "/" && restoredRoute !== "dashboard") {
      navigate(ROUTE_PATHS[restoredRoute], { replace: true });
    }
    // Run once on mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    setActiveRoute(routeFromPath(location.pathname));
  }, [location.pathname, setActiveRoute]);

  return (
    <div className="grid h-full grid-cols-[60px_1fr] bg-surface text-ink">
      <Sidebar />
      <div className="flex min-w-0 flex-col">
        <Breadcrumb />
        <main className="min-h-0 flex-1 overflow-auto p-6">
          <Suspense fallback={<Skeleton />}>
            <Outlet />
          </Suspense>
        </main>
      </div>
      <SettingsOverlay />
    </div>
  );
}
