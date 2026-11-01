use std::fmt::Write as _;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use http::{Method, Request, StatusCode};
use http_body_util::{BodyExt, Full};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::{GpaMcpServer, McpBridge};

/// Spawns the embedded MCP Streamable HTTP server on `127.0.0.1:<port>` inside the global Tokio runtime.
pub fn spawn_mcp_http_server(bridge: McpBridge) {
    let port = bridge.port;
    gpa_sys_utils::runtime().spawn(async move {
        if let Err(err) = run_http_server(bridge, port).await {
            log::error!("Embedded MCP HTTP server on 127.0.0.1:{port} failed: {err}");
        }
    });
}

/// Binds `127.0.0.1:<port>` (or `0` for ephemeral test ports) and returns the bound `SocketAddr`
/// after spawning the accept loop onto the current Tokio runtime.
///
/// # Errors
/// Returns an `std::io::Error` if binding the TCP listener fails.
pub async fn bind_and_spawn_http_server(
    bridge: McpBridge,
    port: u16,
) -> std::io::Result<SocketAddr> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    let local_addr = listener.local_addr()?;

    let bridge_for_factory = bridge.clone();
    let session_mgr = Arc::new(LocalSessionManager::default());
    let config = StreamableHttpServerConfig::default();
    let mcp_service = Arc::new(StreamableHttpService::new(
        move || Ok(GpaMcpServer::new(bridge_for_factory.clone())),
        session_mgr,
        config,
    ));

    log::info!(
        "GPA MCP Server listening on http://{local_addr}/mcp (status: http://{local_addr}/status)"
    );

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _peer)) => {
                    let svc = mcp_service.clone();
                    let br = bridge.clone();
                    tokio::spawn(async move {
                        if let Err(err) = handle_connection(stream, svc, br).await {
                            log::debug!("MCP HTTP connection closed: {err}");
                        }
                    });
                }
                Err(err) => {
                    log::warn!("MCP HTTP accept error: {err}");
                }
            }
        }
    });

    Ok(local_addr)
}

async fn run_http_server(bridge: McpBridge, port: u16) -> std::io::Result<()> {
    let _addr = bind_and_spawn_http_server(bridge, port).await?;
    std::future::pending::<()>().await;
    Ok(())
}

#[allow(clippy::too_many_lines)]
async fn handle_connection(
    mut stream: TcpStream,
    mcp_service: Arc<StreamableHttpService<GpaMcpServer, LocalSessionManager>>,
    bridge: McpBridge,
) -> anyhow::Result<()> {
    loop {
        let Some(req) = read_http_request(&mut stream).await? else {
            return Ok(());
        };

        let method = req.method().clone();
        let path = req.uri().path().to_string();
        let wants_close = req
            .headers()
            .get(http::header::CONNECTION)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|c| c.eq_ignore_ascii_case("close"));

        // Handle CORS preflight
        if method == Method::OPTIONS {
            let resp = concat!(
                "HTTP/1.1 204 No Content\r\n",
                "Access-Control-Allow-Origin: *\r\n",
                "Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\n",
                "Access-Control-Allow-Headers: Content-Type, Accept, Mcp-Session-Id, Last-Event-ID\r\n",
                "Access-Control-Expose-Headers: Mcp-Session-Id\r\n",
                "Content-Length: 0\r\n",
                "Connection: keep-alive\r\n\r\n"
            );
            stream.write_all(resp.as_bytes()).await?;
            stream.flush().await?;
            if wants_close {
                return Ok(());
            }
            continue;
        }

        // Human/health status endpoint
        if method == Method::GET && (path == "/status" || path == "/health") {
            let snap = bridge.snapshot.load();
            let body_json = serde_json::to_vec_pretty(&json!({
                "status": "ok",
                "server": "gpa-icons-animator-mcp",
                "mcp_endpoint": format!("http://127.0.0.1:{}/mcp", bridge.port),
                "active_icon": snap.document.name,
                "active_state": snap.active_state,
                "states_count": snap.document.states.len(),
                "shapes_count": snap.document.shapes.len(),
                "playing": snap.playing,
            }))?;
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
                body_json.len()
            );
            stream.write_all(header.as_bytes()).await?;
            stream.write_all(&body_json).await?;
            stream.flush().await?;
            return Ok(());
        }

        let response = mcp_service.handle(req).await;
        let (parts, mut body) = response.into_parts();
        let status = parts.status;
        let reason = status.canonical_reason().unwrap_or("OK");

        let is_event_stream = parts
            .headers
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.contains("text/event-stream"));

        if is_event_stream {
            let mut hdr = format!("HTTP/1.1 {} {reason}\r\n", status.as_u16());
            for (k, v) in &parts.headers {
                if let Ok(val) = v.to_str() {
                    let _ = write!(hdr, "{}: {val}\r\n", k.as_str());
                }
            }
            hdr.push_str("Access-Control-Allow-Origin: *\r\n");
            hdr.push_str("Access-Control-Expose-Headers: Mcp-Session-Id\r\n");
            hdr.push_str("Cache-Control: no-cache\r\n");
            hdr.push_str("Connection: close\r\n\r\n");
            stream.write_all(hdr.as_bytes()).await?;
            stream.flush().await?;

            while let Some(Ok(frame)) = body.frame().await {
                if let Some(data) = frame.data_ref()
                    && !data.is_empty()
                {
                    stream.write_all(data).await?;
                    stream.flush().await?;
                }
            }
            return Ok(());
        }

        let collected = body.collect().await?.to_bytes();
        let mut hdr = format!("HTTP/1.1 {} {reason}\r\n", status.as_u16());
        for (k, v) in &parts.headers {
            if k != http::header::CONTENT_LENGTH
                && k != http::header::TRANSFER_ENCODING
                && let Ok(val) = v.to_str()
            {
                let _ = write!(hdr, "{}: {val}\r\n", k.as_str());
            }
        }
        let _ = write!(hdr, "Content-Length: {}\r\n", collected.len());
        hdr.push_str("Access-Control-Allow-Origin: *\r\n");
        hdr.push_str("Access-Control-Expose-Headers: Mcp-Session-Id\r\n");
        if wants_close {
            hdr.push_str("Connection: close\r\n\r\n");
        } else {
            hdr.push_str("Connection: keep-alive\r\n\r\n");
        }
        stream.write_all(hdr.as_bytes()).await?;
        stream.write_all(&collected).await?;
        stream.flush().await?;

        if wants_close {
            return Ok(());
        }
    }
}

