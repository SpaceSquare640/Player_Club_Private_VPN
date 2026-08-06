//! Wintun-backed virtual adapter (Windows).
//!
//! Loads the bundled, signed `wintun.dll`, creates an adapter, assigns the
//! virtual IPv4 address via `netsh`, and exposes the session as a `TunDevice`.
//!
//! Phase E.2 adds best-effort Windows network integration: a freshly-created
//! virtual adapter is often classified `Public` by Windows, which silently
//! blocks the very traffic this app exists to carry (see the "ping fails"
//! entry in `DOC/Two_Machine_Verification.md`). [`configure_network_integration`]
//! sets it to `Private` and adds an inbound allow rule scoped to exactly this
//! interface. This is OS hygiene, not routing — what traffic *reaches* the
//! adapter is `split_tunnel`'s job either way.
//!
//! E.2 also adds OS route management: [`add_extra_routes`] steers
//! caller-supplied prefixes (`TunConfig::extra_routes`) into the adapter via
//! `New-NetRoute`, beyond the adapter's own peer subnet, so a user can reach
//! e.g. another machine's LAN through their peer. This is narrower than
//! full site-to-site LAN sharing (which would additionally need the *peer's*
//! OS to forward traffic on our behalf) — that remains out of scope; this is
//! just "let me route a network I already know the address of through the
//! tunnel," symmetric with what `split_tunnel::SplitPolicy::extra_routes`
//! already does on the packet-filtering side.

use std::io;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use wintun::{Adapter, Session, Wintun};

use super::device::{DeviceInfo, TunConfig, TunDevice};

pub struct WintunDevice {
    // Field order governs drop order: session, then adapter, then the library.
    // `Session`'s methods take `&Arc<Self>`, so it must live in an Arc.
    session: Arc<Session>,
    _adapter: Arc<Adapter>,
    _wintun: Wintun,
    info: DeviceInfo,
    /// Routes `open` actually added, so `Drop` removes exactly those — not
    /// whatever `TunConfig` says *now* if it somehow differed (it can't
    /// today, since nothing mutates it after `open`, but tracking what was
    /// actually done rather than re-deriving it from config is the more
    /// robust invariant to hold as this code evolves).
    extra_routes: Vec<(Ipv4Addr, u8)>,
}

impl WintunDevice {
    pub fn open(cfg: &TunConfig) -> io::Result<Self> {
        let dll = locate_wintun_dll().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "wintun.dll not found in bundled resources",
            )
        })?;

        let wintun = unsafe { wintun::load_from_path(&dll) }
            .map_err(|e| io::Error::other(format!("load wintun.dll: {e}")))?;

        let adapter = Adapter::create(&wintun, &cfg.name, "Player Club", None)
            .map_err(|e| io::Error::other(format!("create adapter: {e}")))?;

        assign_ip(&cfg.name, cfg.virtual_ip, cfg.prefix_len)?;

        // Best-effort (E.2): never abort adapter creation over either of
        // these — see the doc comments on `configure_network_integration`
        // and `add_extra_routes`.
        let _ = configure_network_integration(&cfg.name);
        let _ = add_extra_routes(&cfg.name, &cfg.extra_routes);

        let session = Arc::new(
            adapter
                .start_session(wintun::MAX_RING_CAPACITY)
                .map_err(|e| io::Error::other(format!("start session: {e}")))?,
        );

        Ok(Self {
            session,
            _adapter: adapter,
            _wintun: wintun,
            info: DeviceInfo {
                name: cfg.name.clone(),
                mtu: cfg.mtu,
            },
            extra_routes: cfg.extra_routes.clone(),
        })
    }
}

