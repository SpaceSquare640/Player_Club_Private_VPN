import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import SettingsOverlay from "./SettingsOverlay";
import { useAppStore } from "../../stores/appStore";
import { DEFAULT_THEME } from "../../theme/themes";
import { DEFAULT_CONNECTION_SETTINGS } from "../../types/telemetry";
import i18n from "../../i18n";

const { saveMock, openMock } = vi.hoisted(() => ({
  saveMock: vi.fn(),
  openMock: vi.fn(),
}));
const { writeTextFileMock, readTextFileMock } = vi.hoisted(() => ({
  writeTextFileMock: vi.fn(),
  readTextFileMock: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: saveMock,
  open: openMock,
}));
vi.mock("@tauri-apps/plugin-fs", () => ({
  writeTextFile: writeTextFileMock,
  readTextFile: readTextFileMock,
}));

beforeEach(() => {
  localStorage.clear();
  saveMock.mockReset();
  openMock.mockReset();
  writeTextFileMock.mockReset();
  readTextFileMock.mockReset();
  useAppStore.setState({
    activeRoute: "dashboard",
    theme: DEFAULT_THEME,
    settingsOpen: true,
    language: "en",
    // The Connection section (Basic/Expert layering) is opt-in and defaults to
    // hidden; most tests in this file are about that section's own content, so
    // it is enabled here by default and the visibility behaviour itself gets
    // its own describe block below with an explicit override.
    expertMode: true,
    forwardBroadcast: DEFAULT_CONNECTION_SETTINGS.forwardBroadcast,
    forwardMulticast: DEFAULT_CONNECTION_SETTINGS.forwardMulticast,
    fecParityShards: DEFAULT_CONNECTION_SETTINGS.fecParityShards,
  });
});

describe("SettingsOverlay — Connection section (Phase B.3)", () => {
  it("renders the connection settings with the historical defaults", () => {
    render(<SettingsOverlay />);
    expect(screen.getByTestId("settings-connection")).toBeInTheDocument();
    expect(screen.getByTestId("settings-forward-broadcast")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByTestId("settings-forward-multicast")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByTestId("settings-fec-1")).toHaveAttribute("aria-pressed", "true");
  });

  it("toggling forward broadcast updates the store", () => {
    render(<SettingsOverlay />);
    fireEvent.click(screen.getByTestId("settings-forward-broadcast"));
    expect(useAppStore.getState().forwardBroadcast).toBe(false);
    expect(screen.getByTestId("settings-forward-broadcast")).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  it("toggling forward multicast updates the store independently", () => {
    render(<SettingsOverlay />);
    fireEvent.click(screen.getByTestId("settings-forward-multicast"));
    expect(useAppStore.getState().forwardMulticast).toBe(false);
    expect(useAppStore.getState().forwardBroadcast).toBe(true); // unaffected
  });

  it("selecting an FEC redundancy option updates the store", () => {
    render(<SettingsOverlay />);
    fireEvent.click(screen.getByTestId("settings-fec-2"));
    expect(useAppStore.getState().fecParityShards).toBe(2);
    expect(screen.getByTestId("settings-fec-2")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByTestId("settings-fec-1")).toHaveAttribute("aria-pressed", "false");
  });
});

describe("SettingsOverlay — language switching (i18n)", () => {
  it("selecting a language option updates the store", () => {
    render(<SettingsOverlay />);
    fireEvent.click(screen.getByTestId("language-zh-Hant"));
    expect(useAppStore.getState().language).toBe("zh-Hant");
    expect(screen.getByTestId("language-zh-Hant")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByTestId("language-en")).toHaveAttribute("aria-pressed", "false");
  });

  it("offers Simplified Chinese as a third language option", () => {
    render(<SettingsOverlay />);
    fireEvent.click(screen.getByTestId("language-zh-Hans"));
    expect(useAppStore.getState().language).toBe("zh-Hans");
    expect(screen.getByTestId("language-zh-Hans")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByTestId("language-zh-Hant")).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByTestId("language-en")).toHaveAttribute("aria-pressed", "false");
  });

  // The real proof the plumbing works: when i18next's active language changes
  // (which is what AppShell's store-sync effect does), every already-mounted
  // component using useTranslation() re-renders in the new language — no
  // remount, no store-only assertion standing in for the visible behaviour.
  it("re-renders visible copy when i18next's active language changes", async () => {
    render(<SettingsOverlay />);
    expect(screen.getByText("Theme")).toBeInTheDocument();
    expect(screen.getByText("Forward broadcast")).toBeInTheDocument();

    await i18n.changeLanguage("zh-Hant");

    expect(screen.getByText("主題")).toBeInTheDocument();
    expect(screen.getByText("轉發廣播")).toBeInTheDocument();
    expect(screen.queryByText("Theme")).not.toBeInTheDocument();

    await i18n.changeLanguage("en"); // restore for subsequent tests/files
  });
});

describe("SettingsOverlay — Basic/Expert layering", () => {
  it("hides the Connection section by default (expertMode: false)", () => {
    useAppStore.setState({ expertMode: false });
    render(<SettingsOverlay />);
    expect(screen.queryByTestId("settings-connection")).not.toBeInTheDocument();
    expect(screen.getByTestId("settings-expert-mode")).toHaveAttribute("aria-pressed", "false");
    // Basic personalization is never gated — it isn't an "expert" concern.
    expect(screen.getByTestId("settings-language")).toBeInTheDocument();
  });

  it("reveals the Connection section when Expert mode is switched on", () => {
    useAppStore.setState({ expertMode: false });
    render(<SettingsOverlay />);
    expect(screen.queryByTestId("settings-connection")).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("settings-expert-mode"));

    expect(useAppStore.getState().expertMode).toBe(true);
    expect(screen.getByTestId("settings-connection")).toBeInTheDocument();
  });

  // The core guarantee: hiding the section is display-only. A setting changed
  // while Expert mode was on must still be in effect after switching it back
  // off — toggling the section's visibility must never reset or ignore it.
  it("keeps a changed setting in effect after the section is hidden again", () => {
    useAppStore.setState({ expertMode: true });
    render(<SettingsOverlay />);

    fireEvent.click(screen.getByTestId("settings-fec-2"));
    expect(useAppStore.getState().fecParityShards).toBe(2);

    fireEvent.click(screen.getByTestId("settings-expert-mode"));
    expect(screen.queryByTestId("settings-connection")).not.toBeInTheDocument();
    expect(useAppStore.getState().fecParityShards).toBe(2); // unchanged by hiding it
  });
});

