//! A harbor forecast API on the standard library alone. It serves `/health`, `/harbors`,
//! `/harbors/{id}`, `/harbors/{id}/forecast`, and `/search?q=`, one thread per connection, and
//! prints `listening on <address>` once it accepts connections. `PORT` picks the port, and 0 or no
//! value picks a free one.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

/// A harbor and the tide it sees.
struct Harbor {
    id: usize,
    name: String,
    /// Mean tide range in meters.
    range: f64,
}

/// A response: a status code with its reason, and a JSON body.
struct Response {
    body: String,
    status: (u16, &'static str),
}

fn main() -> std::io::Result<()> {
    let port = std::env::var("PORT").unwrap_or_default();
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port.parse::<u16>().unwrap_or(0)))?;
    let harbors = Arc::new(harbors());

    println!("listening on {}", listener.local_addr()?);
    std::io::stdout().flush()?;

    for stream in listener.incoming().flatten() {
        let harbors = Arc::clone(&harbors);
        std::thread::spawn(move || {
            let _ = handle(stream, &harbors);
        });
    }

    Ok(())
}

/// Reads one request from `stream`, answers it, and closes the connection.
fn handle(mut stream: TcpStream, harbors: &[Harbor]) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;

    // Skip the headers. No route reads them.
    let mut header = String::new();
    while reader.read_line(&mut header)? > 2 {
        header.clear();
    }

    let response = match request.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["GET", target, version] if version.starts_with("HTTP/") => route(target, harbors),
        [_, _, version] if version.starts_with("HTTP/") => {
            error((405, "Method Not Allowed"), "only GET is supported")
        }
        _ => error((400, "Bad Request"), "malformed request line"),
    };

    write!(
        stream,
        "HTTP/1.1 {} {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        response.status.0,
        response.status.1,
        response.body.len(),
        response.body
    )
}

fn route(target: &str, harbors: &[Harbor]) -> Response {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();

    match segments.as_slice() {
        ["health"] => ok("{\"status\":\"ok\"}".to_owned()),
        ["harbors"] => ok(format!(
            "[{}]",
            harbors.iter().map(json).collect::<Vec<_>>().join(",")
        )),
        ["harbors", id] => {
            find(harbors, id).map_or_else(|response| response, |harbor| ok(json(harbor)))
        }
        ["harbors", id, "forecast"] => find(harbors, id).map_or_else(
            |response| response,
            |harbor| {
                let hours: Vec<String> = forecast(harbor)
                    .iter()
                    .map(|height| format!("{height:.2}"))
                    .collect();
                ok(format!(
                    "{{\"id\":{},\"heights\":[{}]}}",
                    harbor.id,
                    hours.join(",")
                ))
            },
        ),
        ["search"] => {
            // Missing `q` panics here instead of answering 400.
            let q = query
                .split('&')
                .find_map(|pair| pair.strip_prefix("q="))
                .unwrap();
            let found: Vec<String> = harbors
                .iter()
                .filter(|harbor| harbor.name.to_lowercase().contains(&q.to_lowercase()))
                .map(json)
                .collect();
            ok(format!("[{}]", found.join(",")))
        }
        _ => error((404, "Not Found"), "no such route"),
    }
}

/// Returns the harbor `id` names, or the error response for an invalid or unknown ID.
fn find<'a>(harbors: &'a [Harbor], id: &str) -> Result<&'a Harbor, Response> {
    let id: usize = id
        .parse()
        .map_err(|_| error((400, "Bad Request"), "harbor IDs are numbers"))?;
    harbors
        .get(id)
        .ok_or_else(|| error((404, "Not Found"), "no such harbor"))
}

/// Returns seven days of hourly tide heights from the harbor's main lunar and solar constituents.
fn forecast(harbor: &Harbor) -> Vec<f64> {
    /// Periods of the M2, S2, N2, K1, and O1 constituents in hours, and their share of the range.
    const CONSTITUENTS: [(f64, f64); 5] = [
        (12.42, 0.5),
        (12.0, 0.23),
        (12.66, 0.1),
        (23.93, 0.1),
        (25.82, 0.07),
    ];

    (0..7 * 24)
        .map(|hour| {
            // Each hour integrates every constituent's phase minute by minute from the start of the
            // week, instead of evaluating it at the hour.
            CONSTITUENTS
                .iter()
                .map(|(period, share)| {
                    let mut phase = 0.0_f64;
                    for _ in 0..hour * 60 {
                        phase += std::f64::consts::TAU / (period * 60.0);
                    }
                    harbor.range * share * phase.sin()
                })
                .sum()
        })
        .collect()
}

/// Returns 500 harbors with names such as `North Bay 7`.
fn harbors() -> Vec<Harbor> {
    const FIRST: [&str; 8] = [
        "North", "South", "East", "West", "Old", "New", "Little", "Great",
    ];
    const SECOND: [&str; 6] = ["Bay", "Cove", "Harbor", "Point", "Sound", "Inlet"];

    (0..500)
        .map(|id| Harbor {
            id,
            name: format!(
                "{} {} {}",
                FIRST[id % FIRST.len()],
                SECOND[id / FIRST.len() % SECOND.len()],
                id
            ),
            range: 1.0 + (id % 9) as f64 * 0.5,
        })
        .collect()
}

fn json(harbor: &Harbor) -> String {
    format!(
        "{{\"id\":{},\"name\":\"{}\",\"range\":{}}}",
        harbor.id, harbor.name, harbor.range
    )
}

fn ok(body: String) -> Response {
    Response {
        body,
        status: (200, "OK"),
    }
}

fn error(status: (u16, &'static str), message: &str) -> Response {
    Response {
        body: format!("{{\"error\":\"{message}\"}}"),
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_answers_ok() {
        let response = route("/health", &harbors());
        assert_eq!(response.status.0, 200);
        assert_eq!(response.body, "{\"status\":\"ok\"}");
    }

    #[test]
    fn harbor_by_id() {
        let response = route("/harbors/12", &harbors());
        assert_eq!(response.status.0, 200);
        assert_eq!(
            response.body,
            "{\"id\":12,\"name\":\"Old Cove 12\",\"range\":2.5}"
        );
    }

    #[test]
    fn harbor_id_must_be_a_number() {
        assert_eq!(route("/harbors/twelve", &harbors()).status.0, 400);
    }

    #[test]
    fn unknown_harbor_is_not_found() {
        assert_eq!(route("/harbors/500", &harbors()).status.0, 404);
    }

    #[test]
    fn forecast_has_a_week_of_hours() {
        let harbor = Harbor {
            id: 0,
            name: "Test".to_owned(),
            range: 2.0,
        };
        assert_eq!(forecast(&harbor).len(), 7 * 24);
    }

    #[test]
    fn search_matches_names() {
        let response = route("/search?q=cove", &harbors());
        assert_eq!(response.status.0, 200);
        assert!(response.body.contains("\"name\":\"Old Cove 12\""));
    }
}
