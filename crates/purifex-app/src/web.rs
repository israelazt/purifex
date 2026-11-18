//! The local page: what it may ask and what it is answered. The page is served
//! to the same computer, but any web page open in a browser could try to talk to
//! a program listening on it, so every request is checked before it is served:
//! the `Host` must be ours (against DNS rebinding) and the API needs the token
//! of this run, which only the page that was opened with it knows.
//!
//! This is the whole logic of the server; the program only moves bytes between a
//! socket and [`Gateway::handle`].

use purifex_domain::session_id::is_valid_id;

use crate::http::{Request, Response};
use crate::services::Services;

/// The JSON text of a value, or the reason it could not be written.
macro_rules! to_json {
    ($value:expr) => {
        serde_json::to_string(&$value).map_err(|e| e.to_string())
    };
}

/// A file of the page, from whoever holds the assets.
#[derive(Debug, Clone, Copy)]
pub struct Asset {
    pub content_type: &'static str,
    pub body: &'static [u8],
}

pub struct Gateway {
    token: String,
    host: String,
}

impl Gateway {
    /// `token` is this run's secret; `port` where the program listens.
    pub fn new(token: &str, port: u16) -> Self {
        Self {
            token: token.to_string(),
            host: format!("127.0.0.1:{port}"),
        }
    }

    pub fn handle(
        &self,
        services: &mut dyn Services,
        request: &Request,
        assets: &dyn Fn(&str) -> Option<Asset>,
    ) -> Response {
        if request.header("host") != Some(self.host.as_str()) {
            return Response::error(403, "host no permitido");
        }
        if request.method != "GET" {
            return Response::error(405, "solo se admite GET");
        }
        if request.path.starts_with("/api/") {
            return self.api(services, request);
        }
        match assets(&request.path) {
            Some(asset) => Response::new(200, asset.content_type, asset.body.to_vec()),
            None => Response::error(404, "no existe"),
        }
    }

    fn api(&self, services: &mut dyn Services, request: &Request) -> Response {
        let given = request.header("x-purifex-token").unwrap_or("");
        if !same_secret(given, &self.token) {
            return Response::error(401, "falta el token o no es válido");
        }
        let mut response = self.route(services, request);
        response.alive = true;
        response
    }

    fn route(&self, services: &mut dyn Services, request: &Request) -> Response {
        let json = |result: Result<String, String>| match result {
            Ok(body) => Response::json(200, body),
            Err(error) => Response::error(500, &error),
        };
        match request.path.as_str() {
            "/api/ping" => Response::json(200, r#"{"ok":true}"#.into()),
            "/api/overview" => json(services.overview().and_then(|v| to_json!(v))),
            "/api/drivers" => json(services.drivers().and_then(|v| to_json!(v))),
            "/api/sessions" => json(
                services
                    .sessions()
                    .map(|rows| serde_json::json!({ "sessions": rows }))
                    .and_then(|v| to_json!(v)),
            ),
            "/api/analysis" => {
                let id = request.query("id").filter(|id| !id.is_empty());
                if let Some(id) = id.filter(|id| !is_valid_id(id)) {
                    return Response::error(
                        400,
                        &format!("«{id}» no es un nombre de grabación válido"),
                    );
                }
                json(services.analyze(id).and_then(|v| to_json!(v)))
            }
            _ => Response::error(404, "no existe"),
        }
    }
}

/// Compares two secrets in the same time whatever they have in common.
fn same_secret(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut difference = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        difference |= usize::from(a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0));
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::Archive;
    use crate::http::{parse_request, Parsed};
    use crate::recorder::Recorder;
    use crate::scanner::Scanner;
    use crate::services::App;
    use crate::testing::{FakeArchive, FakeClock, FakeFile, FakeInventory, FakeSampler};

    const CSV: &str = "t_s,cpu_total,cpu_max_core\n1.0,10.0,20.0\n2.0,11.0,21.0\n";

    type TestApp = App<FakeInventory, FakeClock, FakeSampler, FakeClock, FakeFile, FakeArchive>;

    fn app() -> TestApp {
        App::new(
            Scanner::new(FakeInventory::with(vec![]), FakeClock::ticking(1)),
            Recorder::new(
                FakeSampler::new(0, 3, true),
                FakeClock::ticking(0),
                FakeFile::default(),
            ),
            Archive::new(FakeArchive::with(&[("lol-20261118-165329", CSV)])),
        )
    }

