//! LSP's framing: a header block, a blank line, then exactly that many bytes.
//!
//! ```text
//! Content-Length: 42\r\n
//! \r\n
//! {"jsonrpc":"2.0",...}
//! ```
//!
//! Borrowed rather than invented, and borrowed whole: the reason to speak
//! LSP at all is that editors already have client machinery for it, and
//! machinery that expects `Content-Length` is not helped by a protocol that
//! is JSON-RPC over newlines instead.

use std::io::{BufRead, Read, Write};

/// Header naming the body's length in bytes. Matched case-insensitively,
/// as LSP requires.
const CONTENT_LENGTH: &str = "content-length";

/// Refuse a header claiming a body larger than this.
///
/// A local socket carrying a request to open a file has no business naming
/// megabytes, and a length read off the wire is otherwise an invitation to
/// allocate whatever a confused — or hostile — writer asks for.
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// Refuse a header block longer than this, counting every line of it.
///
/// The body has a length to check before anything is allocated; a header
/// block has none, and is read into a buffer that grows a line at a time.
/// Without a cap, a peer that never sends a newline — or sends header lines
/// forever — grows that buffer for as long as it keeps the socket alive.
const MAX_HEADER_BYTES: usize = 8 * 1024;

/// Read one framed message body.
///
/// Returns `None` at a clean end of stream. Headers other than the length
/// are read and ignored, which is what LSP asks of a reader.
pub fn read_message<R: BufRead>(reader: &mut R) -> std::io::Result<Option<Vec<u8>>> {
    let Some(length) = read_headers(reader)? else {
        return Ok(None);
    };

    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

/// Read the header block and answer with the body length it declared.
fn read_headers<R: BufRead>(reader: &mut R) -> std::io::Result<Option<usize>> {
    let mut length = None;
    let mut started = false;
    let mut budget = MAX_HEADER_BYTES;

    loop {
        if budget == 0 {
            return Err(oversized_header());
        }

        let mut line = String::new();
        let read = (&mut *reader).take(budget as u64).read_line(&mut line)?;
        if read == 0 {
            // A stream that ends between messages ended cleanly; one that
            // ends mid-header did not.
            return if started {
                Err(unexpected_eof("stream ended inside a header block"))
            } else {
                Ok(None)
            };
        }
        started = true;
        budget -= read;

        if !line.ends_with('\n') {
            // The line stopped without ending: either the peer hung up in
            // the middle of it, or it ran past what a header block may be.
            return Err(if budget == 0 {
                oversized_header()
            } else {
                unexpected_eof("stream ended inside a header line")
            });
        }

        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            return match length {
                Some(length) => Ok(Some(length)),
                None => Err(invalid_data("header block declared no Content-Length")),
            };
        }

        let Some((name, value)) = line.split_once(':') else {
            return Err(invalid_data(format!("header line has no colon: {line:?}")));
        };
        if !name.trim().eq_ignore_ascii_case(CONTENT_LENGTH) {
            continue;
        }

        let declared: usize = value
            .trim()
            .parse()
            .map_err(|_| invalid_data(format!("Content-Length is not a number: {value:?}")))?;
        if declared > MAX_BODY_BYTES {
            return Err(invalid_data(format!(
                "Content-Length {declared} exceeds the {MAX_BODY_BYTES} byte limit"
            )));
        }
        length = Some(declared);
    }
}

/// Write one framed message.
///
/// Header and body go out in one `write_all`: the socket is unbuffered, and
/// `write!` would hand it each piece of the format string separately.
pub fn write_message<W: Write>(writer: &mut W, body: &[u8]) -> std::io::Result<()> {
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut framed = Vec::with_capacity(header.len() + body.len());
    framed.extend_from_slice(header.as_bytes());
    framed.extend_from_slice(body);
    writer.write_all(&framed)?;
    writer.flush()
}

fn invalid_data(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into())
}

fn unexpected_eof(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::UnexpectedEof, message)
}

