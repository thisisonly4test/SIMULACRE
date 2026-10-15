//! `simulacre` — run the SIMULACRE platform locally.
//!
//! Environment:
//! - `SIMULACRE_ADDR`   listen address (default `127.0.0.1:7878`)
//! - `SIMULACRE_DATA`   data directory for the vault and faces (default `./simulacre-data`)
//! - `SIMULACRE_TOKEN`  optional shared secret; required when binding beyond localhost
//! - `SIMULACRE_FACE_URL` face source (default thispersondoesnotexist.com)

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use simulacre_server::{AppState, app, faces, store::Store};
use tracing_subscriber::EnvFilter;

const DEFAULT_DATA: &str = "simulacre-data";
const FORMER_DATA: &str = "legend-data";

fn usage() -> ! {
    eprintln!(
        "SIMULACRE {} — sock-puppet legend workbench\n\n\
         usage: simulacre [--addr HOST:PORT] [--data DIR] [--token SECRET]\n\n\
         env: SIMULACRE_ADDR, SIMULACRE_DATA, SIMULACRE_TOKEN, SIMULACRE_FACE_URL",
        env!("CARGO_PKG_VERSION")
    );
    std::process::exit(2)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,simulacre_server=info")))
        .init();

    let mut addr = std::env::var("SIMULACRE_ADDR").unwrap_or_else(|_| "127.0.0.1:7878".into());
    let mut data = PathBuf::from(std::env::var("SIMULACRE_DATA").unwrap_or_else(|_| DEFAULT_DATA.into()));
    let mut token = std::env::var("SIMULACRE_TOKEN").ok();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--addr" => addr = args.next().unwrap_or_else(|| usage()),
            "--data" => data = args.next().unwrap_or_else(|| usage()).into(),
            "--token" => token = Some(args.next().unwrap_or_else(|| usage())),
            _ => usage(),
        }
    }
    let addr: SocketAddr = addr.parse().unwrap_or_else(|_| usage());
    if !addr.ip().is_loopback() && token.as_deref().unwrap_or("").is_empty() {
        eprintln!("refusing to listen on {addr} without SIMULACRE_TOKEN: the vault would be readable by anyone on the network");
        std::process::exit(1);
    }

    // The platform used to be called LEGEND: adopt its default data directory
    // instead of silently starting with an empty vault.
    if data == Path::new(DEFAULT_DATA) && !data.exists() && Path::new(FORMER_DATA).is_dir() {
        match std::fs::rename(FORMER_DATA, DEFAULT_DATA) {
            Ok(()) => tracing::info!("moved {FORMER_DATA}/ to {DEFAULT_DATA}/"),
            Err(e) => tracing::warn!("could not move {FORMER_DATA}/ to {DEFAULT_DATA}/: {e}"),
        }
    }
    let faces_dir = data.join("faces");
    std::fs::create_dir_all(&faces_dir).expect("create data directory");
    let store = Store::open(&data.join("vault.sqlite")).expect("open vault");
    let source = std::env::var("SIMULACRE_FACE_URL").unwrap_or_else(|_| faces::DEFAULT_SOURCE.into());
    let state = AppState::new(store, faces_dir, source, token);

    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    tracing::info!("SIMULACRE listening on http://{addr} — data in {}", data.display());
    axum::serve(listener, app(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("server");
}
