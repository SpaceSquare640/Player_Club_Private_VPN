import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

/**
 * Dashboard page. Also preserves the engine IPC probe (`ping`) introduced at
 * initialization, inside a golden-ratio (φ ≈ 1.618) two-pane grid.
 */
export default function Dashboard() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<string>("idle");

  async function pingEngine() {
    try {
      setStatus(await invoke<string>("ping"));
    } catch (err) {
      setStatus(`error: ${String(err)}`);
    }
  }

  return (
    <div className="space-y-6" data-testid="page-dashboard">
      <div>
        <h1 className="text-2xl font-semibold text-ink">{t("dashboard.title")}</h1>
        <p className="text-sm text-ink-muted">{t("dashboard.subtitle")}</p>
      </div>

      <div className="grid grid-cols-[1.618fr_1fr] gap-6" data-testid="phi-grid">
        <section className="rounded-2xl bg-surface-2 p-6 ring-1 ring-white/5">
          <h2 className="text-sm font-medium text-ink-muted">{t("dashboard.engine")}</h2>
          <button
            type="button"
            data-testid="ping-btn"
            onClick={pingEngine}
            className="mt-4 rounded-lg bg-brand-violet px-4 py-2 text-sm font-medium text-white transition hover:opacity-90 focus:outline-none focus:ring-2 focus:ring-brand-violet/60"
          >
            {t("dashboard.pingEngine")}
          </button>
          <p className="mt-3 font-mono text-xs text-ink-muted">
            engine:{" "}
            <span data-testid="ping-status" className="text-brand-cyan">
              {status}
            </span>
          </p>
        </section>

        <section className="rounded-2xl bg-surface-2 p-6 ring-1 ring-white/5">
          <h2 className="text-sm font-medium text-ink-muted">{t("dashboard.status")}</h2>
          <ul className="mt-3 space-y-2 text-sm">
            <li className="text-brand-cyan">● {t("dashboard.idle")}</li>
            <li className="text-ink-muted">{t("dashboard.peersConnected", { count: 0 })}</li>
          </ul>
        </section>
      </div>
    </div>
  );
}
