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
/// functions. `create_adapter` is the one operation this step does **not**
/// wire up for real — see the module doc comment on `helper` for why (the
/// Wintun session `WintunDevice::open` starts can't simply be created in one
/// process and handed to another; splitting adapter creation from session
/// start is a real design question for a later step, not something to
/// improvise here without a way to verify it).
#[cfg(windows)]
pub struct WindowsDispatcher;

#[cfg(windows)]
impl HelperDispatcher for WindowsDispatcher {
    fn create_adapter(&mut self, _name: &str, _virtual_ip: Ipv4Addr, _prefix_len: u8) -> Result<(), String> {
        Err("create_adapter is not yet implemented by the helper — see the helper module's \
             doc comment for why this one operation is deferred"
            .to_string())
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
