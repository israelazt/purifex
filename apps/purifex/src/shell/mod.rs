//! The window of Purifex: a server for the page on the computer's own address,
//! and the window that shows it. It moves bytes between a socket and the gateway
//! of `purifex-app` (which decides what to answer) and stops when the page is
//! gone. No decision is made here.

mod browser;

use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use purifex_app::http::{parse_request, Parsed, Response};
use purifex_app::{should_stop, Asset, Gateway, Services};
use purifex_domain::{log_info, log_warn};
use purifex_platform::entropy::random_token;

/// A connection whose request has not arrived whole yet.
struct Pending {
    stream: TcpStream,
    bytes: Vec<u8>,
    since: Instant,
}

/// How long a connection may take to send its request.
const REQUEST_TIME: Duration = Duration::from_secs(5);

fn asset(path: &str) -> Option<Asset> {
    purifex_ui::asset(path).map(|a| Asset {
        content_type: a.content_type,
        body: a.body,
    })
}

/// Serves the page until it is closed. `say` shows a line to the person
/// (the address, when no window is opened).
pub fn serve(
    services: &mut dyn Services,
    open: bool,
    say: &mut dyn FnMut(&str),
) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| format!("no se pudo abrir el puerto local: {e}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let token = random_token();
    let gateway = Gateway::new(&token, port);
    let url = format!("http://127.0.0.1:{port}/?t={token}");
    log_info!("serving the page on 127.0.0.1:{port}");

    // The window process is only held, not followed: Edge may hand its window to a
    // process that was already running and exit at once, so only the silence of
    // the page decides when to stop.
    let _window = if open {
        browser::open(&url)?
    } else {
        say(&format!("Abre esta dirección en un navegador: {url}"));
        None
    };

    let (started, mut last_alive, mut pending) =
        (Instant::now(), None::<Instant>, Vec::<Pending>::new());
    loop {
        let mut busy = false;
        while let Ok((stream, _)) = listener.accept() {
            if stream.set_nonblocking(true).is_ok() {
                pending.push(Pending {
                    stream,
                    bytes: Vec::new(),
                    since: Instant::now(),
                });
            }
            busy = true;
        }
        let mut waiting = Vec::new();
        for mut connection in pending.drain(..) {
            match read_request(&mut connection) {
                Read2::Waiting => waiting.push(connection),
                Read2::Dropped => busy = true,
                Read2::Request(parsed) => {
                    busy = true;
                    let response = match parsed {
                        Parsed::Done(request) => gateway.handle(services, &request, &asset),
                        Parsed::Bad(reason) => Response::error(400, &reason),
                        Parsed::Incomplete => Response::error(400, "petición incompleta"),
                    };
                    if response.alive {
                        last_alive = Some(Instant::now());
                    }
                    respond(connection.stream, &response);
                }
            }
        }
        pending = waiting;

        if should_stop(started.elapsed(), last_alive.map(|t| t.elapsed())) {
            log_info!("the page is gone: stopping");
            return Ok(());
        }
        if !busy {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

enum Read2 {
    Waiting,
    Dropped,
    Request(Parsed),
}

/// Reads what has arrived of a request, without waiting for the rest.
fn read_request(connection: &mut Pending) -> Read2 {
    let mut chunk = [0u8; 4096];
    loop {
        match connection.stream.read(&mut chunk) {
            Ok(0) => return Read2::Dropped,
            Ok(n) => connection.bytes.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(_) => return Read2::Dropped,
        }
        if connection.bytes.len() > 128 * 1024 {
            return Read2::Request(Parsed::Bad("petición demasiado grande".into()));
        }
    }
    match parse_request(&connection.bytes) {
        Parsed::Incomplete if connection.since.elapsed() < REQUEST_TIME => Read2::Waiting,
        Parsed::Incomplete => Read2::Dropped,
        parsed => Read2::Request(parsed),
    }
}

fn respond(mut stream: TcpStream, response: &Response) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    if let Err(error) = stream.write_all(&response.to_bytes()) {
        log_warn!("could not answer the page: {error}");
    }
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}
