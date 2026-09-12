//! Chunked HTTP writes for bodies above the 4 KB a single WASI write takes.
//!
//! A COPY of `wasi-examples/near-email/wasi-near-email-ark/src/http_chunked.rs`.
//! Attachments are why it exists.
use std::time::Duration;
use wasi::http::{
    outgoing_handler,
    types::{
        Headers, Method, OutgoingBody, OutgoingRequest, RequestOptions, Scheme,
    },
};

/// HTTP response from chunked request
pub struct ChunkedResponse {
    status: u16,
    body: Vec<u8>,
}

impl ChunkedResponse {
    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

/// Send HTTP POST with large body using chunked writes
///
/// This bypasses the 4KB limit of `blocking_write_and_flush` by using
/// manual chunked writes with `check_write`, `write`, and `flush`.
pub fn post_chunked(
    url: &str,
    content_type: &str,
    body: &[u8],
    timeout: Duration,
    api_secret: Option<&str>,
) -> Result<ChunkedResponse, Box<dyn std::error::Error>> {
    eprintln!("[CHUNKED] post_chunked: url={}, body_len={}", url, body.len());

    // Parse URL
    let parsed_url = url::Url::parse(url)?;

    // Create headers
    let headers = Headers::new();
    headers.set(&"Content-Type".to_string(), &[content_type.as_bytes().to_vec()])?;
    headers.set(&"Content-Length".to_string(), &[body.len().to_string().into_bytes()])?;

    // Add API secret header if provided
    if let Some(secret) = api_secret {
        headers.set(&"X-API-Secret".to_string(), &[secret.as_bytes().to_vec()])?;
    }

    // Create request
    let req = OutgoingRequest::new(headers);
    req.set_method(&Method::Post)
        .map_err(|()| "failed to set method")?;

    let scheme = match parsed_url.scheme() {
        "http" => Scheme::Http,
        "https" => Scheme::Https,
        other => Scheme::Other(other.to_string()),
    };
    req.set_scheme(Some(&scheme))
        .map_err(|()| "failed to set scheme")?;

    req.set_authority(Some(parsed_url.authority()))
        .map_err(|()| "failed to set authority")?;

    let path = match parsed_url.query() {
        Some(query) => format!("{}?{}", parsed_url.path(), query),
        None => parsed_url.path().to_string(),
    };
    req.set_path_with_query(Some(&path))
        .map_err(|()| "failed to set path")?;

    // Get outgoing body
    let outgoing_body = req.body()
        .map_err(|_| "failed to get outgoing body")?;

    // Get write stream
    let output_stream = outgoing_body.write()
        .map_err(|_| "failed to get output stream")?;

    // Write body in chunks using the proper pattern
    let pollable = output_stream.subscribe();
    let mut remaining = body;
    let mut total_written = 0usize;

    eprintln!("[CHUNKED] starting chunked write loop, total={} bytes", body.len());

    while !remaining.is_empty() {
        // Wait for stream to be ready
        pollable.block();

        // Check how many bytes we can write
        let permitted = output_stream.check_write()
            .map_err(|e| format!("check_write failed: {:?}", e))?;

        if permitted == 0 {
            eprintln!("[CHUNKED] check_write returned 0, waiting...");
            continue;
        }

        // Write up to permitted bytes
        let chunk_size = std::cmp::min(permitted as usize, remaining.len());
        let chunk = &remaining[..chunk_size];

        output_stream.write(chunk)
            .map_err(|e| format!("write failed: {:?}", e))?;

        total_written += chunk_size;
        remaining = &remaining[chunk_size..];

        eprintln!("[CHUNKED] wrote {} bytes, total={}/{}", chunk_size, total_written, body.len());
    }

    eprintln!("[CHUNKED] all data written, flushing...");

    // Flush the stream
    output_stream.flush()
        .map_err(|e| format!("flush failed: {:?}", e))?;

    // Wait for flush to complete
    pollable.block();

    // Check for flush errors
    output_stream.check_write()
        .map_err(|e| format!("post-flush check_write failed: {:?}", e))?;

    eprintln!("[CHUNKED] flush complete");

    // Must drop pollable first, then output stream before finishing body
    eprintln!("[CHUNKED] dropping pollable...");
    drop(pollable);
    eprintln!("[CHUNKED] dropping output_stream...");
    drop(output_stream);
    eprintln!("[CHUNKED] output_stream dropped");

    // Finish the body
    eprintln!("[CHUNKED] calling OutgoingBody::finish...");
    match OutgoingBody::finish(outgoing_body, None) {
        Ok(()) => eprintln!("[CHUNKED] OutgoingBody::finish OK"),
        Err(e) => {
            eprintln!("[CHUNKED] OutgoingBody::finish FAILED: {:?}", e);
            return Err(format!("OutgoingBody::finish failed: {:?}", e).into());
        }
    }

    eprintln!("[CHUNKED] body finished, sending request...");

    // Set options
    let options = RequestOptions::new();
    options.set_connect_timeout(Some(timeout.as_nanos() as u64))
        .map_err(|()| "failed to set timeout")?;

    // Send request
    eprintln!("[CHUNKED] calling outgoing_handler::handle...");
    let future_response = match outgoing_handler::handle(req, Some(options)) {
        Ok(fr) => {
            eprintln!("[CHUNKED] outgoing_handler::handle OK");
            fr
        }
        Err(e) => {
            eprintln!("[CHUNKED] outgoing_handler::handle FAILED: {:?}", e);
            return Err(format!("outgoing_handler::handle failed: {:?}", e).into());
        }
    };

    // Wait for response
    let incoming_response = match future_response.get() {
        Some(result) => result.map_err(|()| "response already taken")?,
        None => {
            let pollable = future_response.subscribe();
            pollable.block();
            future_response.get()
                .ok_or("no response after blocking")?
                .map_err(|()| "response already taken")?
        }
    }?;

    drop(future_response);

    eprintln!("[CHUNKED] got response");

    // Read response
    let status = incoming_response.status();
    eprintln!("[CHUNKED] response status={}", status);

    // Read response body
    let response_body = incoming_response.consume()
        .map_err(|_| "failed to consume response body")?;

    let input_stream = response_body.stream()
        .map_err(|_| "failed to get response stream")?;

    let mut response_data = Vec::new();
    let response_pollable = input_stream.subscribe();

    loop {
        response_pollable.block();

        match input_stream.read(64 * 1024) {
            Ok(chunk) => {
                if chunk.is_empty() {
                    break;
                }
                response_data.extend_from_slice(&chunk);
            }
            Err(wasi::io::streams::StreamError::Closed) => {
                break;
            }
            Err(e) => {
                return Err(format!("read error: {:?}", e).into());
            }
        }
    }

    eprintln!("[CHUNKED] response body read, len={}", response_data.len());

    Ok(ChunkedResponse {
        status,
        body: response_data,
    })
}
