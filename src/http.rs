use std::sync::LazyLock;
use std::time::Duration;

use reqwest::Client;

/// Identifies the bot to the APIs it calls. Cloudflare in front of the holiday
/// API started answering 403 to requests from GitHub's runners, and requests
/// without a User-Agent look like bot traffic.
const USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (+",
    env!("CARGO_PKG_REPOSITORY"),
    ")"
);

/// Shared HTTP client with timeouts, so a hanging request can't stall the bot.
pub static CLIENT: LazyLock<Client> = LazyLock::new(client);

/// A new client with the same settings as [`CLIENT`]. Async tests should use
/// this rather than [`CLIENT`]: each test has its own runtime, and pooled
/// connections die with the runtime that opened them.
#[must_use]
pub fn client() -> Client {
    Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("couldn't build http client")
}
