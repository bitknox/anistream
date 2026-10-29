//! Where the relay and the probe fetch from: an [`Upstream`] that answers a GET with a streamed
//! body. The engine hands its own, over the HTTP client every source is fetched with (its TLS,
//! its emulated browser); [`PlainHttp`] is the plain HTTP the mender and the torrent server speak
//! on loopback, and what the tests use.
//!
//! [`RangeFile`] reads one resource as `Read + Seek`, by ranges, for the remuxer: a torrent's
//! file through its loopback server, or a file on disk.

use std::{
    io::{Read, Seek, SeekFrom},
    pin::Pin,
    sync::Arc,
};

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};

/// A streamed body.
pub type Body = Pin<Box<dyn Stream<Item = std::io::Result<Vec<u8>>> + Send>>;

/// An answer from upstream: its status, its headers with lower-case names, and its body.
pub struct UpstreamResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Body,
}

impl UpstreamResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// The whole body, up to `limit` bytes.
    pub async fn bytes(mut self, limit: usize) -> std::io::Result<Vec<u8>> {
        let mut out = Vec::new();
        while let Some(chunk) = self.body.next().await {
            out.extend_from_slice(&chunk?);
            if out.len() > limit {
                return Err(std::io::Error::other("the answer is larger than expected"));
            }
        }
        Ok(out)
    }

    /// The body's first `limit` bytes at most; the rest is left unread.
    pub async fn prefix(mut self, limit: usize) -> std::io::Result<Vec<u8>> {
        let mut out = Vec::new();
        while out.len() < limit
            && let Some(chunk) = self.body.next().await
        {
            out.extend_from_slice(&chunk?);
        }
        out.truncate(limit);
        Ok(out)
    }

    /// The size of the whole resource, from `Content-Range` or, for a whole answer,
    /// `Content-Length`.
    pub fn total_size(&self) -> Option<u64> {
        if let Some(range) = self.header("content-range") {
            return range.rsplit('/').next().and_then(|s| s.trim().parse().ok());
        }
        (self.status == 200)
            .then(|| self.header("content-length")?.trim().parse().ok())
            .flatten()
    }
}

/// A GET with these headers, `Range` among them when one is wanted.
#[async_trait]
pub trait Upstream: Send + Sync {
    async fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> std::io::Result<UpstreamResponse>;
}

/// Plain HTTP/1.1, one request a connection: the loopback servers this device runs.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlainHttp;

#[async_trait]
impl Upstream for PlainHttp {
    async fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> std::io::Result<UpstreamResponse> {
        let rest = url
            .strip_prefix("http://")
            .ok_or_else(|| std::io::Error::other("plain HTTP only"))?;
        let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
        let path = if path.is_empty() { "/" } else { path };
        let mut socket = TcpStream::connect(authority).await?;
        let mut ask =
            format!("GET {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n");
        for (name, value) in headers {
            if name.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
                continue;
            }
            ask.push_str(&format!("{name}: {value}\r\n"));
        }
        ask.push_str("\r\n");
        socket.write_all(ask.as_bytes()).await?;

        let mut reader = BufReader::new(socket);
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        let status = line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| std::io::Error::other("no status line"))?;
        let mut headers = Vec::new();
        let mut total = line.len();
        loop {
            let mut header = String::new();
            let n = reader.read_line(&mut header).await?;
            total += n;
            if n == 0 || total > 64 * 1024 {
                return Err(std::io::Error::other("a broken response head"));
            }
            let header = header.trim_end();
            if header.is_empty() {
                break;
            }
            if let Some((k, v)) = header.split_once(':') {
                headers.push((k.trim().to_ascii_lowercase(), v.trim().to_owned()));
            }
        }
        let chunked = headers.iter().any(|(k, v)| {
            k == "transfer-encoding" && v.to_ascii_lowercase().contains("chunked")
        });
        let length: Option<u64> = headers
            .iter()
            .find(|(k, _)| k == "content-length")
            .and_then(|(_, v)| v.parse().ok());
        let body: Body = if chunked {
            Box::pin(futures::stream::unfold(Some(reader), |reader| async move {
                let mut reader = reader?;
                match next_chunk(&mut reader).await {
                    Ok(Some(chunk)) => Some((Ok(chunk), Some(reader))),
                    Ok(None) => None,
                    Err(e) => Some((Err(e), None)),
                }
            }))
        } else {
            let reader: Box<dyn tokio::io::AsyncRead + Send + Unpin> = match length {
                Some(length) => Box::new(reader.take(length)),
                None => Box::new(reader),
            };
            Box::pin(futures::stream::unfold(Some(reader), |reader| async move {
                let mut reader = reader?;
                let mut buffer = vec![0u8; 64 * 1024];
                match reader.read(&mut buffer).await {
                    Ok(0) => None,
                    Ok(n) => {
                        buffer.truncate(n);
                        Some((Ok(buffer), Some(reader)))
                    }
                    Err(e) => Some((Err(e), None)),
                }
            }))
        };
        Ok(UpstreamResponse { status, headers, body })
    }
}

