use crate::{download_progress::ProgressReporter, semantic_bootstrap::SemanticBootstrapEvent};
use anyhow::{anyhow, Context, Result};
use futures::StreamExt;
use reqwest::{
    header::{CONTENT_LENGTH, CONTENT_RANGE},
    Client, Response, StatusCode,
};
use std::{path::Path, time::Duration};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc::UnboundedSender,
    task::yield_now,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const READ_TIMEOUT: Duration = Duration::from_mins(1);
const TRANSFER_TIMEOUT: Duration = Duration::from_hours(2);
pub(crate) const MAX_RUNTIME_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub(crate) enum DownloadLimit {
    Exact(u64),
    AtMost(u64),
}

impl DownloadLimit {
    pub(crate) const fn exact(bytes: u64) -> Self {
        Self::Exact(bytes)
    }

    pub(crate) const fn runtime_archive() -> Self {
        Self::AtMost(MAX_RUNTIME_ARCHIVE_BYTES)
    }

    pub(crate) const fn max_bytes(self) -> u64 {
        match self {
            Self::Exact(bytes) | Self::AtMost(bytes) => bytes,
        }
    }

    pub(crate) const fn expected_bytes(self) -> Option<u64> {
        match self {
            Self::Exact(bytes) => Some(bytes),
            Self::AtMost(_) => None,
        }
    }

    pub(crate) fn accepts_existing(self, bytes: u64) -> bool {
        match self {
            Self::Exact(expected) => bytes <= expected,
            Self::AtMost(maximum) => bytes <= maximum,
        }
    }

