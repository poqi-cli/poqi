use super::{
    super::specs::{
        join_relative, ArchiveType, RuntimeArtifact, RuntimeDependency, RuntimeSpec, DOWNLOAD_STEM,
    },
    send_status,
    transfer::{download_archive, verify_checksum},
};
use crate::semantic_bootstrap::SemanticBootstrapEvent;
use anyhow::{anyhow, Context, Result};
use async_compression::tokio::bufread::GzipDecoder;
use std::{
    collections::HashSet,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};
use tar::Archive;
use tokio::{fs, io::BufReader, sync::mpsc::UnboundedSender, task};
use tokio_util::io::SyncIoBridge;
use zip::ZipArchive;

const MEBIBYTE: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub(super) struct ArchiveRequest {
    pub(super) download_stem: &'static str,
    pub(super) download_urls: &'static [&'static str],
    pub(super) archive_type: ArchiveType,
    pub(super) label: &'static str,
    pub(super) artifacts: &'static [RuntimeArtifact],
    pub(super) sha256: Option<&'static str>,
}

impl ArchiveRequest {
    pub(super) const fn runtime(spec: &'static RuntimeSpec) -> Self {
        Self {
            download_stem: DOWNLOAD_STEM,
            download_urls: spec.download_urls,
            archive_type: spec.archive_type,
            label: spec.output_name,
            artifacts: spec.artifacts,
            sha256: spec.sha256,
        }
    }

    pub(super) const fn dependency(dep: &'static RuntimeDependency) -> Self {
        Self {
            download_stem: dep.download_stem,
            download_urls: dep.download_urls,
            archive_type: dep.archive_type,
            label: dep.label,
            artifacts: dep.artifacts,
            sha256: dep.sha256,
        }
    }
}

pub(super) async fn download_archive_entry(
    request: ArchiveRequest,
    dir: &Path,
    event_tx: Option<&UnboundedSender<SemanticBootstrapEvent>>,
) -> Result<()> {
    let ArchiveRequest {
        download_stem,
        download_urls,
        archive_type,
        label,
        artifacts,
        sha256,
    } = request;
    let archive_path = dir.join(download_stem);

    let mut restarted_after_checksum_mismatch = false;
    loop {
        send_status(event_tx, format!("Downloading {label}…"));
        download_archive(download_urls, &archive_path, label, sha256, event_tx).await?;

        let Some(checksum) = sha256 else {
            send_status(
                event_tx,
                "Checksum unavailable for this runtime — skipping verification",
            );
            break;
        };
        let verify_result = verify_checksum(&archive_path, checksum, label, event_tx).await;
        let Err(err) = verify_result else {
            break;
        };
        fs::remove_file(&archive_path).await.ok();
        if restarted_after_checksum_mismatch {
            return Err(err);
        }
        restarted_after_checksum_mismatch = true;
        send_status(
            event_tx,
            format!("Checksum mismatch for {label}; restarting clean download."),
        );
    }

    send_status(event_tx, format!("Extracting {label}…"));
    let extracted = match archive_type {
        ArchiveType::Zip => extract_zip_entries(&archive_path, dir, artifacts).await,
        ArchiveType::Tgz => extract_tgz_entries(&archive_path, dir, artifacts).await,
    };
    let extracted = match extracted {
        Ok(size) => size,
        Err(err) => {
            let _ = fs::remove_file(&archive_path).await;
            return Err(err);
        }
    };
    verify_artifacts(dir, artifacts, event_tx).await?;
    fs::remove_file(&archive_path).await.ok();

    let mib_label = format_mib(extracted);
    send_status(event_tx, format!("Extracted {label} ({mib_label})"));
    Ok(())
}

pub(super) async fn verify_artifacts(
    dir: &Path,
    artifacts: &[RuntimeArtifact],
    event_tx: Option<&UnboundedSender<SemanticBootstrapEvent>>,
) -> Result<()> {
    for artifact in artifacts {
        let path = join_relative(dir, artifact.output_path);
        verify_checksum(
            &path,
            artifact.output_sha256,
            artifact.output_path,
            event_tx,
        )
        .await
        .with_context(|| format!("failed to verify {}", path.display()))?;
    }
    Ok(())
}

