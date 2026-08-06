//! Turns a decoded [`HelperRequest`] into an actual privileged action. Kept
//! behind a trait so the request/response cycle ([`super::server`]) can be
//! tested with a fake dispatcher, independent of any real Windows API call —
//! the same "separate what's testable from what needs a real elevated
//! Windows box" split this whole phase has followed.
#![allow(dead_code)]

use std::net::Ipv4Addr;

use super::protocol::RouteSpec;

/// One privileged operation, already validated (version-checked) by the
/// caller — implementations only need to perform the action and report
/// success or a human-readable failure message.
pub trait HelperDispatcher: Send {
    fn create_adapter(&mut self, name: &str, virtual_ip: Ipv4Addr, prefix_len: u8) -> Result<(), String>;
    fn configure_network_integration(&mut self, adapter_name: &str) -> Result<(), String>;
    fn remove_network_integration(&mut self, adapter_name: &str) -> Result<(), String>;
    fn add_extra_routes(&mut self, adapter_name: &str, routes: &[RouteSpec]) -> Result<(), String>;
    fn remove_extra_routes(&mut self, adapter_name: &str, routes: &[RouteSpec]) -> Result<(), String>;
}

/// The real dispatcher: calls straight into `tun::windows`'s existing
/// functions.
///
/// # Why this holds the adapter instead of creating and returning
///
/// Wintun's `WintunCloseAdapter` — which the `wintun` crate calls from
/// `Adapter`'s `Drop` — *removes* an adapter that was made with
/// `WintunCreateAdapter`. So the helper cannot create an adapter, drop its
/// handle, and leave something behind for the main process to use: the
/// adapter would disappear the moment `create_adapter` returned. (This is
/// the concrete answer to the design question steps 1–3 deliberately left
/// open rather than guessing at.)
///
/// The split that does work, and is what this implements: the **helper**
/// creates the adapter and holds the handle for its whole lifetime (so the
/// adapter exists as long as the helper process does), while the **main**
/// process opens that same adapter *by name* (`Adapter::open`, i.e.
/// `WintunOpenAdapter`) and starts its own session against it. Teardown is
/// the helper exiting, which drops the handle and removes the adapter.
///
/// **Unverified:** whether the main process's `Adapter::open` +
/// `start_session` actually succeeds *unelevated* is the open question this
/// whole architecture rests on, and this environment has no Administrator
/// access to answer it. If it turns out to require elevation too, the helper
/// buys nothing over the existing whole-app relaunch and this approach has
/// to change — which is exactly why `WintunDevice` is **not** yet rewired to
/// use it (see the `helper` module doc comment).
#[cfg(windows)]
#[derive(Default)]
pub struct WindowsDispatcher {
    /// Loaded lazily on the first `create_adapter`, then kept alive: every
    /// `Adapter` holds an `Arc` of this, so dropping it early would be
    /// wrong, and loading it per-call would be wasteful.
    wintun: Option<wintun::Wintun>,
    /// Adapters this helper created, keyed by name. Holding them here is
    /// what keeps them alive in the OS — see the type-level doc comment.
    adapters: std::collections::HashMap<String, std::sync::Arc<wintun::Adapter>>,
}

#[cfg(windows)]
impl HelperDispatcher for WindowsDispatcher {
    fn create_adapter(&mut self, name: &str, virtual_ip: Ipv4Addr, prefix_len: u8) -> Result<(), String> {
        if self.adapters.contains_key(name) {
            return Err(format!("adapter {name} was already created by this helper"));
        }

        let wintun = match &self.wintun {
            Some(w) => w.clone(),
            None => {
                let dll = super::super::tun::windows::locate_wintun_dll()
                    .ok_or_else(|| "wintun.dll not found in bundled resources".to_string())?;
                let loaded =
                    unsafe { wintun::load_from_path(&dll) }.map_err(|e| format!("load wintun.dll: {e}"))?;
                self.wintun = Some(loaded.clone());
                loaded
            }
        };

        let adapter = wintun::Adapter::create(&wintun, name, "Player Club", None)
            .map_err(|e| format!("create adapter: {e}"))?;
        super::super::tun::windows::assign_ip(name, virtual_ip, prefix_len).map_err(|e| e.to_string())?;

        self.adapters.insert(name.to_string(), adapter);
        Ok(())
    }

    fn configure_network_integration(&mut self, adapter_name: &str) -> Result<(), String> {
        super::super::tun::windows::configure_network_integration(adapter_name).map_err(|e| e.to_string())
    }

    fn remove_network_integration(&mut self, adapter_name: &str) -> Result<(), String> {
        super::super::tun::windows::remove_network_integration(adapter_name).map_err(|e| e.to_string())
    }

    fn add_extra_routes(&mut self, adapter_name: &str, routes: &[RouteSpec]) -> Result<(), String> {
        let routes: Vec<(Ipv4Addr, u8)> = routes.iter().map(|r| (r.network, r.prefix)).collect();
        super::super::tun::windows::add_extra_routes(adapter_name, &routes).map_err(|e| e.to_string())
    }

    fn remove_extra_routes(&mut self, adapter_name: &str, routes: &[RouteSpec]) -> Result<(), String> {
        let routes: Vec<(Ipv4Addr, u8)> = routes.iter().map(|r| (r.network, r.prefix)).collect();
        super::super::tun::windows::remove_extra_routes(adapter_name, &routes);
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Records every call it receives instead of touching the OS — lets
    /// `server`'s tests assert the dispatch loop calls the right method with
    /// the right arguments, and lets them control success/failure per call
    /// without needing Windows or elevation at all.
    #[derive(Default, Clone)]
    pub struct FakeDispatcher {
        pub calls: Arc<Mutex<Vec<String>>>,
        pub fail_next: Arc<Mutex<Option<String>>>,
    }

    impl FakeDispatcher {
        fn outcome(&self, call: String) -> Result<(), String> {
            self.calls.lock().unwrap().push(call);
            if let Some(msg) = self.fail_next.lock().unwrap().take() {
                return Err(msg);
            }
            Ok(())
        }
    }

    impl HelperDispatcher for FakeDispatcher {
        fn create_adapter(&mut self, name: &str, virtual_ip: Ipv4Addr, prefix_len: u8) -> Result<(), String> {
            self.outcome(format!("create_adapter({name}, {virtual_ip}, {prefix_len})"))
        }
        fn configure_network_integration(&mut self, adapter_name: &str) -> Result<(), String> {
            self.outcome(format!("configure_network_integration({adapter_name})"))
        }
        fn remove_network_integration(&mut self, adapter_name: &str) -> Result<(), String> {
            self.outcome(format!("remove_network_integration({adapter_name})"))
        }
        fn add_extra_routes(&mut self, adapter_name: &str, routes: &[RouteSpec]) -> Result<(), String> {
            self.outcome(format!("add_extra_routes({adapter_name}, {})", routes.len()))
        }
        fn remove_extra_routes(&mut self, adapter_name: &str, routes: &[RouteSpec]) -> Result<(), String> {
            self.outcome(format!("remove_extra_routes({adapter_name}, {})", routes.len()))
        }
    }
}
