🌐 [English](PLATFORM-SUPPORT.md) | **繁體中文**

# 平台支援

最後更新：2026-08-08

本文件的目的，是讓「這個平台有安裝器」與「這個平台上 VPN 真的能用」之間的差異永遠不會被混淆。

## 摘要

| 平台 | CI 是否建置安裝器 | 真實 VPN 通道是否可用 | 狀態 |
| --- | --- | --- | --- |
| **Windows** | ✅ `.msi`、NSIS `.exe` | ✅ 可用（Wintun） | 完整支援 |
| **Linux** | ✅ `.deb`、`.AppImage` | ❌ 尚未支援 | **僅為介面預覽版** |
| **macOS** | ✅ `.dmg` | ❌ 尚未支援 | **僅為介面預覽版** |

## 原因

虛擬網路介面卡——真正負責建立通道介面並傳輸流量的元件——目前僅透過
[Wintun](https://www.wintun.net/) 針對 Windows 實作。詳見
[`src-tauri/src/engine/tun/mod.rs`](src-tauri/src/engine/tun/mod.rs)：真實介面卡的程式路徑僅在
`#[cfg(windows)]` 條件下編譯，non-Windows 分支則是一個回報「不支援此操作」的替身（stub），而非真正執行任何動作。

其餘部分——Rust 引擎的加密機制、NAT 穿透邏輯、信令、前向錯誤更正（FEC）、分流通道政策，以及整個 React／Tauri 介面——皆與平台無關，確實可在 Linux 與 macOS 上建置並執行。這正是 CI 能夠在不謊報實際可編譯內容的前提下，為三個平台皆產出安裝器的*原因*。這也是為什麼這些安裝器被明確標示為預覽版：應用程式能開啟、介面能運作，但由於這些平台上目前尚無真實的介面卡存在，連接對等節點並傳輸通道流量的功能將無法運作。

## 「僅為介面預覽版」在 Linux／macOS 上實際代表什麼

- 應用程式能正常啟動，所有畫面（網路、診斷、設定、Minecraft）皆能正常顯示。
- 交握、信令與遙測相關程式碼路徑仍會執行，但由於沒有真實介面卡可供附接，因此沒有實際的通道，也沒有真實流量傳輸。
- 請勿依賴 Linux 或 macOS 版本進行實際的遊戲／VPN 用途。

## 未來規劃

跨平台 TUN 支援（Linux 的 `/dev/net/tun`、macOS 的 `utun`）目前尚未排入時程。當該功能完成後，本文件與 CI 工作流程中的平台支援聲明將隨之更新，上方表格內容亦會相應變更。進度請追蹤
[Wiki：專案狀態](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Project-Status-zh-Hant)
頁面。

## 此規則於何處被強制執行

由 [`.github/workflows/release.yml`](.github/workflows/release.yml) 發布的每一個 GitHub Release，都會自動在其發布說明中包含上述平台支援聲明（根據本文件的摘要內容自動產生）——因此這項警語會隨著下載內容一同出現，而不只是存在於文件之中。