pub(super) async fn prune_runtime_dir(dir: &Path, retain_paths: &[&str]) -> Result<()> {
    if retain_paths.is_empty() {
        return Ok(());
    }
    let dir = dir.to_path_buf();
    let retain: HashSet<PathBuf> = retain_paths
        .iter()
        .map(|relative| join_relative(&dir, relative))
        .collect();
    let mut entries = fs::read_dir(&dir)
        .await
        .with_context(|| format!("failed to list {}", dir.display()))?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if retain.contains(&path) {
            continue;
        }
        if entry.file_type().await?.is_dir() {
            fs::remove_dir_all(&path)
                .await
                .with_context(|| format!("failed to prune {}", path.display()))?;
        } else {
            fs::remove_file(&path)
                .await
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

#[cfg_attr(not(unix), allow(clippy::unnecessary_wraps))]
pub(super) fn set_unix_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)
            .with_context(|| format!("failed to read metadata for {}", path.display()))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms)
            .with_context(|| format!("failed to set permissions on {}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

async fn extract_zip_entries(
    archive_path: &Path,
    dest_dir: &Path,
    artifacts: &'static [RuntimeArtifact],
) -> Result<u64> {
    let archive_path = archive_path.to_path_buf();
    let dest_dir = dest_dir.to_path_buf();
    task::spawn_blocking(move || extract_zip_entries_blocking(&archive_path, &dest_dir, artifacts))
        .await?
}

fn extract_zip_entries_blocking(
    archive_path: &Path,
    dest_dir: &Path,
    artifacts: &[RuntimeArtifact],
) -> Result<u64> {
    let file = File::open(archive_path)
        .with_context(|| format!("failed to open {}", archive_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("failed to read {}", archive_path.display()))?;
    let mut targets = build_artifact_targets(artifacts);
    let mut total = 0_u64;
    for index in 0..archive.len() {
        if targets.is_empty() {
            break;
        }
        let mut entry = archive
            .by_index(index)
            .with_context(|| format!("failed to access zip entry {index}"))?;
        if entry.is_dir() {
            continue;
        }
        let components = split_archive_components(entry.name());
        if let Some(artifact) = remove_matching_artifact(&mut targets, &components) {
            write_archive_entry(&mut entry, artifact, dest_dir)?;
            total += entry.size();
        }
    }
    ensure_all_artifacts_extracted(targets)
        .with_context(|| "runtime archive was missing required file(s)")?;
    Ok(total)
}

async fn extract_tgz_entries(
    archive_path: &Path,
    dest_dir: &Path,
    artifacts: &[RuntimeArtifact],
) -> Result<u64> {
    let file = fs::File::open(archive_path)
        .await
        .with_context(|| format!("failed to open {}", archive_path.display()))?;
    let reader = BufReader::new(file);
    let decoder = GzipDecoder::new(reader);
    let bridge = SyncIoBridge::new(decoder);
    let targets = build_artifact_targets(artifacts);
    let destination = dest_dir.to_path_buf();
    task::spawn_blocking(move || extract_tgz_stream(bridge, &destination, targets)).await?
}

fn extract_tgz_stream<R: Read>(
    reader: R,
    dest_dir: &Path,
    mut targets: Vec<(Vec<String>, RuntimeArtifact)>,
) -> Result<u64> {
    let mut archive = Archive::new(reader);
    let mut total = 0_u64;
    for entry in archive.entries()? {
        if targets.is_empty() {
            break;
        }
        let mut entry = entry?;
        if entry.header().entry_type().is_dir() {
            continue;
        }
        let raw_path = entry.path()?.to_string_lossy().into_owned();
        let components = split_archive_components(&raw_path);
        if let Some(artifact) = remove_matching_artifact(&mut targets, &components) {
            write_archive_entry(&mut entry, artifact, dest_dir)?;
            total += entry.header().size()?;
        }
    }
    ensure_all_artifacts_extracted(targets)
        .with_context(|| "runtime archive was missing required file(s)")?;
    Ok(total)
}

fn build_artifact_targets(artifacts: &[RuntimeArtifact]) -> Vec<(Vec<String>, RuntimeArtifact)> {
    artifacts
        .iter()
        .map(|artifact| (split_archive_components(artifact.inner_path), *artifact))
        .collect()
}

fn split_archive_components(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|part| !part.is_empty())
        .map(std::string::ToString::to_string)
        .collect()
}

fn remove_matching_artifact(
    targets: &mut Vec<(Vec<String>, RuntimeArtifact)>,
    components: &[String],
) -> Option<RuntimeArtifact> {
    targets
        .iter()
        .position(|(expected, _)| expected.as_slice() == components)
        .map(|index| targets.remove(index).1)
}

fn ensure_all_artifacts_extracted(remaining: Vec<(Vec<String>, RuntimeArtifact)>) -> Result<()> {
    if remaining.is_empty() {
        return Ok(());
    }
    let missing = remaining
        .into_iter()
        .map(|(_, artifact)| artifact.inner_path)
        .collect::<Vec<_>>()
        .join(", ");
    Err(anyhow!(
        "runtime archive missing required file(s): {missing}"
    ))
}

fn write_archive_entry<R: Read>(
    entry: &mut R,
    artifact: RuntimeArtifact,
    dest_dir: &Path,
) -> Result<()> {
    let output_path = join_relative(dest_dir, artifact.output_path);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut dest = File::create(&output_path)
        .with_context(|| format!("failed to create {}", output_path.display()))?;
    io::copy(entry, &mut dest)
        .with_context(|| format!("failed to extract {}", output_path.display()))?;
    dest.sync_all()
        .with_context(|| format!("failed to flush {}", output_path.display()))?;
    Ok(())
}

fn to_mib_components(bytes: u64) -> (u64, u64) {
    let whole = bytes / MEBIBYTE;
    let tenths = ((bytes % MEBIBYTE) * 10) / MEBIBYTE;
    (whole, tenths)
}

fn format_mib(bytes: u64) -> String {
    let (whole, tenths) = to_mib_components(bytes);
    format!("{whole}.{tenths:01} MiB")
}