/// One chunk of a chunked body, or `None` at its end.
async fn next_chunk(reader: &mut BufReader<TcpStream>) -> std::io::Result<Option<Vec<u8>>> {
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let size = usize::from_str_radix(line.trim().split(';').next().unwrap_or_default(), 16)
        .map_err(|_| std::io::Error::other("a broken chunk size"))?;
    if size == 0 {
        return Ok(None);
    }
    let mut chunk = vec![0u8; size];
    reader.read_exact(&mut chunk).await?;
    let mut end = [0u8; 2];
    reader.read_exact(&mut end).await?;
    Ok(Some(chunk))
}

/// The bytes a [`RangeFile`] reads ahead of where it was asked, so the remuxer's small reads
/// don't each cost a request.
const READ_AHEAD: u64 = 1 << 20;

/// One resource as `Read + Seek`.
pub enum RangeFile {
    Local(std::io::BufReader<std::fs::File>),
    Remote(RemoteFile),
}

/// A resource over HTTP, read by ranges from a blocking thread.
pub struct RemoteFile {
    upstream: Arc<dyn Upstream>,
    url: String,
    headers: Vec<(String, String)>,
    runtime: tokio::runtime::Handle,
    size: u64,
    position: u64,
    /// What was read ahead: from `buffer_at`, these bytes.
    buffer: Vec<u8>,
    buffer_at: u64,
}

impl RangeFile {
    pub fn local(path: &std::path::Path) -> std::io::Result<Self> {
        Ok(Self::Local(std::io::BufReader::with_capacity(1 << 20, std::fs::File::open(path)?)))
    }

    /// `url` read through `upstream`. Called from a blocking thread, as every read is.
    pub fn remote(
        upstream: Arc<dyn Upstream>,
        url: &str,
        headers: Vec<(String, String)>,
        runtime: tokio::runtime::Handle,
    ) -> std::io::Result<Self> {
        let mut file = RemoteFile {
            upstream,
            url: url.to_owned(),
            headers,
            runtime,
            size: 0,
            position: 0,
            buffer: Vec::new(),
            buffer_at: 0,
        };
        file.size = file.fetch(0)?;
        Ok(Self::Remote(file))
    }
}

impl RemoteFile {
    /// Read ahead from `at`, and return the resource's size.
    fn fetch(&mut self, at: u64) -> std::io::Result<u64> {
        let mut headers = self.headers.clone();
        headers.push(("Range".into(), format!("bytes={at}-{}", at + READ_AHEAD - 1)));
        let (upstream, url) = (Arc::clone(&self.upstream), self.url.clone());
        let (size, body) = self.runtime.block_on(async move {
            let response = upstream.get(&url, &headers).await?;
            if !matches!(response.status, 200 | 206) {
                return Err(std::io::Error::other(format!(
                    "upstream answered {}",
                    response.status
                )));
            }
            if response.status == 200 && at > 0 {
                return Err(std::io::Error::other("upstream ignores ranges"));
            }
            let size = response.total_size();
            let want = usize::try_from(READ_AHEAD).unwrap_or(usize::MAX);
            let body = response.prefix(want).await?;
            Ok::<_, std::io::Error>((size, body))
        })?;
        self.buffer = body;
        self.buffer_at = at;
        Ok(size.unwrap_or(at + self.buffer.len() as u64))
    }
}

