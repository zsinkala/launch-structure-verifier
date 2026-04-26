# Launch Structure Verifier

Launch Structure Verifier is a Rust service for checking whether a token launch looks structurally fair. It analyzes on-chain token facts, runs a set of launch-safety checks, and returns a score, grade, evidence, and human-readable interpretation.

The project currently supports:

- Solana SPL tokens through Helius RPC
- Base, Ethereum, and generic EVM ERC-20 tokens through Alchemy RPC
- A small Axum HTTP API
- In-memory response caching
- Unit and integration tests for checks, scoring, caching, and API flow

## What It Checks

For Solana tokens:

- Mint authority disabled
- Freeze authority disabled
- Holder concentration
- Token age
- Token standard sanity

For EVM tokens:

- Ownership renounced
- Holder concentration
- Token age
- Token standard sanity

The scoring model is a weighted sum. Unknown checks are excluded from the active weight total, and any failed critical check forces the final grade to `Compromised`.

## Project Layout

```text
src/
  api/          Request/response types, analysis orchestration, cached analysis
  cache/        Simple in-memory cache and TTL logic
  checks/       Individual launch-structure checks
  providers/    Helius, Alchemy, and mock provider implementations
  scoring/      Score aggregation and grade logic
  server.rs     Axum HTTP server setup
  main.rs       Server entrypoint
tests/          Integration tests and fixtures
frontend/       Static frontend entrypoint
index.html      Static browser UI
nixpacks.toml   Render/Nixpacks deployment config
```

## Requirements

- Rust toolchain from `rust-toolchain.toml`
- `cargo`
- Helius API key for Solana analysis
- Alchemy API key for Base/Ethereum/EVM analysis

## Environment Variables

The server requires these variables:

```bash
export HELIUS_API_KEY="your-helius-key"
export ALCHEMY_API_KEY="your-alchemy-key"
```

Optional:

```bash
export PORT=3000
export FRONTEND_ORIGIN="https://your-frontend-domain.example"
export PAYMENT_WALLET_ADDRESS="0x6aeaEC86d147e5A13cB7bD50CF2200C85656D6d9"
export PAID_REPORT_PRICE_USDC=5
```

If `PORT` is not set, the server uses `3000`.

If `FRONTEND_ORIGIN` is set, browser CORS requests are restricted to that origin. If it is not set, the server allows requests from any origin, which is convenient for local testing but too open for production.

`PAYMENT_WALLET_ADDRESS` is the dedicated wallet that receives paid report payments. `PAID_REPORT_PRICE_USDC` defaults to `5` and is measured in Base USDC.

## Run Locally

```bash
cargo run --bin launch-structure-verifier-server
```

The server binds to:

```text
http://0.0.0.0:3000
```

The API endpoint is:

```text
POST /api/v1/analyze
```

Health check:

```text
GET /health
```

Paid report payment verification:

```text
POST /api/v1/payments/verify
```

## Example Request

```bash
curl -X POST http://localhost:3000/api/v1/analyze \
  -H "Content-Type: application/json" \
  -d '{
    "chain": "solana",
    "address": "TOKEN_MINT_ADDRESS",
    "options": {
      "include_holders": true,
      "max_holders": 10,
      "force_refresh": false
    }
  }'
```

For EVM/Base:

```json
{
  "chain": "base",
  "address": "0xTokenContractAddress",
  "options": {
    "include_holders": true,
    "max_holders": 10,
    "force_refresh": false
  }
}
```

Supported `chain` values:

- `solana`
- `base`
- `ethereum`
- `evm`

Requests with unsupported chains or obviously invalid token addresses return `400 Bad Request` before provider APIs are called.

## Stablecoin Payments

Paid reports use manual transaction-hash verification for USDC on Base.

Official Base USDC contract:

```text
0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913
```

This address is published in Circle's official USDC contract address docs.

Payment flow:

1. User sends the configured price, currently `5 USDC`, to `PAYMENT_WALLET_ADDRESS` on Base.
2. User submits the transaction hash.
3. Backend checks the Base transaction receipt through Alchemy.
4. Backend verifies:
   - transaction succeeded
   - transfer token is official Base USDC
   - recipient is `PAYMENT_WALLET_ADDRESS`
   - amount is at least `PAID_REPORT_PRICE_USDC`
   - transaction hash has not already been used in the current server process

Verification request:

```bash
curl -X POST https://launch-structure-verifier.onrender.com/api/v1/payments/verify \
  -H "Content-Type: application/json" \
  -d '{
    "tx_hash": "0xTransactionHash",
    "token_address": "0xTokenBeingAnalyzed"
  }'
```

