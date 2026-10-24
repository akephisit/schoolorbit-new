//! Compatibility executable delegates to the same application-owned operational CLI.
use backend_school::operations;
use std::process::ExitCode;
#[tokio::main]
async fn main() -> ExitCode {
    match operations::storage_namespace::execute(std::env::args().skip(1).collect()).await {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            operations::cli_output::diagnostic(&error);
            ExitCode::FAILURE
        }
    }
}
