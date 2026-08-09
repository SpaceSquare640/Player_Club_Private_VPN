//! Standalone relay server binary — run this on any always-reachable machine
//! (a cheap VPS, or a home machine with one port forwarded) to let virtual
//! networks created/joined across the internet find each other. See
//! `engine::relay`'s module doc comment for the handshake it implements.
//!
//! Not part of the desktop app's bundle — this is a separate opt-in tool for
//! whoever wants to run a relay, not something shipped to every user. Pure
//! `tokio`/`std::net`, so it builds and runs the same on every platform; no
//! GUI, no elevation, no OS-specific TUN code.
//!
//! Usage: `relay [--port <port>]` (default port 9420).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use player_club_private_vpn_lib::engine::relay::RelayServer;

fn parse_port(args: &[String]) -> Result<u16, String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--port" {
            let value = iter.next().ok_or("--port requires a value")?;
            return value.parse::<u16>().map_err(|e| format!("invalid --port value: {e}"));
        }
    }
    Ok(9420)
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let port = match parse_port(&args) {
        Ok(port) => port,
        Err(e) => {
            eprintln!("{e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
    let server = match RelayServer::start(bind_addr).await {
        Ok(server) => server,
        Err(e) => {
            eprintln!("failed to bind {bind_addr}: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    println!("[*] Player Club Private VPN relay listening on {}", server.local_addr());
    println!("[*] Give this machine's reachable address (ip:port) to anyone hosting/joining across the internet.");
    println!("[*] Press Ctrl+C to stop (the process exits immediately — this relay carries no state worth flushing).");

    // No graceful-shutdown path: a bare process kill (Ctrl+C, service
    // manager stop) is exactly as clean as calling `RelayServer::shutdown` —
    // every connection it's splicing just closes, which is already how a
    // participant leaving mid-session is handled everywhere else in this
    // relay. `server` is kept alive by staying in scope, not used again.
    let _server = server;
    std::future::pending::<()>().await;
    unreachable!()
}
