//! Emit the browser specialist's registration spec as a JSON bundle on stdout.
//!
//! Redirect this into an instance's `specialists/browser.json` to register the
//! browser specialist via `[[specialists]]` config instead of compiling it into
//! a product binary:
//!
//! ```sh
//! cargo run --bin emit-spec > <home>/.assistant/specialists/browser.json
//! ```
//!
//! The spec carries the reviewed system prompt and `allowed_tools`; the operator
//! tunes only capacity/pinning via config overrides.

use assistant_specialist_browser::{browser_specialist_spec, NetworkPolicy};

fn main() {
    let spec = browser_specialist_spec(NetworkPolicy::open());
    match serde_json::to_string_pretty(&spec) {
        Ok(json) => println!("{json}"),
        Err(e) => {
            eprintln!("failed to serialize browser specialist spec: {e}");
            std::process::exit(1);
        }
    }
}
