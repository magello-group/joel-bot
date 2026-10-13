use std::sync::LazyLock;
use std::time::Duration;

use reqwest::Client;

/// Shared HTTP client with timeouts, so a hanging request can't stall the bot.
pub static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("couldn't build http client")
});
