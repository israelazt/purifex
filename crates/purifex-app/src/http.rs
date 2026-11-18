//! Just enough HTTP/1.1 for a page served to the same computer: reading a request
//! from bytes and writing a response. Pure, so it is tested without a socket.
//! Anything unusual is refused (big headers, big bodies, odd request lines).

/// The most the head of a request (line and headers) may take.
const MAX_HEAD: usize = 16 * 1024;
/// The most a body may take. Nothing here sends one yet.
const MAX_BODY: usize = 64 * 1024;
const MAX_HEADERS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    /// The path, without the query. Taken literally (no decoding).
    pub path: String,
    pub query: Vec<(String, String)>,
    /// Names in lowercase.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn query(&self, name: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parsed {
    /// More bytes are needed.
    Incomplete,
    Done(Request),
    /// Not a request this program accepts, with the reason.
    Bad(String),
}

pub fn parse_request(buffer: &[u8]) -> Parsed {
    let Some(head_end) = find(buffer, b"\r\n\r\n") else {
        return if buffer.len() > MAX_HEAD {
            Parsed::Bad("cabecera demasiado grande".into())
        } else {
            Parsed::Incomplete
        };
    };
    if head_end > MAX_HEAD {
        return Parsed::Bad("cabecera demasiado grande".into());
    }
    let Ok(head) = std::str::from_utf8(&buffer[..head_end]) else {
        return Parsed::Bad("la cabecera no es texto".into());
    };
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Parsed::Bad("línea de petición no válida".into());
    };
    if !version.starts_with("HTTP/1.") || method.is_empty() || !target.starts_with('/') {
        return Parsed::Bad("línea de petición no válida".into());
    }
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path, parse_query(query)),
        None => (target, Vec::new()),
    };
    let mut headers = Vec::new();
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return Parsed::Bad("cabecera mal formada".into());
        };
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        if headers.len() > MAX_HEADERS {
            return Parsed::Bad("demasiadas cabeceras".into());
        }
    }
    let length = match headers.iter().find(|(n, _)| n == "content-length") {
        Some((_, value)) => match value.parse::<usize>() {
            Ok(n) if n <= MAX_BODY => n,
            _ => return Parsed::Bad("cuerpo demasiado grande o no válido".into()),
        },
        None => 0,
    };
    let body_start = head_end + 4;
    if buffer.len() < body_start + length {
        return Parsed::Incomplete;
    }
    Parsed::Done(Request {
        method: method.to_string(),
        path: path.to_string(),
        query,
        headers,
        body: buffer[body_start..body_start + length].to_vec(),
    })
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((name, value)) => (percent_decode(name), percent_decode(value)),
            None => (percent_decode(pair), String::new()),
        })
        .collect()
}

/// `%41` → `A`, `+` → space. A bad escape is left as it is.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len()
                && hex(bytes[i + 1]).is_some()
                && hex(bytes[i + 2]).is_some() =>
            {
                out.push(hex(bytes[i + 1]).unwrap_or(0) * 16 + hex(bytes[i + 2]).unwrap_or(0));
                i += 2;
            }
            other => out.push(other),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(byte: u8) -> Option<u8> {
    (byte as char).to_digit(16).map(|d| d as u8)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
    /// The request was an authorized one from the page: a sign that it is open.
    pub alive: bool,
}