Successful response:

```json
{
  "valid": true,
  "report_access_id": "report_...",
  "message": "Payment verified.",
  "amount_usdc": "5"
}
```

This first version stores used transaction hashes in memory. For real paid usage, move used transaction hashes and report access records into a persistent database such as Supabase, Neon, Render Postgres, or Turso.

## Response Shape

The response includes:

- `analysis_id`: unique ID for this analysis
- `requested_at`: timestamp string
- `status`: `ok`, `partial`, or `error`
- `token`: normalized token metadata when available
- `checks`: individual check results with evidence
- `score`: weighted fairness score, grade, components, and notes
- `explain`: short explanation and interpretation
- `errors`: provider or data-fetching errors

## Cache Behavior

`analyze_with_cache` stores responses in a simple in-memory cache.

Cache key:

```text
chain:address:include_holders:max_holders
```

`force_refresh: true` bypasses reading from cache and stores a fresh response.

Current TTL behavior:

- Less than 24 hours old: 10 minutes
- 1 to 7 days old: 1 hour
- More than 7 days old: 1 hour
- Unknown age: 30 minutes
- Missing token metadata: 30 minutes

This cache is process-local. It does not survive restarts.

## Test

Run everything:

```bash
cargo test
```

Run integration tests only:

```bash
cargo test --test integration_test
```

Some provider tests are marked `ignored` because they make real network calls and require API keys. To run ignored tests:

```bash
cargo test -- --ignored
```

## Build

Debug build:

```bash
cargo build
```

Release server build:

```bash
cargo build --release --bin launch-structure-verifier-server
```

## Deployment Notes

`nixpacks.toml` builds the release server binary and starts:

```text
./target/release/launch-structure-verifier-server
```

For Render or similar platforms, make sure these environment variables are set:

- `HELIUS_API_KEY`
- `ALCHEMY_API_KEY`
- `PORT`
- `FRONTEND_ORIGIN`
- `PAYMENT_WALLET_ADDRESS`
- `PAID_REPORT_PRICE_USDC`

The server binds to `0.0.0.0`, which is required for external hosting.

## Render Setup

This repo includes a `render.yaml` Blueprint for the backend service.

Recommended setup:

1. Push this repository to GitHub.
2. Open Render.
3. Click `New` -> `Blueprint`.
4. Connect this repository.
5. Use the default Blueprint path: `render.yaml`.
6. Deploy the Blueprint.
7. In the created service, open `Environment`.
8. Add the secret values for:
   - `HELIUS_API_KEY`
   - `ALCHEMY_API_KEY`
9. Confirm `FRONTEND_ORIGIN` is:

```text
https://zsinkala.github.io
```

10. Confirm the health check path is:

```text
/health
```

The Blueprint configures:

- Rust runtime
- Free web service plan
- Release build command
- Server start command
- `/health` health check
- GitHub Pages CORS origin
- Secret placeholders for provider API keys

The current public API URL used by the static frontend is:

```text
https://launch-structure-verifier.onrender.com/api/v1/analyze
```

The API has a simple process-local rate limit of 60 analyze requests per client per 60 seconds. Render usually forwards the original client IP in `x-forwarded-for`; otherwise the server falls back to the TCP peer IP.

## Current Limitations

- Holder data is currently incomplete in real provider implementations.
- Solana metadata is minimal and currently reads basic mint/account info.
- EVM supply formatting assumes 18 decimals in the supply path.
- Timestamps are simple strings and should eventually use a proper datetime library.
- Cache is in-memory only.
- Rate limiting is in-memory only, so it resets when the process restarts and is not shared across multiple instances.
- Payment transaction replay protection is in-memory only in the first version, so use a database before taking meaningful payments.
- There is no `dfx.json` or canister deployment config in this repository right now, even though `candid` and `ic-cdk` dependencies are present.

## Useful Maintenance Notes

- Keep check IDs stable because API clients and score components may depend on them.
- If adding a new check, give it a clear `id`, `severity`, `weight`, `score_component`, and evidence payload.
- Unknown data should usually return `CheckStatus::Unknown` with `score_component: None` so it does not unfairly penalize the weighted score.
- Critical failed checks should remain capable of forcing `Grade::Compromised`.
- If changing provider normalization, keep the checks robust too. For example, EVM ownership renunciation should handle both `None` and explicit zero/burn addresses.
- Run `cargo test` before deploying.
