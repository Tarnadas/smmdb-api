# AGENTS.md

Guidance for AI agents and humans working in this repository.

## Self-hosting checklist

This project is designed to be self-hostable. When you change deployment-related code,
keep the following working and documented (see `docs/SELFHOSTING.md`):

1. **No hardcoded secrets.** Search the repo for real-looking credentials
   (`899493559187-...apps.googleusercontent.com`, `[0; 32]` cookie keys, API keys,
   passwords) before committing. Secrets must come from environment variables.
2. **Environment variables** used by the runtime (`MONGODB_URI`, `PORT`,
   `GOOGLE_CLIENT_ID`, `COOKIE_SECRET`) must stay documented in `docs/SELFHOSTING.md`
   and must not be baked into artifacts that ship. This server reads all of them at
   runtime — do not reintroduce constants.
3. **No domain-specific hardcodes.** `smmdb.net` and `api.smmdb.net` references in
   README/docs are historical links; they must not be baked into served code or
   configuration. The swagger `Info` description in `crates/smmdb/src/server/mod.rs`
   already had the website link removed.
4. **Docker build args.** `GOOGLE_CLIENT_ID` is a runtime env var here (no webpack
   client bake), so it must be set in the `environment:` block of `docker-compose.yml`,
   not only as a build arg.
5. **Secrets not tracked.** No credential files may be committed. `.gitignore` covers
   `/.idea/`, `/.vscode/`, `/db/`, `/courses/`, `/mongo/db`, `/target`.
6. **Backup sanitization.** The production mongodump contains PII (plaintext emails,
   Google IDs, API keys, ID tokens in `accounts`; AWS access keys in `amazon`
   `detailPageURL`). Any shared/redistributed backup must be run through the `smmdb`
   repo's `scripts/sanitize-backup.js` (drops `accounts` and `amazon` collections).

## Quick commands

- Build: `cargo build --release` (requires the pinned `nightly-2021-04-25` toolchain).
- Check: `cargo check --workspace`.
- Test: `cargo test --workspace`.
- Docker: `docker compose up --build -d` (API + MongoDB 3.4).

## Tech notes

- Stack: Rust 2018, actix-web 3, MongoDB (driver `mongodb_cwal` 0.6 fork), paperclip
  (OpenAPI), nightly-2021-04-25 toolchain (pinned in `rust-toolchain`).
- Workspace: `crates/smmdb` (server + routes), `crates/smmdb-auth` (identity/session),
  `crates/smmdb-common` (models), `crates/smmdb-db` (Database layer).
- Entry: `crates/smmdb/src/main.rs` -> `crates/smmdb/src/server/mod.rs`
  (`Server::start`, binds `0.0.0.0:{PORT}` default 3030, cookie session signed with
  `COOKIE_SECRET`).
- Auth: `crates/smmdb/src/session.rs` (APIKEY header) + `crates/smmdb-auth`.
- Google login: `crates/smmdb/src/routes/login.rs` verifies ID tokens against
  `GOOGLE_CLIENT_ID` (`config.rs` `get_google_client_id`, runtime env).
- DB config: `crates/smmdb-db/src/lib.rs` reads `MONGODB_URI` (panics if unset), db
  name hardcoded `admin`.
- Course serialization: `smmdb-lib` git dependency; courses2 data is encrypted/brotli'd.