async fn read_http_request(
    stream: &mut TcpStream,
) -> anyhow::Result<Option<Request<Full<Bytes>>>> {
    let mut buf = Vec::with_capacity(4096);
    let mut temp = [0_u8; 4096];
    let header_end = loop {
        if let Some(pos) = find_double_crlf(&buf) {
            break pos;
        }
        let n = stream.read(&mut temp).await?;
        if n == 0 {
            if buf.is_empty() {
                return Ok(None);
            }
            anyhow::bail!("unexpected EOF while reading HTTP headers");
        }
        buf.extend_from_slice(&temp[..n]);
        if buf.len() > 256 * 1024 {
            anyhow::bail!("HTTP header too large");
        }
    };

    let header_bytes = &buf[..header_end];
    let header_str = std::str::from_utf8(header_bytes)?;
    let mut lines = header_str.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing request line"))?;
    let mut parts = request_line.split_whitespace();
    let method_str = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing HTTP method"))?;
    let uri_str = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing HTTP URI"))?;

    let mut builder = Request::builder().method(method_str).uri(uri_str);
    let mut content_length = 0_usize;

    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            let k = name.trim();
            let v = value.trim();
            if k.eq_ignore_ascii_case("content-length") {
                content_length = v.parse::<usize>().unwrap_or(0);
            }
            builder = builder.header(k, v);
        }
    }

    let body_start = header_end + 4;
    let mut body_bytes = if buf.len() > body_start {
        buf[body_start..].to_vec()
    } else {
        Vec::with_capacity(content_length)
    };

    while body_bytes.len() < content_length {
        let n = stream.read(&mut temp).await?;
        if n == 0 {
            break;
        }
        body_bytes.extend_from_slice(&temp[..n]);
    }
    if body_bytes.len() > content_length {
        body_bytes.truncate(content_length);
    }

    // Ensure Accept header includes both application/json and text/event-stream if client omitted one,
    // making curl / custom MCP clients work effortlessly alongside strict MCP clients.
    let mut req = builder.body(Full::new(Bytes::from(body_bytes)))?;
    let needs_accept_upgrade = req
        .headers()
        .get(http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_none_or(|a| {
            req.method() == Method::POST
                && (!a.contains("application/json") || !a.contains("text/event-stream"))
        });
    if needs_accept_upgrade && req.method() == Method::POST {
        req.headers_mut().insert(
            http::header::ACCEPT,
            http::HeaderValue::from_static("application/json, text/event-stream"),
        );
    }

    let _ = StatusCode::OK;
    Ok(Some(req))
}

fn find_double_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}
