use super::policy::{copy_local_file, download_client, write_http_response, DownloadLimit};
use crate::{
    download_progress::{resume_action, send_status, ProgressReporter, ResumeAction},
    semantic_bootstrap::SemanticBootstrapEvent,
};
use anyhow::{anyhow, Context, Result};
use reqwest::{header::RANGE, StatusCode};
use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::{fs, io::AsyncReadExt, sync::mpsc::UnboundedSender, task::yield_now};
use url::Url;

/// HTTP UA string for runtime downloads.
const USER_AGENT: &str = "poqi-runtime-bootstrap/1.23.0";

pub(super) async fn download_archive(
    urls: &'static [&'static str],
    dest: &Path,
    label: &str,
    checksum: Option<&str>,
    event_tx: Option<&UnboundedSender<SemanticBootstrapEvent>>,
) -> Result<u64> {
    let mut last_err: Option<anyhow::Error> = None;
    for url in urls {
        let parsed = Url::parse(url).with_context(|| format!("invalid runtime URL {url}"))?;
        let result = if parsed.scheme() == "file" {
            let source = parsed
                .to_file_path()
                .map_err(|()| anyhow!("invalid file URL {url}"))?;
            copy_local_file(
                &source,
                dest,
                DownloadLimit::runtime_archive(),
                label,
                event_tx,
            )
            .await
        } else {
            download_http(&parsed, dest, label, checksum, event_tx).await
        };
        match result {
            Ok(bytes) => return Ok(bytes),
            Err(err) => {
                last_err = Some(err);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("no runtime download URLs succeeded")))
}

async fn download_http(
    url: &Url,
    dest: &Path,
    label: &str,
    checksum: Option<&str>,
    event_tx: Option<&UnboundedSender<SemanticBootstrapEvent>>,
) -> Result<u64> {
    let client = download_client(USER_AGENT)?;
    let limit = DownloadLimit::runtime_archive();
    let mut retried_after_range_not_satisfiable = false;
    loop {
        let mut existing_bytes = fs::metadata(dest).await.ok().map(|metadata| metadata.len());
        if existing_bytes.is_some_and(|bytes| !limit.accepts_existing(bytes)) {
            fs::remove_file(dest).await.ok();
            existing_bytes = None;
        }
        let resume_from = match resume_action(existing_bytes, None) {
            ResumeAction::StartFresh | ResumeAction::DeleteAndRestart => 0,
            ResumeAction::Resume(bytes) => bytes,
            ResumeAction::VerifyComplete => existing_bytes.unwrap_or_default(),
        };

        let mut request = client.get(url.clone());
        if resume_from > 0 {
            send_status(
                event_tx,
                format!(
                    "Resuming {label} archive from {}.",
                    crate::download_progress::format_mib(resume_from)
                ),
            );
            request = request.header(RANGE, format!("bytes={resume_from}-"));
        }

        let response = request
            .send()
            .await
            .with_context(|| format!("failed to request {url}"))?;
        let status = response.status();
        if status == StatusCode::RANGE_NOT_SATISFIABLE {
            if let Some(checksum) = checksum {
                if verify_checksum(dest, checksum, label, event_tx)
                    .await
                    .is_ok()
                {
                    return Ok(resume_from);
                }
            }
            fs::remove_file(dest).await.ok();
            if retried_after_range_not_satisfiable {
                return Err(anyhow!(
                    "server repeatedly rejected a clean download range for {label}"
                ));
            }
            retried_after_range_not_satisfiable = true;
            continue;
        }
        let response = response
            .error_for_status()
            .with_context(|| format!("download failed for {url}"))?;

        return write_http_response(response, dest, resume_from, limit, label, event_tx).await;
    }
}

pub(super) async fn verify_checksum(
    path: &Path,
    checksum: &str,
    label: &str,
    event_tx: Option<&UnboundedSender<SemanticBootstrapEvent>>,
) -> Result<()> {
    let mut file = fs::File::open(path)
        .await
        .with_context(|| format!("failed to open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let total = file.metadata().await.ok().map(|metadata| metadata.len());
    let mut verified = 0_u64;
    let mut progress = ProgressReporter::new("Verifying", label, total, event_tx);
    progress.emit(0);
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        verified += read as u64;
        hasher.update(&buffer[..read]);
        progress.maybe_emit(verified);
        yield_now().await;
    }
    progress.finish(verified);
    let digest = format!("{:x}", hasher.finalize());
    if digest != checksum {
        return Err(anyhow!(
            "checksum mismatch for {} (expected {}, got {digest})",
            path.display(),
            checksum
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::download_http;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        time::timeout,
    };
    use url::Url;

    #[tokio::test]
    async fn repeated_range_not_satisfiable_response_is_finite_and_cleans_partial() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = vec![0_u8; 2048];
                let bytes = socket.read(&mut request).await.unwrap();
                assert!(bytes > 0);
                socket
                    .write_all(
                        b"HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await
                    .unwrap();
            }
        });
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("runtime.download");
        tokio::fs::write(&path, b"partial").await.unwrap();
        let url = Url::parse(&format!("http://{address}/runtime")).unwrap();

        let error = timeout(
            Duration::from_secs(2),
            download_http(&url, &path, "test runtime", None, None),
        )
        .await
        .expect("download loop should be finite")
        .unwrap_err();

        assert!(error.to_string().contains("repeatedly rejected"));
        assert!(!tokio::fs::try_exists(&path).await.unwrap());
        server.await.unwrap();
    }
}
