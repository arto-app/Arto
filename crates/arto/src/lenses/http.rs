//! Asking a server once: the request goes out as a POST, the answer comes
//! back as it is streamed.
//!
//! The request runs on a thread of its own with a blocking client, handing
//! what arrives to the task that waits for it. When that task is dropped —
//! the lens was stopped — the thread's next hand-over fails, and it drops
//! the response, which closes the connection and tells the server to stop.

use super::runner::{complete_text, RunError, MAX_OUTPUT};
use std::io::Read;
use std::time::Duration;
use tokio::sync::mpsc;

/// How much of an error response is kept to say what went wrong.
const MAX_ERROR_BODY: usize = 64 * 1024;

/// POST `body` as JSON to `url` with `headers`, and return the response
/// body. `on_output` is called with everything received so far each time
/// more arrives, cut at the last complete character.
pub(crate) async fn post(
    url: &str,
    headers: Vec<(String, String)>,
    body: String,
    timeout: Duration,
    mut on_output: impl FnMut(&str),
) -> Result<String, RunError> {
    let (sender, mut receiver) = mpsc::channel::<Result<Vec<u8>, RunError>>(16);
    let url = url.to_string();
    std::thread::spawn(move || {
        if let Err(error) = stream(&url, &headers, &body, timeout, &sender) {
            let _ = sender.blocking_send(Err(error));
        }
    });

    let exchange = async {
        let mut output = Vec::new();
        while let Some(chunk) = receiver.recv().await {
            let chunk = chunk?;
            if output.len() + chunk.len() > MAX_OUTPUT {
                return Err(RunError::TooLarge);
            }
            output.extend_from_slice(&chunk);
            on_output(complete_text(&output));
        }
        Ok(output)
    };
    let output = tokio::time::timeout(timeout, exchange)
        .await
        .map_err(|_| RunError::Timeout(timeout))??;
    String::from_utf8(output).map_err(|_| RunError::NotUtf8)
}

/// Send the request and hand the response over as it arrives, until the
/// receiver is gone.
fn stream(
    url: &str,
    headers: &[(String, String)],
    body: &str,
    timeout: Duration,
    sender: &mpsc::Sender<Result<Vec<u8>, RunError>>,
) -> Result<(), RunError> {
    let mut request = ureq::post(url).header("content-type", "application/json");
    for (name, value) in headers {
        request = request.header(name, value);
    }
    let response = request
        .config()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .build()
        .send(body)
        .map_err(|error| match error {
            ureq::Error::Timeout(_) => RunError::Timeout(timeout),
            other => RunError::Unreachable(other.to_string()),
        })?;
    let status = response.status().as_u16();
    let mut reader = response.into_body().into_reader();

    if status >= 400 {
        let mut text = Vec::new();
        let _ = reader
            .by_ref()
            .take(MAX_ERROR_BODY as u64)
            .read_to_end(&mut text);
        return Err(RunError::Status {
            status,
            message: error_message(&String::from_utf8_lossy(&text)),
        });
    }

    let mut chunk = vec![0; 8192];
    loop {
        let read = reader.read(&mut chunk).map_err(|error| {
            // The client gives up on its own at the same deadline, and its
            // timeout reaches here as an I/O error.
            if error.kind() == std::io::ErrorKind::TimedOut || error.to_string().contains("timeout")
            {
                RunError::Timeout(timeout)
            } else {
                RunError::Unreachable(error.to_string())
            }
        })?;
        if read == 0 || sender.blocking_send(Ok(chunk[..read].to_vec())).is_err() {
            return Ok(());
        }
    }
}

/// GET `url` with `headers`, and return the response body: a short answer,
/// read whole on a thread of its own.
pub(crate) async fn get(
    url: &str,
    headers: Vec<(String, String)>,
    timeout: Duration,
) -> Result<String, RunError> {
    let url = url.to_string();
    let exchange = tokio::task::spawn_blocking(move || {
        let mut request = ureq::get(&url);
        for (name, value) in &headers {
            request = request.header(name, value);
        }
        let response = request
            .config()
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .build()
            .call()
            .map_err(|error| match error {
                ureq::Error::Timeout(_) => RunError::Timeout(timeout),
                other => RunError::Unreachable(other.to_string()),
            })?;
        let status = response.status().as_u16();
        let mut body = Vec::new();
        response
            .into_body()
            .into_reader()
            .take(MAX_OUTPUT as u64)
            .read_to_end(&mut body)
            .map_err(|error| RunError::Unreachable(error.to_string()))?;
        let text = String::from_utf8(body).map_err(|_| RunError::NotUtf8)?;
        if status >= 400 {
            return Err(RunError::Status {
                status,
                message: error_message(&text),
            });
        }
        Ok(text)
    });
    exchange
        .await
        .map_err(|error| RunError::Unreachable(error.to_string()))?
}

