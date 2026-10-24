//! Compatibility executable delegates to the same application-owned operational CLI.
#[path = "../operations/mod.rs"]
pub mod operations;
use std::process::ExitCode;
#[tokio::main]
async fn main() -> ExitCode {
    match operations::storage_namespace::execute(std::env::args().skip(1).collect()).await {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
