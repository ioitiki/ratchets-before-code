//! `ledgerd`: serve the ledger over HTTP and run its jobs.
//!
//! LEDGER_DB    path to the journal file (required)
//! LEDGER_ADDR  address to listen on (default 127.0.0.1:8080; port 0 picks a free port)

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = PathBuf::from(
        std::env::var("LEDGER_DB").map_err(|_| "LEDGER_DB must name the journal file")?,
    );
    let addr = std::env::var("LEDGER_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let ledger = Arc::new(Mutex::new(ledgerd::Ledger::open(&db, ledgerd::now())?));
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    // The first line on stdout is a contract: the tests read the address from it.
    println!("ledgerd listening on {}", listener.local_addr()?);
    tokio::spawn(ledgerd::run_executor(Arc::clone(&ledger)));
    axum::serve(listener, ledgerd::router(ledger)).await?;
    Ok(())
}
