🌐 [English](README.md) | **繁體中文**

# Player Club Private VPN

> 高效能**遊戲虛擬網路**。Player Club Private VPN 透過公開網際網路建立安全、低延遲的虛擬區域網路，讓地理位置分散的玩家如同身處同一個子網路——由 **Rust** 網路引擎搭配現代化的 **Tauri + React** 桌面用戶端組成。

[![狀態](https://img.shields.io/badge/status-alpha-22d3ee)](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Project-Status-zh-Hant)
[![引擎](https://img.shields.io/badge/engine-Rust-orange)](#技術架構)
[![介面](https://img.shields.io/badge/UI-Tauri%20%2B%20React%20%2B%20TS-blue)](#技術架構)
[![授權](https://img.shields.io/badge/license-Proprietary-red)](LICENSE.zh-Hant.md)
[![平台](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-0078D6)](PLATFORM-SUPPORT.zh-Hant.md)

---

## 概觀

**Player Club Private VPN**（*PCP-VPN*）在公開網際網路上模擬區域網路（LAN），讓遠端玩家能夠加入同一個虛擬子網路，使原本僅限區域網路的多人遊戲如同大家身處同一個房間般正常運作。連線是直接的、點對點的，並且全程加密——沒有中央中繼站，也沒有代管的後端伺服器。

**目前狀態：搶先版／Alpha。** 完整的點對點資料路徑（交握、加密、NAT 穿透、虛擬介面卡、FEC、分流通道）已可運作，並由自動化測試套件全程覆蓋，但兩台實體機器之間的真實 NAT 穿透尚未經過驗證。完整、逐項的建置狀態請見
**[Wiki：專案狀態](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Project-Status-zh-Hant)**。

## 技術架構

| 層級 | 技術 |
| --- | --- |
| 核心引擎 | Rust |
| 桌面外殼 | Tauri |
| 介面框架 | React + TypeScript |
| 樣式 | Tailwind CSS |
| CI／打包 | GitHub Actions + `tauri-action` |
| 平台 | Windows、Linux、macOS——三平台皆有安裝器；**真實 VPN 通道目前僅支援 Windows** |

> ⚠️ **Linux 與 macOS 版本目前僅為介面預覽版。** 虛擬介面卡程式碼
> （`src-tauri/src/engine/tun/mod.rs`）目前僅在 `#[cfg(windows)]` 下實作，因此
> 該應用程式可在 Linux／macOS 上啟動，但尚無法建立真實通道。詳細說明請見
> [`PLATFORM-SUPPORT.zh-Hant.md`](PLATFORM-SUPPORT.zh-Hant.md)，原因則見
> [Wiki：架構](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Architecture-zh-Hant)。

## 快速開始

```bash
pnpm install
pnpm tauri dev        # 開發模式執行
pnpm tauri build       # 建置正式版與安裝器
```

需要 Rust（MSVC 工具鏈）、Node.js 20+、pnpm 10+，以及 Windows 上的 VS C++ Build
Tools。完整的先決條件、疑難排解與使用教學，請見
**[Wiki：快速上手](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Getting-Started-zh-Hant)**
與
**[Wiki：使用者手冊](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/User-Manual-zh-Hant)**。

每次推送 `v*` 標籤時，CI 會自動建置 Windows、Linux、macOS 三平台安裝器（見
[`.github/workflows/release.yml`](.github/workflows/release.yml)），並發布為
**草稿（draft）** GitHub Release 供審閱——每次的發布說明都會自動根據
`CHANGELOG.md` 產生，並附上下方的平台支援聲明，不需手動撰寫。

## 文件

| 資源 | 位置 |
| --- | --- |
| 完整功能清單與建置狀態 | [Wiki：專案狀態](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Project-Status-zh-Hant)、[Wiki：功能](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Features-zh-Hant) |
| 快速上手與從原始碼建置 | [Wiki：快速上手](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Getting-Started-zh-Hant) |
| **使用者手冊**（如何使用本應用程式） | [Wiki：使用者手冊](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/User-Manual-zh-Hant) |
| 架構與專案結構 | [Wiki：架構](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Architecture-zh-Hant) |
| 常見問題 | [Wiki：FAQ](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/FAQ-zh-Hant) |
| 版本異動記錄 | [`CHANGELOG.md`](./CHANGELOG.md)（僅英文） |
| 平台支援說明 | [`PLATFORM-SUPPORT.zh-Hant.md`](./PLATFORM-SUPPORT.zh-Hant.md) |
| 第三方元件 | [`THIRD-PARTY-NOTICES.md`](./THIRD-PARTY-NOTICES.md)（僅英文） |

## 社群

| 管道 | 用途 |
| --- | --- |
| [Issues](https://github.com/SpaceSquare640/Player_Club_Private_VPN/issues) | 回報錯誤、提出功能請求 |
| [Discussions](https://github.com/SpaceSquare640/Player_Club_Private_VPN/discussions) | 提問、想法交流、一般討論 |
| [Wiki](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki) | 使用指南、手冊與參考資料 |
| [安全性建議通報](https://github.com/SpaceSquare640/Player_Club_Private_VPN/security/advisories/new) | 私下回報安全漏洞——見 [`SECURITY.zh-Hant.md`](SECURITY.zh-Hant.md) |

## 法律聲明

**專有軟體——保留一切權利。** 這是**網路軟體**：它會建立虛擬網路介面卡（需要系統管理員權限）、執行 NAT 穿透、於對等節點間加密流量，並在已連線的機器之間傳輸任意 IP 流量。**設定錯誤或誤用可能為您與第三方帶來嚴重的安全風險。** 請僅在您擁有或已獲得明確授權的網路上使用本軟體。本軟體為**搶先版、Alpha 品質、尚未經過安全稽核**，以**「現狀」提供、不附帶任何保證**，著作權持有人對於因使用本軟體所產生的損害**不負任何責任**。

使用或散布本軟體前，請詳閱完整條款：
[`LICENSE.zh-Hant.md`](LICENSE.zh-Hant.md)（授權條款，中文參考譯本，**以英文版
[`LICENSE`](LICENSE) 為準**）·
[`TERMS_OF_SERVICE.zh-Hant.md`](TERMS_OF_SERVICE.zh-Hant.md)（使用條款，中文參考譯本）·
[`PRIVACY_POLICY.zh-Hant.md`](PRIVACY_POLICY.zh-Hant.md)（私隱政策，中文參考譯本）·
[`SECURITY.zh-Hant.md`](SECURITY.zh-Hant.md)

隨附的第三方元件（尤其是 **Wintun**）仍受其各自的授權條款規範，詳見
[`src-tauri/resources/wintun/NOTICE.txt`](src-tauri/resources/wintun/NOTICE.txt)。
