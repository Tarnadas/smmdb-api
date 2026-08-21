# Self-hosting SMMDB API

This document explains how to run the SMMDB API backend (the SMM2 / courses2 side of
the platform) on your own infrastructure. The web frontend is a separate repository
(`smmdb`) and has its own `docs/SELFHOSTING.md`.

## What you need

- Rust nightly toolchain pinned to `nightly-2021-04-25` (see `rust-toolchain`). Paperclip
  and `actix-nightly` require this specific nightly. Install with:
  ```
  rustup toolchain install nightly-2021-04-25
  ```
- MongoDB 3.4+ (the driver is an old `mongodb_cwal` 0.6 fork)
- A Google Cloud OAuth Client ID (see below)
- Optional: a domain and reverse proxy for TLS

## Google OAuth setup

1. Create a project at <https://console.cloud.google.com/apis/credentials>.
2. Create an OAuth 2.0 Client ID of type *Web application*.
3. Add your frontend origin (e.g. `http://localhost:3000`) to *Authorized JavaScript
   origins*. No redirect URIs are needed.
4. The client ID is used to verify Google ID tokens at login. It must match the client
   ID the frontend sends (in the `smmdb` repo that is `GOOGLE_CLIENT_ID` baked into the
   client bundle at build time).

## Environment variables

All of these are read **at runtime** — no rebuild is needed when they change.

| Variable          | Required | Default  | Notes                                                             |
|-------------------|----------|----------|-------------------------------------------------------------------|
| `MONGODB_URI`     | yes      | —        | `mongodb://host:27017` — the server **panics** if unset            |
| `GOOGLE_CLIENT_ID`| yes      | `""`     | Must equal the `aud` of the Google ID tokens accepted at login     |
| `COOKIE_SECRET`   | no       | `""`     | Signs the session cookie; use `openssl rand -hex 32`              |
| `PORT`            | no       | `3030`   | Port the API listens on (always binds `0.0.0.0`)                  |

The database name is hardcoded to `admin` (see `crates/smmdb-db/src/lib.rs`), matching
the `smmdb` mongodump layout.

## Building

```
cargo build --release
```

or, if you prefer Docker:

```
docker compose up --build -d
```

`docker compose up` starts both the API and a `mongo:3.4` container, and exposes the API
on `127.0.0.1:3030`.

## Running

```
MONGODB_URI=mongodb://localhost:27017 \
GOOGLE_CLIENT_ID=your-client-id.apps.googleusercontent.com \
COOKIE_SECRET=$(openssl rand -hex 32) \
cargo run
```

## Importing course data

A pre-sanitized copy of the final production dump (all courses, no user data) is
available for download:

**https://drive.proton.me/urls/6TF4JYA5QW#pFLyVy6mmCjG**

(`backup-sanitized.7z`, ~25 GiB — extract it and restore the contained `admin/` dump.)

Restore a dump **before** starting the API:

```
mongorestore --drop --uri mongodb://localhost:27017 /path/to/dump
```

If the dump came from the original SMMDB instance and you intend to share it with third
parties, **sanitize it first**: the `accounts` collection stores plaintext emails,
Google IDs, API keys and ID tokens, and `amazon` stores affiliate URLs with embedded AWS
access keys. Use the `smmdb` repo's script:

```
node ../smmdb/scripts/sanitize-backup.js /path/to/original/dump ./clean-dump
mongorestore --drop --uri mongodb://localhost:27017 ./clean-dump
```

`accounts` and `amazon` are dropped entirely; all course data is preserved. For a fresh
instance you can skip the dump and register new accounts through Google login.

## Reverse proxy

Put the API behind a TLS-terminating proxy if needed:

```
server {
    server_name api.example.com;
    location / {
        proxy_pass http://127.0.0.1:3030;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
}
```

## Troubleshooting

- **Server exits with a MongoError / panic on startup**: `MONGODB_URI` is unset or the
  MongoDB instance is unreachable. Set `MONGODB_URI` and verify the database is up.
- **Login always fails with `ClientIdInvalid`**: `GOOGLE_CLIENT_ID` does not match the
  `aud` claim of the ID token the frontend sends. Set it to the same value the frontend
  was built with.
- **Sessions do not survive a restart**: the session cookie signature key is empty.
  Set `COOKIE_SECRET` to a stable random value.
- **`cargo build` fails on paperclip macros**: you must use the exact nightly
  `nightly-2021-04-25` from `rust-toolchain`, not a newer toolchain.