impl TunDevice for WintunDevice {
    fn read_frame(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        match self.session.try_receive() {
            Ok(Some(packet)) => {
                let bytes = packet.bytes();
                let n = bytes.len().min(buf.len());
                buf[..n].copy_from_slice(&bytes[..n]);
                Ok(Some(n))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(io::Error::other(e.to_string())),
        }
    }

    fn write_frame(&mut self, frame: &[u8]) -> io::Result<usize> {
        let len = frame.len() as u16;
        let mut packet = self
            .session
            .allocate_send_packet(len)
            .map_err(|e| io::Error::other(e.to_string()))?;
        packet.bytes_mut().copy_from_slice(frame);
        self.session.send_packet(packet);
        Ok(frame.len())
    }

    fn info(&self) -> DeviceInfo {
        self.info.clone()
    }
}

impl Drop for WintunDevice {
    /// Remove the firewall rule `configure_network_integration` added and any
    /// routes `add_extra_routes` added. This runs before the field-order
    /// teardown below it (`session`, `_adapter`, `_wintun`) — neither removal
    /// depends on any of them still being alive, so the ordering is
    /// inconsequential here. Best-effort: a `Drop` cannot propagate an error,
    /// and one is not warranted — leaving a stale allow-rule or route for an
    /// adapter that no longer exists is inert, not a security regression
    /// (Windows removes routes bound to a deleted interface automatically in
    /// practice; this is defense in depth, not the only cleanup path).
    fn drop(&mut self) {
        let _ = remove_network_integration(&self.info.name);
        remove_extra_routes(&self.info.name, &self.extra_routes);
    }
}

/// Assign the IPv4 address via `netsh` (requires elevation, already verified
/// before the device is opened).
fn assign_ip(name: &str, ip: Ipv4Addr, prefix_len: u8) -> io::Result<()> {
    let mask = prefix_to_mask(prefix_len);
    let status = Command::new("netsh")
        .args([
            "interface",
            "ipv4",
            "set",
            "address",
            &format!("name={name}"),
            "static",
            &ip.to_string(),
            &mask.to_string(),
        ])
        .status()?;
    if !status.success() {
        return Err(io::Error::other(
            format!("netsh set address failed (exit {:?})", status.code()),
        ));
    }
    Ok(())
}

/// Firewall rule display name for a given adapter — deterministic, so teardown
/// can find and remove exactly the rule this adapter's creation added.
fn firewall_rule_name(adapter_name: &str) -> String {
    format!("PlayerClubVPN-{adapter_name}")
}

/// PowerShell single-quoted string literal (doubling embedded quotes). Used to
/// embed the adapter name inside a `-Command` script rather than relying on
/// `Command::args` joining, which PowerShell re-tokenizes internally as its own
/// syntax — a literal is unambiguous regardless of what the name contains.
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn run_powershell(script: &str) -> io::Result<()> {
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "powershell command failed (exit {:?}): {script}",
            status.code()
        )));
    }
    Ok(())
}

/// Best-effort Windows network integration (Phase E.2): classify the adapter's
/// network as `Private` (a fresh virtual adapter often defaults to `Public`,
/// which silently drops the traffic this app exists to carry) and add an
/// inbound allow rule scoped to exactly this interface via its `-InterfaceAlias`
/// (`netsh advfirewall` cannot scope by a specific interface name — only by
/// `interfacetype`, which would also match the host's real Ethernet/Wi-Fi
/// adapters — so this uses the `NetSecurity`/`NetConnection` PowerShell
/// cmdlets instead). Neither call is correctness-critical: what may reach the
/// adapter is `split_tunnel`'s job, done before any packet gets here, so a
/// failure here degrades to "Windows Firewall might also need a manual nudge"
/// rather than a broken adapter — callers ignore the `Result`.
fn configure_network_integration(adapter_name: &str) -> io::Result<()> {
    run_powershell(&format!(
        "Set-NetConnectionProfile -InterfaceAlias {} -NetworkCategory Private",
        ps_quote(adapter_name)
    ))?;
    run_powershell(&format!(
        "New-NetFirewallRule -DisplayName {} -InterfaceAlias {} -Direction Inbound -Action Allow -Profile Private | Out-Null",
        ps_quote(&firewall_rule_name(adapter_name)),
        ps_quote(adapter_name),
    ))
}

/// Remove the rule `configure_network_integration` added. Best-effort, like
/// its counterpart — see `Drop for WintunDevice`.
fn remove_network_integration(adapter_name: &str) -> io::Result<()> {
    run_powershell(&format!(
        "Remove-NetFirewallRule -DisplayName {} -ErrorAction SilentlyContinue",
        ps_quote(&firewall_rule_name(adapter_name)),
    ))
}

/// Steers `routes` (each `(network, prefix)`) into `adapter_name` via
/// `New-NetRoute`, scoped to that interface by alias — same reasoning as
/// `configure_network_integration`'s firewall rule: `route add`/`netsh`
/// route commands don't offer a clean by-interface-name scope the way the
/// PowerShell `NetTCPIP` cmdlets do. Best-effort and non-fatal per route: one
/// malformed or conflicting entry must not block the others, or take down
/// adapter creation over what is fundamentally a convenience feature.
fn add_route_script(adapter_name: &str, network: Ipv4Addr, prefix: u8) -> String {
    format!(
        "New-NetRoute -DestinationPrefix {} -InterfaceAlias {} -ErrorAction Stop | Out-Null",
        ps_quote(&format!("{network}/{prefix}")),
        ps_quote(adapter_name),
    )
}

