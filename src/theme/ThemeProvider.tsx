import { useEffect, type ReactNode } from "react";
import { useAppStore } from "../stores/appStore";

/**
 * Applies the active theme by setting `data-theme` on the document root.
 * The matching `:root[data-theme="..."]` block in index.css overrides the
 * semantic CSS variables, so all theme-aware utilities update instantly.
 */
export function ThemeProvider({ children }: { children: ReactNode }) {
  const theme = useAppStore((s) => s.theme);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  return <>{children}</>;
}