    fn validate_total(self, bytes: u64, label: &str) -> Result<()> {
        match self {
            Self::Exact(expected) if bytes != expected => Err(anyhow!(
                "invalid size for {label}: expected {expected} bytes, got {bytes}"
            )),
            Self::AtMost(maximum) if bytes > maximum => Err(anyhow!(
                "download for {label} exceeds the {maximum}-byte limit"
            )),
            Self::Exact(_) | Self::AtMost(_) => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ResponsePlan {
    append: bool,
    start_bytes: u64,
    expected_end: Option<u64>,
    progress_total: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct ContentRange {
    start: u64,
    end: u64,
    total: u64,
}

pub(crate) fn download_client(user_agent: &str) -> Result<Client> {
    download_client_with_deadlines(user_agent, CONNECT_TIMEOUT, READ_TIMEOUT, TRANSFER_TIMEOUT)
}

fn download_client_with_deadlines(
    user_agent: &str,
    connect_timeout: Duration,
    read_timeout: Duration,
    transfer_timeout: Duration,
) -> Result<Client> {
    Client::builder()
        .user_agent(user_agent)
        .connect_timeout(connect_timeout)
        .read_timeout(read_timeout)
        .timeout(transfer_timeout)
        .build()
        .context("failed to build bounded download HTTP client")
}

pub(crate) async fn write_http_response(
    response: Response,
    path: &Path,
    resume_from: u64,
    limit: DownloadLimit,
    label: &str,
    event_tx: Option<&UnboundedSender<SemanticBootstrapEvent>>,
) -> Result<u64> {
    let plan = match response_plan(&response, resume_from, limit, label) {
        Ok(plan) => plan,
        Err(err) => {
            remove_invalid_partial(path).await;
            return Err(err);
        }
    };

    if plan.append {
        let current_bytes = match fs::metadata(path).await {
            Ok(metadata) => metadata.len(),
            Err(err) => {
                remove_invalid_partial(path).await;
                return Err(err)
                    .with_context(|| format!("failed to inspect resume file {}", path.display()));
            }
        };
        if current_bytes != plan.start_bytes {
            remove_invalid_partial(path).await;
            return Err(anyhow!(
                "resume file for {label} changed size: expected {} bytes, got {current_bytes}",
                plan.start_bytes
            ));
        }
    }

    let mut stream = response.bytes_stream();
    let mut file = open_download_file(path, plan.append).await?;
    let mut downloaded = plan.start_bytes;
    let mut progress = ProgressReporter::new("Downloading", label, plan.progress_total, event_tx);
    progress.emit(downloaded);

    while let Some(chunk) = stream.next().await {
        let data = chunk.context("download response body failed")?;
        let next = match downloaded.checked_add(data.len() as u64) {
            Some(next) if next <= limit.max_bytes() => next,
            _ => {
                drop(file);
                remove_invalid_partial(path).await;
                return Err(anyhow!(
                    "download for {label} exceeds the {}-byte limit",
                    limit.max_bytes()
                ));
            }
        };
        file.write_all(&data)
            .await
            .with_context(|| format!("failed to write {}", path.display()))?;
        downloaded = next;
        progress.maybe_emit(downloaded);
        yield_now().await;
    }

    file.flush()
        .await
        .with_context(|| format!("failed to flush {}", path.display()))?;
    drop(file);

    if let Some(expected_end) = plan.expected_end {
        if downloaded != expected_end {
            remove_invalid_partial(path).await;
            return Err(anyhow!(
                "incomplete download for {label}: expected {expected_end} bytes, got {downloaded}"
            ));
        }
    }
    if let Err(err) = limit.validate_total(downloaded, label) {
        remove_invalid_partial(path).await;
        return Err(err);
    }

    progress.finish(downloaded);
    Ok(downloaded)
}

pub(crate) async fn copy_local_file(
    source: &Path,
    path: &Path,
    limit: DownloadLimit,
    label: &str,
    event_tx: Option<&UnboundedSender<SemanticBootstrapEvent>>,
) -> Result<u64> {
    let source_size = fs::metadata(source)
        .await
        .with_context(|| format!("failed to read metadata for {}", source.display()))?
        .len();
    if let Err(err) = limit.validate_total(source_size, label) {
        remove_invalid_partial(path).await;
        return Err(err);
    }

    let mut source_file = fs::File::open(source)
        .await
        .with_context(|| format!("failed to open {}", source.display()))?;
    let mut dest_file = fs::File::create(path)
        .await
        .with_context(|| format!("failed to create {}", path.display()))?;
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut copied = 0_u64;
    let mut progress = ProgressReporter::new("Downloading", label, Some(source_size), event_tx);
    progress.emit(0);

    loop {
        let read = source_file
            .read(&mut buffer)
            .await
            .with_context(|| format!("failed to read {}", source.display()))?;
        if read == 0 {
            break;
        }
        let next = match copied.checked_add(read as u64) {
            Some(next) if next <= limit.max_bytes() => next,
            _ => {
                drop(dest_file);
                remove_invalid_partial(path).await;
                return Err(anyhow!(
                    "download for {label} exceeds the {}-byte limit",
                    limit.max_bytes()
                ));
            }
        };
        dest_file
            .write_all(&buffer[..read])
            .await
            .with_context(|| format!("failed to write {}", path.display()))?;
        copied = next;
        progress.maybe_emit(copied);
        yield_now().await;
    }

    dest_file
        .flush()
        .await
        .with_context(|| format!("failed to flush {}", path.display()))?;
    drop(dest_file);
    if let Err(err) = limit.validate_total(copied, label) {
        remove_invalid_partial(path).await;
        return Err(err);
    }
    progress.finish(copied);
    Ok(copied)
}

fn response_plan(
    response: &Response,
    resume_from: u64,
    limit: DownloadLimit,
    label: &str,
) -> Result<ResponsePlan> {
    let content_length = header_u64(response, CONTENT_LENGTH, label)?;
    match response.status() {
        StatusCode::OK => full_response_plan(content_length, limit, label),
        StatusCode::PARTIAL_CONTENT => {
            partial_response_plan(response, content_length, resume_from, limit, label)
        }
        status => Err(anyhow!("unexpected HTTP status {status} for {label}")),
    }
}

fn full_response_plan(
    content_length: Option<u64>,
    limit: DownloadLimit,
    label: &str,
) -> Result<ResponsePlan> {
    if let Some(length) = content_length {
        limit.validate_total(length, label)?;
    }
    Ok(ResponsePlan {
        append: false,
        start_bytes: 0,
        expected_end: content_length.or(limit.expected_bytes()),
        progress_total: limit.expected_bytes().or(content_length),
    })
}

fn partial_response_plan(
    response: &Response,
    content_length: Option<u64>,
    resume_from: u64,
    limit: DownloadLimit,
    label: &str,
) -> Result<ResponsePlan> {
    if resume_from == 0 {
        return Err(anyhow!(
            "unexpected partial response for fresh download of {label}"
        ));
    }
    if !limit.accepts_existing(resume_from) {
        return Err(anyhow!(
            "resume offset {resume_from} for {label} exceeds the download limit"
        ));
    }
    let range = response
        .headers()
        .get(CONTENT_RANGE)
        .ok_or_else(|| anyhow!("partial response for {label} is missing Content-Range"))?
        .to_str()
        .with_context(|| format!("invalid Content-Range for {label}"))
        .and_then(|value| parse_content_range(value, label))?;
    if range.start != resume_from {
        return Err(anyhow!(
            "invalid resume range for {label}: requested byte {resume_from}, response starts at {}",
            range.start
        ));
    }
    limit.validate_total(range.total, label)?;
    if range.end >= limit.max_bytes() {
        return Err(anyhow!(
            "Content-Range for {label} exceeds the download limit"
        ));
    }
    let segment_length = range
        .end
        .checked_sub(range.start)
        .and_then(|length| length.checked_add(1))
        .ok_or_else(|| anyhow!("invalid Content-Range length for {label}"))?;
    let response_end = range
        .end
        .checked_add(1)
        .ok_or_else(|| anyhow!("invalid Content-Range end for {label}"))?;
    if response_end != range.total {
        return Err(anyhow!(
            "partial response for {label} ended at byte {response_end}, expected {}",
            range.total
        ));
    }
    if let Some(length) = content_length {
        if length != segment_length {
            return Err(anyhow!(
                "Content-Length mismatch for {label}: range contains {segment_length} bytes, header says {length}"
            ));
        }
    }
    Ok(ResponsePlan {
        append: true,
        start_bytes: resume_from,
        expected_end: Some(response_end),
        progress_total: Some(range.total),
    })
}

fn header_u64(
    response: &Response,
    name: reqwest::header::HeaderName,
    label: &str,
) -> Result<Option<u64>> {
    response
        .headers()
        .get(name)
        .map(|value| {
            value
                .to_str()
                .with_context(|| format!("invalid Content-Length for {label}"))?
                .parse::<u64>()
                .with_context(|| format!("invalid Content-Length for {label}"))
        })
        .transpose()
}

fn parse_content_range(value: &str, label: &str) -> Result<ContentRange> {
    let (unit, range_and_total) = value
        .split_once(' ')
        .ok_or_else(|| anyhow!("invalid Content-Range for {label}"))?;
    if !unit.eq_ignore_ascii_case("bytes") {
        return Err(anyhow!("unsupported Content-Range unit for {label}"));
    }
    let (range, total) = range_and_total
        .split_once('/')
        .ok_or_else(|| anyhow!("invalid Content-Range for {label}"))?;
    let (start, end) = range
        .split_once('-')
        .ok_or_else(|| anyhow!("invalid Content-Range for {label}"))?;
    let parsed = ContentRange {
        start: start
            .parse::<u64>()
            .with_context(|| format!("invalid Content-Range start for {label}"))?,
        end: end
            .parse::<u64>()
            .with_context(|| format!("invalid Content-Range end for {label}"))?,
        total: total
            .parse::<u64>()
            .with_context(|| format!("invalid Content-Range total for {label}"))?,
    };
    if parsed.end < parsed.start || parsed.end >= parsed.total {
        return Err(anyhow!("invalid Content-Range ordering for {label}"));
    }
    Ok(parsed)
}

async fn open_download_file(path: &Path, append: bool) -> Result<fs::File> {
    if append {
        return fs::OpenOptions::new()
            .append(true)
            .open(path)
            .await
            .with_context(|| format!("failed to open {} for resume", path.display()));
    }
    fs::File::create(path)
        .await
        .with_context(|| format!("failed to create {}", path.display()))
}

async fn remove_invalid_partial(path: &Path) {
    fs::remove_file(path).await.ok();
}

#[cfg(test)]
mod tests {
    use super::{download_client_with_deadlines, write_http_response, DownloadLimit};
    use reqwest::header::RANGE;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
        time::{sleep, timeout},
    };

    const TEST_TIMEOUT: Duration = Duration::from_millis(100);

    #[tokio::test]
    async fn valid_partial_response_resumes_exact_download() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("asset.download");
        tokio::fs::write(&path, b"abc").await.unwrap();
        let (url, server) = serve_once(
            b"HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 3-5/6\r\nConnection: close\r\n\r\ndef"
                .to_vec(),
            None,
        )
        .await;

        let response = test_client()
            .get(url)
            .header(RANGE, "bytes=3-")
            .send()
            .await
            .unwrap();
        let downloaded = write_http_response(
            response,
            &path,
            3,
            DownloadLimit::exact(6),
            "test asset",
            None,
        )
        .await
        .unwrap();

        assert_eq!(downloaded, 6);
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"abcdef");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn wrong_resume_offset_is_rejected_and_partial_is_removed() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("asset.download");
        tokio::fs::write(&path, b"abc").await.unwrap();
        let (url, server) = serve_once(
            b"HTTP/1.1 206 Partial Content\r\nContent-Length: 4\r\nContent-Range: bytes 2-5/6\r\nConnection: close\r\n\r\ncdef"
                .to_vec(),
            None,
        )
        .await;

        let response = test_client()
            .get(url)
            .header(RANGE, "bytes=3-")
            .send()
            .await
            .unwrap();
        let error = write_http_response(
            response,
            &path,
            3,
            DownloadLimit::exact(6),
            "test asset",
            None,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("response starts at 2"));
        assert!(!tokio::fs::try_exists(&path).await.unwrap());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn invalid_partial_lengths_and_totals_are_rejected_and_cleaned() {
        let cases = [
            (
                "bytes 3-5/7",
                3,
                DownloadLimit::exact(6),
                "expected 6 bytes, got 7",
            ),
            (
                "bytes 3-5/6",
                2,
                DownloadLimit::exact(6),
                "range contains 3 bytes, header says 2",
            ),
            (
                "bytes 3-6/7",
                4,
                DownloadLimit::AtMost(6),
                "exceeds the 6-byte limit",
            ),
            (
                "bytes 3-4/6",
                2,
                DownloadLimit::exact(6),
                "ended at byte 5, expected 6",
            ),
        ];

        for (index, (content_range, content_length, limit, expected_error)) in
            cases.into_iter().enumerate()
        {
            let temp = TempDir::new().unwrap();
            let path = temp.path().join(format!("asset-{index}.download"));
            tokio::fs::write(&path, b"abc").await.unwrap();
            let response_bytes = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {content_length}\r\nContent-Range: {content_range}\r\nConnection: close\r\n\r\ndefg"
            )
            .into_bytes();
            let (url, server) = serve_once(response_bytes, None).await;
            let response = test_client()
                .get(url)
                .header(RANGE, "bytes=3-")
                .send()
                .await
                .unwrap();

            let error = write_http_response(response, &path, 3, limit, "test asset", None)
                .await
                .unwrap_err();

            assert!(
                error.to_string().contains(expected_error),
                "unexpected error for case {index}: {error}"
            );
            assert!(!tokio::fs::try_exists(&path).await.unwrap());
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn invalid_full_content_length_is_rejected_before_file_open() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("missing-parent").join("asset.download");
        let (url, server) = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nabcdefg".to_vec(),
            None,
        )
        .await;
        let response = test_client().get(url).send().await.unwrap();

        let error = write_http_response(
            response,
            &path,
            0,
            DownloadLimit::exact(6),
            "test asset",
            None,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("expected 6 bytes, got 7"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn full_response_to_range_request_restarts_from_zero() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("asset.download");
        tokio::fs::write(&path, b"stale partial").await.unwrap();
        let (url, server) = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabcdef".to_vec(),
            None,
        )
        .await;

        let response = test_client()
            .get(url)
            .header(RANGE, "bytes=5-")
            .send()
            .await
            .unwrap();
        write_http_response(
            response,
            &path,
            5,
            DownloadLimit::exact(6),
            "test asset",
            None,
        )
        .await
        .unwrap();

        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"abcdef");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn chunked_body_is_capped_before_oversized_chunk_is_written() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("archive.download");
        let (url, server) = serve_once(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n3\r\ndef\r\n0\r\n\r\n"
                .to_vec(),
            None,
        )
        .await;

        let response = test_client().get(url).send().await.unwrap();
        let error = write_http_response(
            response,
            &path,
            0,
            DownloadLimit::AtMost(5),
            "test archive",
            None,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("exceeds the 5-byte limit"));
        assert!(!tokio::fs::try_exists(&path).await.unwrap());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn incomplete_exact_chunked_body_is_removed() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("asset.download");
        let (url, server) = serve_once(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n0\r\n\r\n"
                .to_vec(),
            None,
        )
        .await;

        let response = test_client().get(url).send().await.unwrap();
        let error = write_http_response(
            response,
            &path,
            0,
            DownloadLimit::exact(6),
            "test asset",
            None,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("incomplete download"));
        assert!(!tokio::fs::try_exists(&path).await.unwrap());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn stalled_response_headers_hit_the_read_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_request(&mut socket).await;
            sleep(Duration::from_millis(500)).await;
        });
        let error = timeout(
            Duration::from_secs(1),
            test_client().get(format!("http://{address}/asset")).send(),
        )
        .await
        .expect("header read deadline should fire")
        .unwrap_err();