impl Response {
    pub fn new(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            status,
            content_type: content_type.into(),
            body,
            alive: false,
        }
    }

    pub fn json(status: u16, body: String) -> Self {
        Self::new(status, "application/json; charset=utf-8", body.into_bytes())
    }

    pub fn error(status: u16, message: &str) -> Self {
        Self::json(status, serde_json::json!({ "error": message }).to_string())
    }

    fn reason(&self) -> &'static str {
        match self.status {
            200 => "OK",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            408 => "Request Timeout",
            413 => "Content Too Large",
            _ => "Internal Server Error",
        }
    }

    /// The bytes to send, with the headers that keep another web page from
    /// reading or framing this one, and the connection closed after it.
    pub fn to_bytes(&self) -> Vec<u8> {
        let head = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nCross-Origin-Resource-Policy: same-origin\r\nX-Frame-Options: DENY\r\nContent-Security-Policy: default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; frame-ancestors 'none'\r\nConnection: close\r\n\r\n",
            self.status,
            self.reason(),
            self.content_type,
            self.body.len()
        );
        let mut bytes = head.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn done(text: &str) -> Request {
        match parse_request(text.as_bytes()) {
            Parsed::Done(request) => request,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_simple_request_is_read() {
        let r =
            done("GET /api/ping HTTP/1.1\r\nHost: 127.0.0.1:5000\r\nX-Purifex-Token: abc\r\n\r\n");
        assert_eq!((r.method.as_str(), r.path.as_str()), ("GET", "/api/ping"));
        assert_eq!(r.header("host"), Some("127.0.0.1:5000"));
        assert_eq!(r.header("x-purifex-token"), Some("abc"));
        assert!(r.body.is_empty());
    }

    #[test]
    fn the_query_is_split_and_decoded() {
        let r = done("GET /api/analysis?id=lol-1&x=a%20b+c&flag HTTP/1.1\r\n\r\n");
        assert_eq!(r.path, "/api/analysis");
        assert_eq!(r.query("id"), Some("lol-1"));
        assert_eq!(r.query("x"), Some("a b c"));
        assert_eq!(r.query("flag"), Some(""));
        assert_eq!(r.query("nope"), None);
    }

    #[test]
    fn a_bad_escape_is_left_as_it_is() {
        let r = done("GET /a?x=100%&y=%zz HTTP/1.1\r\n\r\n");
        assert_eq!((r.query("x"), r.query("y")), (Some("100%"), Some("%zz")));
    }

    #[test]
    fn a_request_that_is_still_arriving_is_incomplete() {
        assert_eq!(
            parse_request(b"GET /a HTTP/1.1\r\nHost: x"),
            Parsed::Incomplete
        );
        assert_eq!(parse_request(b""), Parsed::Incomplete);
        let partial = b"POST /a HTTP/1.1\r\nContent-Length: 5\r\n\r\nab";
        assert_eq!(parse_request(partial), Parsed::Incomplete);
        let whole = b"POST /a HTTP/1.1\r\nContent-Length: 5\r\n\r\nabcde";
        assert!(matches!(parse_request(whole), Parsed::Done(r) if r.body == b"abcde"));
    }

    #[test]
    fn what_is_not_a_request_this_program_accepts_is_refused() {
        for bad in [
            "GET\r\n\r\n",
            "GET /a\r\n\r\n",
            "GET a HTTP/1.1\r\n\r\n",
            "GET /a SPDY/3\r\n\r\n",
            "GET /a HTTP/1.1 extra\r\n\r\n",
            "GET /a HTTP/1.1\r\nno-colon\r\n\r\n",
            "POST /a HTTP/1.1\r\nContent-Length: 99999999\r\n\r\n",
            "POST /a HTTP/1.1\r\nContent-Length: x\r\n\r\n",
        ] {
            assert!(
                matches!(parse_request(bad.as_bytes()), Parsed::Bad(_)),
                "{bad:?}"
            );
        }
        let huge = vec![b'a'; MAX_HEAD + 10];
        assert!(matches!(parse_request(&huge), Parsed::Bad(_)));
    }

    #[test]
    fn a_response_carries_its_length_and_the_protective_headers() {
        let text = String::from_utf8(Response::json(200, "{}".into()).to_bytes()).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Content-Length: 2\r\n"));
        assert!(text.contains("Content-Security-Policy: default-src 'self'"));
        assert!(text.contains("X-Content-Type-Options: nosniff"));
        assert!(text.contains("Cache-Control: no-store"));
        assert!(text.ends_with("\r\n\r\n{}"));
    }

    #[test]
    fn an_error_response_says_what_went_wrong_in_json() {
        let r = Response::error(401, "falta el token");
        assert_eq!(r.status, 401);
        let value: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(value["error"], "falta el token");
    }
}