    fn request(text: &str) -> Request {
        match parse_request(text.as_bytes()) {
            Parsed::Done(r) => r,
            other => panic!("{other:?}"),
        }
    }

    fn assets(path: &str) -> Option<Asset> {
        match path {
            "/" | "/index.html" => Some(Asset {
                content_type: "text/html",
                body: b"<html>",
            }),
            _ => None,
        }
    }

    fn get(path: &str, token: Option<&str>, host: &str) -> Response {
        let token = token
            .map(|t| format!("X-Purifex-Token: {t}\r\n"))
            .unwrap_or_default();
        let request = request(&format!(
            "GET {path} HTTP/1.1\r\nHost: {host}\r\n{token}\r\n"
        ));
        Gateway::new("secreto", 5000).handle(&mut app(), &request, &assets)
    }

    const HOST: &str = "127.0.0.1:5000";

    #[test]
    fn the_page_is_served_without_a_token() {
        let r = get("/", None, HOST);
        assert_eq!((r.status, r.body.as_slice()), (200, b"<html>".as_slice()));
        assert!(!r.alive, "only the API counts as the page being open");
        assert_eq!(get("/nope.js", None, HOST).status, 404);
    }

    #[test]
    fn the_api_needs_the_token_of_this_run() {
        assert_eq!(get("/api/overview", None, HOST).status, 401);
        assert_eq!(get("/api/overview", Some("otro"), HOST).status, 401);
        assert_eq!(
            get("/api/overview", Some("secret"), HOST).status,
            401,
            "a prefix is not enough"
        );
        assert_eq!(get("/api/ping", Some("secreto"), HOST).status, 200);
    }

    #[test]
    fn a_host_that_is_not_ours_is_refused_even_with_the_token() {
        for host in ["evil.example", "127.0.0.1:5001", "localhost:5000", ""] {
            assert_eq!(
                get("/api/ping", Some("secreto"), host).status,
                403,
                "{host:?}"
            );
        }
        let no_host = request("GET /api/ping HTTP/1.1\r\nX-Purifex-Token: secreto\r\n\r\n");
        assert_eq!(
            Gateway::new("secreto", 5000)
                .handle(&mut app(), &no_host, &assets)
                .status,
            403
        );
    }

    #[test]
    fn only_get_is_accepted() {
        let post = request(
            "POST /api/ping HTTP/1.1\r\nHost: 127.0.0.1:5000\r\nX-Purifex-Token: secreto\r\n\r\n",
        );
        assert_eq!(
            Gateway::new("secreto", 5000)
                .handle(&mut app(), &post, &assets)
                .status,
            405
        );
    }

    #[test]
    fn an_authorized_request_says_the_page_is_open() {
        assert!(get("/api/ping", Some("secreto"), HOST).alive);
        assert!(get("/api/nada", Some("secreto"), HOST).alive);
    }

    #[test]
    fn the_api_answers_with_json_from_the_services() {
        for path in [
            "/api/overview",
            "/api/drivers",
            "/api/sessions",
            "/api/analysis",
        ] {
            let r = get(path, Some("secreto"), HOST);
            assert_eq!(
                r.status,
                200,
                "{path}: {}",
                String::from_utf8_lossy(&r.body)
            );
            let value: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
            assert!(value.is_object(), "{path}");
        }
        let sessions: serde_json::Value =
            serde_json::from_slice(&get("/api/sessions", Some("secreto"), HOST).body).unwrap();
        assert_eq!(sessions["sessions"][0]["id"], "lol-20261118-165329");
    }

    #[test]
    fn an_analysis_takes_a_name_that_is_never_a_path() {
        let ok = get(
            "/api/analysis?id=lol-20261118-165329",
            Some("secreto"),
            HOST,
        );
        assert_eq!(ok.status, 200);
        for bad in ["..%5Cx", "../x", "a.csv"] {
            let r = get(&format!("/api/analysis?id={bad}"), Some("secreto"), HOST);
            assert_eq!(r.status, 400, "{bad}");
        }
        assert_eq!(
            get("/api/analysis?id=zzz", Some("secreto"), HOST).status,
            500
        );
    }

    #[test]
    fn an_unknown_api_path_is_not_found() {
        assert_eq!(get("/api/borrar", Some("secreto"), HOST).status, 404);
    }

    #[test]
    fn secrets_are_compared_whole() {
        assert!(same_secret("abc", "abc"));
        assert!(!same_secret("abc", "abd"));
        assert!(!same_secret("abc", "abcd"));
        assert!(!same_secret("", "abc"));
        assert!(same_secret("", ""));
    }
}
