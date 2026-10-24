//! Serves the media API to a parent process over stdin/stdout, one JSON
//! object per line, for hosts that run native code in a separate process.
//!
//! Request:  {"id": 1, "method": "sessions", "params": {}}
//! Response: {"id": 1, "ok": true, "value": [...]}
//!           {"id": 1, "ok": false, "error": {"code": "...", "message": "..."}}
//!
//! Requests are answered in order. The process exits when stdin closes.

mod protocol;

use now_playing_service::MediaService;
use protocol::{Request, Response, handle, parse};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

/// Longer requests are refused; legitimate ones are a few hundred bytes.
const MAX_REQUEST_BYTES: usize = 64 * 1024;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let service = MediaService::new();
    let mut input = BufReader::new(tokio::io::stdin());
    let mut output = tokio::io::stdout();
    let mut line = Vec::new();
    loop {
        line.clear();
        let read = (&mut input)
            .take(MAX_REQUEST_BYTES as u64 + 1)
            .read_until(b'\n', &mut line)
            .await;
        let response = match read {
            Ok(0) | Err(_) => break,
            Ok(_) if line.len() > MAX_REQUEST_BYTES && !line.ends_with(b"\n") => {
                // The rest of the oversized line cannot be trusted to frame anything.
                let response =
                    Response::error(None, "INVALID_ARGUMENT", "The request is too large.");
                let _ = write(&mut output, &response).await;
                break;
            }
            Ok(_) => match parse(&line) {
                Ok(Request { id, method, params }) => handle(&service, id, &method, &params).await,
                Err(response) => response,
            },
        };
        if write(&mut output, &response).await.is_err() {
            break;
        }
    }
}

async fn write(output: &mut tokio::io::Stdout, response: &Response) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec(response)?;
    bytes.push(b'\n');
    output.write_all(&bytes).await?;
    output.flush().await
}
