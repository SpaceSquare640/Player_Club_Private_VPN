import { lazy } from "react";
import { createHashRouter } from "react-router-dom";
import AppShell from "../components/layout/AppShell";

// Lazy-loaded page stubs — rendered inside AppShell's <Suspense> boundary,
// which falls back to the Skeleton while a chunk loads.
const Dashboard = lazy(() => import("../pages/Dashboard"));
const Network = lazy(() => import("../pages/Network"));
const Diagnostics = lazy(() => import("../pages/Diagnostics"));
const Minecraft = lazy(() => import("../pages/Minecraft"));

export const router = createHashRouter([
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <Dashboard /> },
      { path: "network", element: <Network /> },
      { path: "diagnostics", element: <Diagnostics /> },
      { path: "minecraft", element: <Minecraft /> },
    ],
  },
]);
