# VoidGate backend

Small Rust skeleton for an HTTP security decision service. It returns `decoy` only for exact paths configured by the operator; other valid paths return `allow`. The service does not proxy traffic or run a honeypot yet. The integrating gateway must enforce each decision.

## Run

Requires a stable Rust toolchain. Set `VOIDGATE_API_KEY` to a random secret of at least 32 characters. Optionally set comma-separated `VOIDGATE_DECOY_PATHS` and `VOIDGATE_BIND` (default `127.0.0.1:8080`). The example environment file documents these values; this skeleton does not load `.env` automatically.

```sh
cargo run
```

Health endpoints: `GET /health`, `GET /live`, `GET /ready`.

## Decision API

`POST /v1/decisions/http` accepts at most 4096 bytes and requires `x-voidgate-key`. Example:

```sh
curl -X POST http://127.0.0.1:8080/v1/decisions/http \
  -H "content-type: application/json" \
  -H "x-voidgate-key: $VOIDGATE_API_KEY" \
  -d '{"method":"GET","path":"/internal/backup"}'
```

Response: `{"action":"decoy","reason_code":"registered_decoy_path"}` for a configured path, or `{"action":"allow","reason_code":"no_matching_rule"}` otherwise. Use TLS and a trusted gateway before exposing the decision API beyond localhost. A real deployment also needs tenant identity, rate limits, persistent policy, replicas and an isolated decoy service.

## Verify

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