        assert!(error.is_timeout());
        server.abort();
    }

    #[tokio::test]
    async fn stalled_body_hits_read_deadline_and_preserves_bounded_partial() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("asset.download");
        let (url, server) = serve_once(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n".to_vec(),
            Some(Duration::from_secs(5)),
        )
        .await;
        let response = test_client().get(url).send().await.unwrap();
        let error = timeout(
            Duration::from_secs(1),
            write_http_response(
                response,
                &path,
                0,
                DownloadLimit::exact(6),
                "test asset",
                None,
            ),
        )
        .await
        .expect("body read deadline should fire")
        .unwrap_err();

        assert!(error
            .chain()
            .any(|cause| cause.to_string().contains("timed out")));
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"abc");
        server.abort();
    }

    fn test_client() -> reqwest::Client {
        download_client_with_deadlines(
            "poqi-download-policy-test",
            Duration::from_secs(2),
            TEST_TIMEOUT,
            Duration::from_secs(1),
        )
        .unwrap()
    }

    async fn serve_once(
        response: Vec<u8>,
        hold_open: Option<Duration>,
    ) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_request(&mut socket).await;
            socket.write_all(&response).await.unwrap();
            socket.flush().await.unwrap();
            if let Some(duration) = hold_open {
                sleep(duration).await;
            }
        });
        (format!("http://{address}/asset"), server)
    }

    async fn read_request(socket: &mut tokio::net::TcpStream) {
        let mut request = vec![0_u8; 2048];
        let bytes = socket.read(&mut request).await.unwrap();
        assert!(bytes > 0);
    }

    #[test]
    fn invalid_content_range_forms_are_rejected() {
        for value in [
            "bytes 6-5/10",
            "bytes 5-10/10",
            "bytes 5-9/*",
            "items 5-9/10",
            "invalid",
        ] {
            assert!(
                super::parse_content_range(value, "test").is_err(),
                "{value}"
            );
        }
    }
}
