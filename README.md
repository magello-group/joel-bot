# joel-bot

Magello's Slack bot that reminds everyone to time report on the last workday of the month.

- `--operation=check_last_workday` runs as a scheduled job every weekday and posts a
  reminder to `#allmant` on the last workday of the month (messages in `config.yaml`).
  Messages are picked per part (beginning, middle, end). On an occasion (`easter`, `valborg`,
  `midsommar`, `semester`, `kraftskiva`, `halloween`, see `src/occasion.rs`) its messages are
  always used. Otherwise the month's (`1`, `12`) are used 60 % of the time and `general` the
  rest. Unknown keys fail at startup.
- `--operation=api` serves Slack:
  - `@joel-bot` mentions: `tid` (next time report day), `pricing`, `skribenter`, or
    nothing for an introduction.
  - `/joel` (`/time-report`): when you next need to time report, only visible to you.
  - `/gg`: how long until the workday ends.

Both operations need the bot token in `JOEL_BOT_SLACK_TOKEN`.

## Building

The project uses the latest stable Rust (see `rust-toolchain.toml`); run
`rustup update` if the build says your toolchain is too old.

Simply build with cargo:

```console
cargo build
...
   Compiling joel-bot v2.0.0 (/path/to/joel-bot)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.35s
```

## Testing

```console
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Some tests call the live holiday API (sholiday.faboul.se). `holidays::test::live_validation`
checks the local holiday fallback (`src/holidays.rs`) against the API day by day; it also
runs weekly in GitHub Actions.

## Releasing

Publishing a GitHub release builds and deploys that version (`.github/workflows/rust_publish.yml`).
Once it is deployed, a birthday message with a link to the release is posted to Slack through the
`SLACK_RELEASE_HOOK_URL` repo secret (an incoming webhook, which decides the channel).
It must be a repo-level secret (Settings → Secrets and variables → Actions), not a
`Production` environment secret, as the announce job doesn't use that environment.
Pre-releases (and tags containing a `-`, like `v3.1.0-beta.1`) are deployed but not announced.

Once announced, a `📣 Announced in Slack` line is added to the release notes, and a re-run of the
workflow skips the announcement if the line is there. If a pre-release is later promoted to a
full release, announce it with Actions → Rust CI/CD → Run workflow, giving the tag. That only
announces, nothing is built or deployed.

## Running locally

Put `JOEL_BOT_SLACK_TOKEN` in a `.env` file (loaded with dotenvy), then:

```console
cargo run -- --operation=api
cargo run -- --operation=check_last_workday
```

### Debug using Dev Containers

> **NOTE:** This environment was set up using GitHub Copilot.

This Rust project supports developing using Dev Containers so you don't need Rust locally.

#### Requirements

- In Windows you will need WSL2 as Dev Containers works best with UN*X based environment
  - Linux and MacOS environments does not need this requirement.
- A Docker environment
  - Any Docker-compatible environment like e.g., Docker for Desktop (Windows/Linux), Rancher Desktop (all environments) or native Docker (Linux) works.
- Visual Studio Code with the Remote Development extension
  - Or a IDE that supports devcontainers like e.g. GitHub Codespaces

#### Open the dev container environment

- Clone this repo to your local development environment.
- Open the folder in Visual Studio Code.
- Open the Visual Studio Code command palette (`F1`/`SHIFT+CTRL+P`/`SHIFT+⌘+P`) and then select `Dev containers: Reopen in Container`.
- Create a `.env` file in the root folder containing the value for `JOEL_BOT_SLACK_TOKEN`
  - **DO NOT COMMIT THIS** (The `.env` file is excluded in `.gitignore` so it should not be an issue.)

And that's it, you can now rebuild/debug `joel-bot`.

#### Troubleshooting

In the event you change dependencies or environment variables, run the `Rebuild Container` command in the command palette to update the Dev Container.