describe("SettingsOverlay — JSON profile import/export", () => {
  beforeEach(() => {
    useAppStore.setState({ expertMode: true });
  });

  it("exports the current connection settings to the chosen path", async () => {
    saveMock.mockResolvedValue("C:/fake/profile.json");
    render(<SettingsOverlay />);

    fireEvent.click(screen.getByTestId("settings-export-profile"));

    await waitFor(() => expect(writeTextFileMock).toHaveBeenCalledTimes(1));
    const [path, json] = writeTextFileMock.mock.calls[0];
    expect(path).toBe("C:/fake/profile.json");
    expect(JSON.parse(json)).toEqual({
      formatVersion: 1,
      forwardBroadcast: true,
      forwardMulticast: true,
      fecParityShards: 1,
    });
  });

  it("does not write anything if the save dialog is cancelled", async () => {
    saveMock.mockResolvedValue(null);
    render(<SettingsOverlay />);

    fireEvent.click(screen.getByTestId("settings-export-profile"));

    await waitFor(() => expect(saveMock).toHaveBeenCalledTimes(1));
    expect(writeTextFileMock).not.toHaveBeenCalled();
  });

  it("imports a valid profile and applies it to the store", async () => {
    openMock.mockResolvedValue("C:/fake/profile.json");
    readTextFileMock.mockResolvedValue(
      JSON.stringify({
        formatVersion: 1,
        forwardBroadcast: false,
        forwardMulticast: false,
        fecParityShards: 3,
      }),
    );
    render(<SettingsOverlay />);

    fireEvent.click(screen.getByTestId("settings-import-profile"));

    await waitFor(() => expect(useAppStore.getState().fecParityShards).toBe(3));
    expect(useAppStore.getState().forwardBroadcast).toBe(false);
    expect(useAppStore.getState().forwardMulticast).toBe(false);
    expect(screen.queryByTestId("settings-import-error")).not.toBeInTheDocument();
  });

  it("shows an inline error and leaves the store untouched for a malformed profile", async () => {
    openMock.mockResolvedValue("C:/fake/profile.json");
    readTextFileMock.mockResolvedValue(JSON.stringify({ formatVersion: 99 }));
    render(<SettingsOverlay />);

    fireEvent.click(screen.getByTestId("settings-import-profile"));

    await waitFor(() => expect(screen.getByTestId("settings-import-error")).toBeInTheDocument());
    expect(useAppStore.getState().fecParityShards).toBe(DEFAULT_CONNECTION_SETTINGS.fecParityShards);
  });

  it("does not read anything if the open dialog is cancelled", async () => {
    openMock.mockResolvedValue(null);
    render(<SettingsOverlay />);

    fireEvent.click(screen.getByTestId("settings-import-profile"));

    await waitFor(() => expect(openMock).toHaveBeenCalledTimes(1));
    expect(readTextFileMock).not.toHaveBeenCalled();
  });
});
