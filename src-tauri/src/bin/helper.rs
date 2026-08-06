//! Elevation helper binary (Phase E.3, step 3). Launched elevated, separate
//! from the main GUI process, by a caller that has already verified
//! elevation (mirroring `engine::tun::privilege::relaunch_elevated`'s own
//! contract) — this binary does not itself check or request elevation; it
//! assumes whoever started it (with the `runas` verb) already secured that.
//!
//! Usage: `helper.exe <pipe-name>`. Serves exactly one client connection —
//! one helper process per app session, not a long-running daemon accepting
//! arbitrary reconnects — then exits once that connection ends or a
//! `Shutdown` request is processed. See `engine::helper`'s module doc
//! comment for what remains unverified about this (real elevated access has
//! never exercised this binary).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    use player_club_private_vpn_lib::engine::helper::dispatch::WindowsDispatcher;
    use player_club_private_vpn_lib::engine::helper::pipe;
    use std::process::ExitCode;

    let Some(pipe_name) = std::env::args().nth(1) else {
        eprintln!("usage: helper.exe <pipe-name>");
        return ExitCode::FAILURE;
    };

    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to start async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };

    runtime.block_on(async move {
        let server_pipe = match pipe::create_server(&pipe_name) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("failed to create pipe {pipe_name}: {e}");
                return ExitCode::FAILURE;
            }
        };
        if let Err(e) = server_pipe.connect().await {
            eprintln!("failed to accept a connection on {pipe_name}: {e}");
            return ExitCode::FAILURE;
        }
        let (read_half, write_half) = tokio::io::split(server_pipe);
        match player_club_private_vpn_lib::engine::helper::server::run(read_half, write_half, WindowsDispatcher).await
        {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("helper session ended with an error: {e}");
                ExitCode::FAILURE
            }
        }
    })
}

#[cfg(not(windows))]
fn main() -> std::process::ExitCode {
    eprintln!("the elevation helper is only supported on Windows");
    std::process::ExitCode::FAILURE
}
