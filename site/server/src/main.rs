//! `ferrochart-site-server [--addr 127.0.0.1:8080] [--site ../out] [--cors]`
//!
//! Serves ferrochart.com: the static site from `--site` and the data API
//! under `/api`. Without `--site`, the API only (for `next dev`, with
//! `--cors`).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use ferrochart_site_server::{Config, app};
use tracing_subscriber::EnvFilter;

const USAGE: &str = "usage: ferrochart-site-server [--addr HOST:PORT] [--site DIR] [--cors]";

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .init();
    let mut addr: SocketAddr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let mut config = Config::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--addr" => match args.next().and_then(|v| v.parse().ok()) {
                Some(v) => addr = v,
                None => return fail("--addr needs HOST:PORT"),
            },
            "--site" => match args.next() {
                Some(v) => config.site_dir = Some(PathBuf::from(v)),
                None => return fail("--site needs a directory"),
            },
            "--cors" => config.cors = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other => return fail(&format!("unknown argument '{other}'")),
        }
    }
    if let Some(dir) = &config.site_dir
        && !dir.join("index.html").is_file()
    {
        return fail(&format!(
            "{} has no index.html (run `npm run build` in site/)",
            dir.display()
        ));
    }
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => return fail(&format!("cannot listen on {addr}: {e}")),
    };
    let addr = listener.local_addr().unwrap_or(addr);
    tracing::info!("listening on http://{addr}");
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    match axum::serve(listener, app(config))
        .with_graceful_shutdown(shutdown)
        .await
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&format!("server error: {e}")),
    }
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("{msg}\n{USAGE}");
    ExitCode::FAILURE
}
