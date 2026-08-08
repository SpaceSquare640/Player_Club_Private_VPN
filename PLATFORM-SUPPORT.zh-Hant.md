🌐 [English](PLATFORM-SUPPORT.md) | **繁體中文**

# 平台支援

最後更新：2026-08-09

本文件的目的，是讓「這個平台有安裝器」與「這個平台上 VPN 真的能用」之間的差異永遠不會被混淆。

## 摘要

| 平台 | CI 是否建置安裝器 | 真實 VPN 通道 | 狀態 |
| --- | --- | --- | --- |
| **Windows** | ✅ `.msi`、NSIS `.exe` | ✅ 已實作（Wintun） | 完整支援 |
| **Linux** | ✅ `.deb`、`.AppImage` | 🚧 已實作，**尚未於實機驗證**（`/dev/net/tun`） | 預覽版 |
| **macOS** | ✅ `.dmg` | 🚧 已實作，**尚未於實機驗證**（`utun`） | 預覽版 |

## 為什麼「已實作」不等於「已驗證」

虛擬網路介面卡——真正負責建立通道介面並傳輸流量的元件——現在三個平台都有真實的實作：Windows 使用 Wintun，Linux 使用 `/dev/net/tun`，macOS 使用 `utun` 核心控制通訊端（kernel-control socket）。詳見
[`src-tauri/src/engine/tun/`](src-tauri/src/engine/tun/) 底下的
`windows.rs`、`linux.rs`、`macos.rs`，於 `mod.rs` 中依編譯目標選擇對應實作。

Windows 實作已經過端對端驗證（見
[Wiki：專案狀態](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Project-Status-zh-Hant)）。Linux 與 macOS 實作是全新的：它們在 CI 的 `ubuntu-latest` 與 `macos-latest` 上皆可編譯，並通過各自的單元測試（ioctl／結構體記憶體佈局的數學運算、介面名稱封裝等），但**尚未有人在真實的 Linux 或 macOS 硬體上、對真實對等節點驗證過**——目前沒有人確認過介面卡真的能建立可運作的介面、傳輸流量，或在真實世界的權限／防火牆設定下正常運作。在此驗證完成之前，請將其視為預覽版而非保證可用——進度追蹤請見
[Wiki：專案狀態](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Project-Status-zh-Hant)。

另外還有兩項與 Linux／macOS 相關、規模比介面卡本身小、但仍未解決的缺口：

- **沒有一鍵提升權限。** Windows 可透過 UAC 自行以提升權限重新啟動；
  Linux／macOS 沒有對應的單一、跨版本安全的 API（實際可用選項是
  `pkexec`、`sudo`，或平台專屬的驗證對話框，沒有一個能直接取代）。在此功能完成之前，這些平台上必須先以 root（`sudo`）方式啟動應用程式，真實介面卡才可用——詳見
  [`src-tauri/src/engine/tun/privilege.rs`](src-tauri/src/engine/tun/privilege.rs)。
- **僅 Windows 才有的作業系統整合細節** — 自動將介面卡所在網路歸類為私人網路，並限定防火牆允許規則範圍（`src-tauri/src/engine/tun/windows.rs` 的
  `configure_network_integration`）在 Linux／macOS 上尚無對應實作。流量仍可正常傳輸；但在這些平台上您可能需要自行手動調整防火牆設定。

## Linux／macOS 上實際會遇到什麼情況

- 應用程式能正常啟動，所有畫面（網路、診斷、設定、Minecraft）皆能正常顯示，與 Windows 相同。
- 若想嘗試真實介面卡，請以 `sudo` 啟動應用程式（或以其他方式讓真實介面卡路徑以 root 身分執行）——若未以 root 執行，會如同 Windows 一樣回報「需要管理員權限」，但這些平台目前尚無一鍵重新啟動按鈕。
- 介面卡建立、IP 位址指派、封包讀寫路徑皆為真實程式碼，而非替身（stub）——但再次強調，**尚未於實體硬體上驗證**。若您嘗試後無法運作，目前這是預期中的狀況，不一定代表其他地方壞掉；無論如何都歡迎回報您觀察到的情況（安全性相關回報請見
  [`SECURITY.zh-Hant.md`](SECURITY.zh-Hant.md)，其他問題請至
  [Issues](https://github.com/SpaceSquare640/Player_Club_Private_VPN/issues)）。

## 未來規劃

依大致優先順序：在真實硬體上對真實對等節點驗證 Linux 與 macOS 介面卡；為兩者加入一鍵提升權限；讓作業系統網路整合（防火牆／路由管理）追上 Windows 的水準。每完成一項，本文件與 CI 發布說明中的聲明都會隨之更新，上方表格內容也會相應變更。進度請追蹤
[Wiki：專案狀態](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Project-Status-zh-Hant)
頁面。

## 此規則於何處被強制執行

由 [`.github/workflows/release.yml`](.github/workflows/release.yml) 發布的每一個 GitHub Release，都會自動在其發布說明中包含上述平台支援聲明（根據本文件的摘要內容自動產生）——因此這項警語會隨著下載內容一同出現，而不只是存在於文件之中。