fn remove_route_script(adapter_name: &str, network: Ipv4Addr, prefix: u8) -> String {
    format!(
        "Remove-NetRoute -DestinationPrefix {} -InterfaceAlias {} -Confirm:$false -ErrorAction SilentlyContinue",
        ps_quote(&format!("{network}/{prefix}")),
        ps_quote(adapter_name),
    )
}

fn add_extra_routes(adapter_name: &str, routes: &[(Ipv4Addr, u8)]) -> io::Result<()> {
    for (network, prefix) in routes {
        let _ = run_powershell(&add_route_script(adapter_name, *network, *prefix));
    }
    Ok(())
}

/// Remove the routes `add_extra_routes` added. Best-effort, like its
/// counterpart — see `Drop for WintunDevice`. Does not return a `Result`
/// since every call site already treats it as fire-and-forget.
fn remove_extra_routes(adapter_name: &str, routes: &[(Ipv4Addr, u8)]) {
    for (network, prefix) in routes {
        let _ = run_powershell(&remove_route_script(adapter_name, *network, *prefix));
    }
}

fn prefix_to_mask(prefix_len: u8) -> Ipv4Addr {
    let bits: u32 = if prefix_len == 0 {
        0
    } else if prefix_len >= 32 {
        u32::MAX
    } else {
        !(u32::MAX >> prefix_len)
    };
    Ipv4Addr::from(bits)
}

/// Resolve the bundled `wintun.dll` for the current architecture across both
/// dev and bundled layouts.
fn locate_wintun_dll() -> Option<PathBuf> {
    let arch = if cfg!(target_arch = "x86_64") {
        "amd64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        return None;
    };
    let rel = format!("resources/wintun/{arch}/wintun.dll");

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(&rel));
            candidates.push(dir.join(format!("wintun/{arch}/wintun.dll")));
            candidates.push(dir.join("wintun.dll"));
        }
    }
    // Dev fallback: the crate's resources dir, baked in at compile time.
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&rel));

    candidates.into_iter().find(|p| p.exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ps_quote_wraps_in_single_quotes() {
        assert_eq!(ps_quote("PlayerClubVPN"), "'PlayerClubVPN'");
    }

    /// PowerShell's own escaping rule for a single-quoted string literal is to
    /// double each embedded `'`. Getting this wrong would let a crafted or
    /// unusual adapter name break out of the literal and inject PowerShell.
    #[test]
    fn ps_quote_escapes_embedded_single_quotes() {
        assert_eq!(ps_quote("it's"), "'it''s'");
        assert_eq!(ps_quote("'; Remove-Item C:\\ -Recurse; '"), "'''; Remove-Item C:\\ -Recurse; '''");
    }

    #[test]
    fn firewall_rule_name_is_deterministic_and_prefixed() {
        assert_eq!(firewall_rule_name("PlayerClubVPN"), "PlayerClubVPN-PlayerClubVPN");
        // Same input always produces the same name, so teardown can find it.
        assert_eq!(firewall_rule_name("x"), firewall_rule_name("x"));
    }

    #[test]
    fn add_route_script_scopes_by_interface_alias_and_prefix() {
        let script = add_route_script("PlayerClubVPN", Ipv4Addr::new(192, 168, 50, 0), 24);
        assert!(script.starts_with("New-NetRoute"));
        assert!(script.contains("-DestinationPrefix '192.168.50.0/24'"));
        assert!(script.contains("-InterfaceAlias 'PlayerClubVPN'"));
    }

    #[test]
    fn remove_route_script_matches_the_same_prefix_it_was_added_with() {
        let added = add_route_script("PlayerClubVPN", Ipv4Addr::new(10, 0, 5, 0), 24);
        let removed = remove_route_script("PlayerClubVPN", Ipv4Addr::new(10, 0, 5, 0), 24);
        assert!(added.contains("-DestinationPrefix '10.0.5.0/24'"));
        assert!(removed.contains("-DestinationPrefix '10.0.5.0/24'"));
        assert!(removed.starts_with("Remove-NetRoute"));
    }

    /// The escaping guard already proven for the firewall path (`ps_quote`)
    /// applies here too — a hostile adapter name must not break out of the
    /// quoted `-InterfaceAlias` argument.
    #[test]
    fn add_route_script_escapes_a_hostile_adapter_name() {
        let script = add_route_script("'; Remove-Item C:\\ -Recurse; '", Ipv4Addr::new(1, 2, 3, 0), 24);
        assert!(script.contains("-InterfaceAlias '''; Remove-Item C:\\ -Recurse; '''"));
    }
}
