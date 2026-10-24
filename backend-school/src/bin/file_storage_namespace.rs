//! Offline explicit namespace attestation. This command never migrates or calls storage.
use school_file_platform::{
    namespace_binding::{self, BindingError, BindingManifest, ConfiguredNamespaces},
    r2_storage_provider::R2StorageConfig,
};
use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    process::ExitCode,
};
use uuid::Uuid;

struct Command {
    mode: String,
    manifest: Option<String>,
    output: Option<String>,
    watch_ids: Vec<Uuid>,
    attestation: Option<String>,
    confirmed: bool,
}
fn arguments() -> Result<Command, BindingError> {
    let mut args = env::args().skip(1);
    let mode = args.next().ok_or(BindingError::InvalidManifest)?;
    if !matches!(mode.as_str(), "check" | "plan" | "dry-run" | "apply") {
        return Err(BindingError::InvalidManifest);
    }
    let mut command = Command {
        mode,
        manifest: None,
        output: None,
        watch_ids: Vec::new(),
        attestation: None,
        confirmed: false,
    };
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--manifest" => {
                command.manifest = Some(args.next().ok_or(BindingError::InvalidManifest)?)
            }
            "--output" => command.output = Some(args.next().ok_or(BindingError::InvalidManifest)?),
            "--watch-id" => command.watch_ids.push(
                args.next()
                    .ok_or(BindingError::InvalidManifest)?
                    .parse()
                    .map_err(|_| BindingError::InvalidManifest)?,
            ),
            "--attestation-digest" => {
                command.attestation = Some(args.next().ok_or(BindingError::InvalidManifest)?)
            }
            "--attest-historical-namespace" => command.confirmed = true,
            _ => return Err(BindingError::InvalidManifest),
        }
    }
    Ok(command)
}
async fn run() -> Result<bool, BindingError> {
    let command = arguments()?;
    let config = R2StorageConfig::from_env().map_err(|_| BindingError::Configuration)?;
    let namespaces = ConfiguredNamespaces::from_r2_config(&config)?;
    // Dedicated explicit operator URL. No DATABASE_URL discovery or migration/permission sync.
    let database_url =
        env::var("FILE_STORAGE_BINDING_DATABASE_URL").map_err(|_| BindingError::Configuration)?;
    let pool = school_tenancy::connect_privileged_pool(&database_url, 1)
        .await
        .map_err(|_| BindingError::Database)?;
    match command.mode.as_str() {
        "check" => {
            let result =
                namespace_binding::check_namespace_readiness(pool.as_ref(), &namespaces).await?;
            println!(
                "storage_namespace_check total={} unresolved={} mismatched={}",
                result.total, result.unresolved, result.mismatched
            );
            Ok(result.ready())
        }
        "plan" => {
            let attestation = command.attestation.ok_or(BindingError::InvalidManifest)?;
            let manifest = namespace_binding::prepare_binding_manifest(
                pool.as_ref(),
                &command.watch_ids,
                attestation,
                &namespaces,
            )
            .await?;
            let data =
                serde_json::to_vec_pretty(&manifest).map_err(|_| BindingError::InvalidManifest)?;
            let output = command.output.ok_or(BindingError::InvalidManifest)?;
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(output)
                .map_err(|_| BindingError::InvalidManifest)?;
            file.write_all(&data)
                .map_err(|_| BindingError::InvalidManifest)?;
            println!(
                "storage_namespace_manifest watches={}",
                manifest.watches.len()
            );
            Ok(true)
        }
        "dry-run" | "apply" => {
            let path = command.manifest.ok_or(BindingError::InvalidManifest)?;
            if fs::metadata(&path)
                .map_err(|_| BindingError::InvalidManifest)?
                .len()
                > 1024 * 1024
            {
                return Err(BindingError::InvalidManifest);
            }
            let data = fs::read(path).map_err(|_| BindingError::InvalidManifest)?;
            let manifest: BindingManifest =
                serde_json::from_slice(&data).map_err(|_| BindingError::InvalidManifest)?;
            let apply = command.mode == "apply";
            if apply && !command.confirmed {
                return Err(BindingError::InvalidManifest);
            }
            let selected = namespace_binding::bind_attested_namespaces(
                pool.as_ref(),
                &manifest,
                &namespaces,
                apply,
            )
            .await?;
            println!(
                "storage_namespace_binding selected={} applied={}",
                selected,
                if apply { selected } else { 0 }
            );
            Ok(true)
        }
        _ => Err(BindingError::InvalidManifest),
    }
}
#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