/// What an error response says: its `error.message` or `error` when it is
/// JSON, as servers of both APIs answer, and the text as it is otherwise.
fn error_message(body: &str) -> String {
    let parsed = serde_json::from_str::<serde_json::Value>(body).ok();
    let message = parsed.as_ref().and_then(|value| {
        value["error"]["message"]
            .as_str()
            .or_else(|| value["error"].as_str())
            .map(str::to_string)
    });
    message.unwrap_or_else(|| body.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    const LONG: Duration = Duration::from_secs(10);

    /// Serve one request on a local port: read it, then write `response`
    /// in `parts`, pausing between them. Returns the URL and what the
    /// request said.
    fn serve(parts: Vec<&'static str>) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/chat", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut head = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
                head.push_str(&line);
                if line == "\r\n" {
                    break;
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            for part in parts {
                let _ = socket.write_all(part.as_bytes());
                let _ = socket.flush();
                std::thread::sleep(Duration::from_millis(100));
            }
            format!("{head}{}", String::from_utf8(body).unwrap())
        });
        (url, handle)
    }

    #[tokio::test]
    async fn the_answer_arrives_as_it_is_streamed() {
        let (url, server) = serve(vec![
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            "data: one\n\n",
            "data: two\n\n",
        ]);
        let mut seen = Vec::new();

        let answer = post(
            &url,
            vec![("authorization".to_string(), "Bearer key".to_string())],
            r#"{"model":"m"}"#.to_string(),
            LONG,
            |text| seen.push(text.to_string()),
        )
        .await
        .unwrap();

        assert_eq!(answer, "data: one\n\ndata: two\n\n");
        assert!(seen.len() >= 2, "{seen:?}");
        assert_eq!(seen.first().map(String::as_str), Some("data: one\n\n"));
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /chat "), "{request}");
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer key"),
            "{request}"
        );
        assert!(request.ends_with(r#"{"model":"m"}"#), "{request}");
    }

    #[tokio::test]
    async fn an_error_response_says_why() {
        let (url, _server) = serve(vec![concat!(
            "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\n",
            "Content-Length: 47\r\nConnection: close\r\n\r\n",
            r#"{"error":{"message":"Incorrect API key given"}}"#,
        )]);

        let error = post(&url, Vec::new(), "{}".to_string(), LONG, |_| {})
            .await
            .unwrap_err();

        match error {
            RunError::Status { status, message } => {
                assert_eq!(status, 401);
                assert_eq!(message, "Incorrect API key given");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_server_that_is_not_there_is_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/chat", listener.local_addr().unwrap());
        drop(listener);

        let error = post(&url, Vec::new(), "{}".to_string(), LONG, |_| {})
            .await
            .unwrap_err();

        assert!(matches!(error, RunError::Unreachable(_)), "{error:?}");
    }

    #[tokio::test]
    async fn a_slow_server_times_out() {
        let (url, _server) = serve(vec![
            "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n",
            "",
            "",
            "",
            "",
            "late",
        ]);

        let error = post(
            &url,
            Vec::new(),
            "{}".to_string(),
            Duration::from_millis(150),
            |_| {},
        )
        .await
        .unwrap_err();

        assert!(matches!(error, RunError::Timeout(_)), "{error:?}");
    }

    #[tokio::test]
    async fn a_get_returns_the_body_and_sends_the_headers() {
        let (url, server) = serve(vec![concat!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n",
            "Content-Length: 11\r\nConnection: close\r\n\r\n",
            r#"{"data":[]}"#,
        )]);

        let body = get(
            &url,
            vec![("authorization".to_string(), "Bearer key".to_string())],
            LONG,
        )
        .await
        .unwrap();

        assert_eq!(body, r#"{"data":[]}"#);
        let request = server.join().unwrap();
        assert!(request.starts_with("GET /chat "), "{request}");
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer key"),
            "{request}"
        );
    }

    #[test]
    fn an_error_message_is_read_from_either_shape() {
        assert_eq!(
            error_message(r#"{"error":{"message":"bad key"}}"#),
            "bad key"
        );
        assert_eq!(
            error_message(r#"{"error":"model not found"}"#),
            "model not found"
        );
        assert_eq!(error_message("  Bad Gateway \n"), "Bad Gateway");
    }
}
