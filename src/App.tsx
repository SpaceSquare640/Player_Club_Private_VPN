import { RouterProvider } from "react-router-dom";
import { router } from "./app/router";
import { ThemeProvider } from "./theme/ThemeProvider";

/**
 * Application root: wires the theme engine around the hash router, which renders
 * the AppShell (sidebar + breadcrumb + content) and its routed pages.
 */
export default function App() {
  return (
    <ThemeProvider>
      <RouterProvider router={router} />
    </ThemeProvider>
  );
}
