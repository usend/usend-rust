//! Runs the client against a tiny HTTP server on localhost, through the built-in `StdTransport`.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use usend::{BroadcastRecipientsParams, Client, ClientBuilder, Error, ListParams, RecipientType, SendEmail, SendEvent};

/// Serves one canned raw response per connection, in order, and records each raw request.
fn serve(responses: Vec<String>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen2 = seen.clone();
    thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = stream.read(&mut chunk).unwrap_or(0);
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(split) = text.find("\r\n\r\n") {
                    let len = text[..split]
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                        .unwrap_or(0);
                    if buf.len() >= split + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            seen2.lock().unwrap().push(String::from_utf8_lossy(&buf).to_string());
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (base, seen)
}

fn ok(body: &str) -> String {
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body)
}

fn status(code: u16, extra: &str, body: &str) -> String {
    format!("HTTP/1.1 {code} X\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
}

fn client(base: &str) -> Client {
    ClientBuilder::new().api_key("us_test").base_url(base).max_retries(2).build().unwrap()
}

#[test]
fn sends_an_email() {
    let (base, seen) = serve(vec![ok(r#"{"id":"e1"}"#)]);
    let email = SendEmail::new("a@acme.com", ["b@x.com"]).subject("Hi").text("yo").idempotency_key("k1");
    let res = client(&base).emails().send(&email).unwrap();
    assert_eq!(res.id, "e1");
    let req = seen.lock().unwrap()[0].clone();
    assert!(req.starts_with("POST /emails HTTP/1.1"), "{req}");
    assert!(req.contains("Authorization: Bearer us_test"));
    assert!(req.contains("Idempotency-Key: k1"));
    assert!(req.contains(r#""subject":"Hi""#));
    assert!(!req.contains("html"), "unset fields must be omitted");
    assert!(!req.contains("idempotency_key"), "the key goes in a header, not the body");
}

#[test]
fn parses_api_errors() {
    let body = r#"{"statusCode":422,"name":"validation_error","message":"The `to` field is required."}"#;
    let (base, _) = serve(vec![status(422, "", body)]);
    let err = client(&base).emails().get("x").unwrap_err();
    match err {
        Error::Api(e) => {
            assert_eq!((e.status_code, e.name.as_str()), (422, "validation_error"));
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn retries_429_with_retry_after() {
    let limited = status(429, "Retry-After: 0\r\n", r#"{"statusCode":429,"name":"rate_limit_exceeded","message":"slow"}"#);
    let (base, seen) = serve(vec![limited, ok(r#"{"id":"e1"}"#)]);
    let res = client(&base).emails().send(&SendEmail::new("a@b.com", ["c@d.com"]).text("x")).unwrap();
    assert_eq!(res.id, "e1");
    assert_eq!(seen.lock().unwrap().len(), 2);
}

#[test]
fn does_not_retry_5xx_on_post_without_idempotency_key() {
    let boom = status(503, "", r#"{"statusCode":503,"name":"internal_server_error","message":"x"}"#);
    let (base, seen) = serve(vec![boom.clone(), ok(r#"{"id":"e1"}"#)]);
    let err = client(&base).emails().send(&SendEmail::new("a@b.com", ["c@d.com"]).text("x")).unwrap_err();
    assert!(matches!(err, Error::Api(_)));
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[test]
fn retries_5xx_on_get() {
    let boom = status(502, "Retry-After: 0\r\n", "{}");
    let (base, seen) = serve(vec![boom, ok(r#"{"object":"list","data":[],"has_more":false}"#)]);
    let list = client(&base).emails().list(&ListParams::new().limit(5)).unwrap();
    assert!(list.data.is_empty());
    let first = seen.lock().unwrap()[0].clone();
    assert!(first.starts_with("GET /emails?limit=5 HTTP/1.1"), "{first}");
}

#[test]
fn broadcast_recipients_sends_the_required_type() {
    let (base, seen) = serve(vec![ok(r#"{"object":"list","has_more":false,"data":[{"id":"e1","contact_id":null,"email":"ann@x.com","count":2,"clicked_links":[{"url":"https://a.com","clicks":2}]}]}"#)]);
    let params = BroadcastRecipientsParams { email: Some("ann".into()), ..Default::default() };
    let list = client(&base).broadcasts().recipients("b1", RecipientType::Clicked, &params).unwrap();
    assert_eq!(list.data[0].count, Some(2));
    assert_eq!(list.data[0].clicked_links[0].clicks, 2);
    let req = seen.lock().unwrap()[0].clone();
    assert!(req.starts_with("GET /broadcasts/b1/recipients?type=clicked&email=ann HTTP/1.1"), "{req}");
}

#[test]
fn events_send_accepts_202() {
    let body = r#"{"object":"event","event":"order.paid","id":"x1"}"#;
    let (base, seen) = serve(vec![status(202, "", body)]);
    let ev = SendEvent::for_email("order.paid", "a@b.com").payload("total", 5).idempotency_key("k");
    let res = client(&base).events().send(&ev).unwrap();
    assert_eq!((res.id.as_str(), res.event.as_str()), ("x1", "order.paid"));
    let req = seen.lock().unwrap()[0].clone();
    assert!(req.contains("Idempotency-Key: k"));
}

#[test]
fn delete_with_empty_object_body_is_ok() {
    let (base, _) = serve(vec![ok("{}")]);
    client(&base).api_keys().remove("k1").unwrap();
}

#[test]
fn https_is_refused_by_the_builtin_transport() {
    let c = ClientBuilder::new().api_key("us_test").base_url("https://api.usend.et").max_retries(0).build().unwrap();
    // Without the `ureq`/`reqwest` features the default transport cannot do TLS.
    #[cfg(not(any(feature = "ureq", feature = "reqwest")))]
    assert!(matches!(c.emails().get("x").unwrap_err(), Error::Transport(_)));
    let _ = c;
}
