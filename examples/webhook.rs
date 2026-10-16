//! Verify a webhook delivery from the command line (the same call works inside any web framework).
//!
//!     echo -n "$BODY" | cargo run --example webhook -- whsec_... msg_id 1760000000 "v1,base64sig"

use std::io::Read;

use usend::verify_webhook_headers;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        eprintln!("usage: webhook <whsec_secret> <webhook-id> <webhook-timestamp> <webhook-signature>  (body on stdin)");
        std::process::exit(2);
    }
    let mut body = Vec::new();
    std::io::stdin().read_to_end(&mut body).unwrap();

    let headers = [("webhook-id", args[1].as_str()), ("webhook-timestamp", args[2].as_str()), ("webhook-signature", args[3].as_str())];
    match verify_webhook_headers(&body, &headers, &args[0]) {
        Ok(event) => println!("valid {} ({})", event.event_type, event.id),
        Err(e) => {
            eprintln!("rejected: {e}");
            std::process::exit(1);
        }
    }
}