impl Read for RangeFile {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let file = match self {
            Self::Local(file) => return file.read(out),
            Self::Remote(file) => file,
        };
        if file.position >= file.size || out.is_empty() {
            return Ok(0);
        }
        let end = file.buffer_at + file.buffer.len() as u64;
        if file.position < file.buffer_at || file.position >= end {
            file.fetch(file.position)?;
        }
        let offset = usize::try_from(file.position - file.buffer_at).unwrap_or(usize::MAX);
        let available = file.buffer.get(offset..).unwrap_or_default();
        if available.is_empty() {
            return Ok(0);
        }
        let n = available.len().min(out.len());
        out[..n].copy_from_slice(&available[..n]);
        file.position += n as u64;
        Ok(n)
    }
}

impl Seek for RangeFile {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        let file = match self {
            Self::Local(file) => return file.seek(to),
            Self::Remote(file) => file,
        };
        let position = match to {
            SeekFrom::Start(at) => Some(at),
            SeekFrom::End(delta) => file.size.checked_add_signed(delta),
            SeekFrom::Current(delta) => file.position.checked_add_signed(delta),
        };
        file.position =
            position.ok_or_else(|| std::io::Error::other("a seek before the start"))?;
        Ok(file.position)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use tokio::net::TcpListener;

    use super::*;

    /// A loopback server answering every GET with `body`, with ranges, chunked when asked.
    pub(crate) async fn serve(body: Vec<u8>, chunked: bool) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let body = body.clone();
                tokio::spawn(async move {
                    let mut reader = BufReader::new(socket);
                    let mut range = None;
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                            return;
                        }
                        if line.trim().is_empty() {
                            break;
                        }
                        if let Some(value) =
                            line.to_ascii_lowercase().strip_prefix("range: bytes=")
                        {
                            let (a, b) = value.trim().split_once('-').unwrap();
                            let a: usize = a.parse().unwrap();
                            let b: usize = b
                                .parse()
                                .map_or(body.len() - 1, |b: usize| b.min(body.len() - 1));
                            range = Some((a, b));
                        }
                    }
                    let mut socket = reader.into_inner();
                    let (status, part, extra) = match range {
                        Some((a, b)) => (
                            "206 Partial Content",
                            &body[a..=b],
                            format!("Content-Range: bytes {a}-{b}/{}\r\n", body.len()),
                        ),
                        None => ("200 OK", &body[..], String::new()),
                    };
                    let head = if chunked {
                        format!(
                            "HTTP/1.1 {status}\r\n{extra}Transfer-Encoding: chunked\r\n\r\n"
                        )
                    } else {
                        format!(
                            "HTTP/1.1 {status}\r\n{extra}Content-Length: {}\r\n\r\n",
                            part.len()
                        )
                    };
                    let _ = socket.write_all(head.as_bytes()).await;
                    if chunked {
                        for piece in part.chunks(1000) {
                            let _ = socket
                                .write_all(format!("{:x}\r\n", piece.len()).as_bytes())
                                .await;
                            let _ = socket.write_all(piece).await;
                            let _ = socket.write_all(b"\r\n").await;
                        }
                        let _ = socket.write_all(b"0\r\n\r\n").await;
                    } else {
                        let _ = socket.write_all(part).await;
                    }
                });
            }
        });
        format!("http://{address}/file")
    }

    #[tokio::test]
    async fn plain_http_streams_whole_and_chunked_bodies() {
        let body: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        for chunked in [false, true] {
            let url = serve(body.clone(), chunked).await;
            let response = PlainHttp.get(&url, &[]).await.unwrap();
            assert_eq!(response.status, 200);
            assert_eq!(response.bytes(1 << 20).await.unwrap(), body);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_remote_file_reads_and_seeks_by_ranges() {
        let body: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        let url = serve(body.clone(), false).await;
        let runtime = tokio::runtime::Handle::current();
        let read = tokio::task::spawn_blocking(move || {
            let mut file =
                RangeFile::remote(Arc::new(PlainHttp), &url, Vec::new(), runtime).unwrap();
            let mut tail = [0u8; 10];
            file.seek(SeekFrom::End(-10)).unwrap();
            file.read_exact(&mut tail).unwrap();
            let mut middle = vec![0u8; 2_000_000];
            file.seek(SeekFrom::Start(500_000)).unwrap();
            file.read_exact(&mut middle).unwrap();
            (tail, middle)
        })
        .await
        .unwrap();
        assert_eq!(read.0[..], body[body.len() - 10..]);
        assert_eq!(read.1[..], body[500_000..2_500_000]);
    }
}