fn oversized_header() -> std::io::Error {
    invalid_data(format!(
        "header block exceeds the {MAX_HEADER_BYTES} byte limit"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    fn framed(body: &str) -> Vec<u8> {
        let mut out = Vec::new();
        write_message(&mut out, body.as_bytes()).unwrap();
        out
    }

    #[test]
    fn a_message_is_written_with_its_length_in_bytes() {
        assert_eq!(
            String::from_utf8(framed(r#"{"a":1}"#)).unwrap(),
            "Content-Length: 7\r\n\r\n{\"a\":1}"
        );
    }

    #[test]
    fn the_length_counts_bytes_and_not_characters() {
        // The document a request names can be anything the file system
        // allows, so a path of Japanese characters has to survive the trip.
        let body = r#"{"path":"読み物.md"}"#;
        assert!(body.len() > body.chars().count());

        let written = framed(body);
        assert!(String::from_utf8_lossy(&written)
            .starts_with(&format!("Content-Length: {}\r\n", body.len())));

        let mut reader = BufReader::new(std::io::Cursor::new(written));
        assert_eq!(read_message(&mut reader).unwrap().unwrap(), body.as_bytes());
    }

    #[test]
    fn messages_round_trip_back_to_back() {
        let mut stream = framed(r#"{"first":true}"#);
        stream.extend(framed(r#"{"second":true}"#));

        let mut reader = BufReader::new(std::io::Cursor::new(stream));
        assert_eq!(
            read_message(&mut reader).unwrap().unwrap(),
            br#"{"first":true}"#
        );
        assert_eq!(
            read_message(&mut reader).unwrap().unwrap(),
            br#"{"second":true}"#
        );
        assert_eq!(read_message(&mut reader).unwrap(), None);
    }

    #[test]
    fn headers_beside_the_length_are_read_and_ignored() {
        let stream = "Content-Type: application/vscode-jsonrpc; charset=utf-8\r\n\
                      Content-Length: 2\r\n\
                      \r\n\
                      {}";
        let mut reader = BufReader::new(std::io::Cursor::new(stream));
        assert_eq!(read_message(&mut reader).unwrap().unwrap(), b"{}");
    }

    #[test]
    fn the_length_header_is_matched_whatever_its_case() {
        let mut reader = BufReader::new(std::io::Cursor::new("content-length: 2\r\n\r\n{}"));
        assert_eq!(read_message(&mut reader).unwrap().unwrap(), b"{}");
    }

    #[test]
    fn a_header_block_without_a_length_is_refused() {
        let mut reader = BufReader::new(std::io::Cursor::new("Content-Type: text/plain\r\n\r\n{}"));
        assert!(read_message(&mut reader).is_err());
    }

    #[test]
    fn a_length_that_is_not_a_number_is_refused() {
        let mut reader = BufReader::new(std::io::Cursor::new("Content-Length: lots\r\n\r\n{}"));
        assert!(read_message(&mut reader).is_err());
    }

    #[test]
    fn a_length_beyond_the_limit_is_refused_before_anything_is_allocated() {
        let stream = format!("Content-Length: {}\r\n\r\n", MAX_BODY_BYTES + 1);
        let mut reader = BufReader::new(std::io::Cursor::new(stream));
        assert!(read_message(&mut reader).is_err());
    }

    #[test]
    fn a_stream_that_ends_between_messages_ended_cleanly() {
        let mut reader = BufReader::new(std::io::Cursor::new(""));
        assert_eq!(read_message(&mut reader).unwrap(), None);
    }

    #[test]
    fn a_stream_that_ends_inside_a_header_did_not() {
        let mut reader = BufReader::new(std::io::Cursor::new("Content-Length: 2\r\n"));
        assert!(read_message(&mut reader).is_err());
    }

    #[test]
    fn a_stream_that_ends_after_a_header_but_before_the_length_did_not() {
        // The block was started and never finished, which is a truncated
        // message and not a connection that closed between messages.
        let mut reader = BufReader::new(std::io::Cursor::new("Content-Type: text/plain\r\n"));
        assert!(read_message(&mut reader).is_err());
    }

    #[test]
    fn a_header_line_that_never_ends_is_refused_before_it_can_grow() {
        // Nothing bounds a header the way Content-Length bounds a body, so
        // a peer that writes forever without a newline must be cut off.
        let stream = "X: ".to_string() + &"y".repeat(MAX_HEADER_BYTES * 2);
        let mut reader = BufReader::new(std::io::Cursor::new(stream));
        assert!(read_message(&mut reader).is_err());
    }

    #[test]
    fn a_header_block_of_endless_lines_is_refused_too() {
        let stream = "X: y\r\n".repeat(MAX_HEADER_BYTES);
        let mut reader = BufReader::new(std::io::Cursor::new(stream));
        assert!(read_message(&mut reader).is_err());
    }

    #[test]
    fn a_body_shorter_than_its_header_claims_is_an_error() {
        let mut reader = BufReader::new(std::io::Cursor::new("Content-Length: 99\r\n\r\n{}"));
        assert!(read_message(&mut reader).is_err());
    }
}
