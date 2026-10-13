# joel-bot

Här ska vi skriva funktionalitet så småningom, men det är dag tre kl 17:43 så vi avbryter nu.

## Building

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
