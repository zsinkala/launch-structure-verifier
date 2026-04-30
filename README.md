# Launch Structure Verifier

Launch Structure Verifier is a Rust service for checking whether a token launch looks structurally fair. It analyzes on-chain token facts, runs a set of launch-safety checks, and returns a score, grade, evidence, and human-readable interpretation.

The project currently supports:

- Solana SPL tokens through Helius RPC
- Base, Ethereum, and generic EVM ERC-20 tokens through Alchemy RPC
- A small Axum HTTP API
- In-memory response caching
- Unit and integration tests for checks, scoring, caching, and API flow

## Continuation Notes

Last known good state:

- Frontend is live at `https://zsinkala.github.io/launch-structure-verifier/`.
- Backend is live at `https://launch-structure-verifier.onrender.com`.
- Health check is `https://launch-structure-verifier.onrender.com/health`.
- Render/main deployed commit `e8f0613`: `Fix printable report popup`.
- GitHub Pages deployed commit `03d648e`: `Deploy printable report popup fix`.
- Paid Report UI is visible after running an analysis and unlocks after a valid Base USDC payment.
- Paid report now renders a cleaner structured report instead of raw score/evidence JSON.
- Paid report has `Copy ID` and `Download JSON` actions after unlock.
- Solana token metadata was improved with Helius `getAsset`; BONK now shows as `Bonk` instead of `Unknown`.
- Supabase table `used_payment_txs` exists.
- Render has `SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY` set.
- Payment wallet is `0x6aeaEC86d147e5A13cB7bD50CF2200C85656D6d9`.
- Paid report price is `5 USDC` on Base.
- Real test payment was sent on Base and appears valid:
  - Transaction hash: `0xedff7307b21ca982d42a46a200368a2c372c2810104568dc5da48289dd4ab325`
  - It transferred `5 USDC` from `0x4afa189f594e3a19c63debffb12429b283bf4e88` to the payment wallet.
  - Public Base RPC returned status `0x1`, official Base USDC contract, and amount `0x4c4b40` (`5 USDC`).
- Initial paid-report verification failed with `400` because the browser field contained a full BaseScan URL. Local frontend now extracts raw `0x...` hashes from either URLs or raw hash input.
- Retrying with the raw hash reached the backend but returned `502`.
- Render logs showed `Payment store lookup error: InvalidResponse(404)`, meaning Supabase REST could not find `used_payment_txs`.
- Supabase URL had been set as `https://kewultygallbtruozutw.supabase.co/rest/v1/`; it should be exactly `https://kewultygallbtruozutw.supabase.co` because the backend appends `/rest/v1/...`.
- Supabase SQL table creation was run and returned success:
  ```sql
  create table if not exists used_payment_txs (
    tx_hash text primary key,
    report_access_id text not null,
    token_address text not null,
    amount_usdc text,
    created_at timestamptz not null default now()
  );
  ```
- Earlier Render startup trouble (`Port scan timeout reached`) was resolved; server now binds to `0.0.0.0:<PORT>` and `/health` returns `ok`.
- Repo-side Render config uses `type: web`, `cargo build --release --bin launch-structure-verifier-server`, and `./target/release/launch-structure-verifier-server`.
- Payment verification was fixed after replacing the wrong Render `SUPABASE_SERVICE_ROLE_KEY` with the correct service-role key.
- RLS is enabled on `public.used_payment_txs`.
- Supabase Security Advisor later reported `RLS Enabled No Policy` for `public.used_payment_txs`; run `supabase/used_payment_txs_security.sql` in the Supabase SQL editor to add explicit deny-all browser policies while keeping backend service-role access.
- Live payment verification results on April 28, 2026:
  - Fake tx `0x0000000000000000000000000000000000000000000000000000000000000001` returned `404`, confirming Supabase auth passed and Alchemy returned transaction-not-found.
  - Real tx `0xedff7307b21ca982d42a46a200368a2c372c2810104568dc5da48289dd4ab325` returned `200` with `valid: true` and `amount_usdc: "5"`.
  - Reusing the same real tx returned `valid: false` with `This transaction hash has already been used.`
  - Live frontend shows the already-used message correctly and no longer duplicates that message.
  - A new real Base USDC payment was used successfully to unlock a BONK paid report.
  - The paid BONK report showed `Token: Bonk`, `Chain: Solana`, `Score: 100/100`, `Status: Ok`, and grade `Strong`.
- Final local test run after Solana metadata improvement: `cargo test` passed with 44 active tests and 7 ignored live-provider tests.
- Workspace was clean on `main` after the last deployment.

Current follow-up checklist:

Done locally after continuation:

- Added a top-of-report `Buyer Verdict` section to the paid report with `Low structural risk`, `Mixed structural risk`, or `High structural risk` plain-language reasoning.
- Added `buyer_verdict` to the downloaded paid report JSON.
- Added a payment UX note explaining valid payments: Base network, official Base USDC, exact payment wallet, minimum `5 USDC`, and one report per transaction hash.
- Added `supabase/used_payment_txs_security.sql` to resolve the Supabase `RLS Enabled No Policy` advisor item with explicit deny-all browser policies.
- Added Solana holder concentration fetching through Helius/Solana RPC `getTokenLargestAccounts` plus `getTokenSupply`.
- Added Solana token age detection through Helius/Solana RPC `getSignaturesForAddress` using the oldest mint-account signature block time.
- Added Base token age detection through Alchemy/EVM RPC by binary-searching for the first block where contract bytecode exists.
- Added optional Base holder concentration through Etherscan V2 `tokenholderlist` on Base (`chainid=8453`) when `BASESCAN_API_KEY` is configured.
- Added a paid-report `Print / Save PDF` action that opens a clean printable HTML report.
- Fixed the first `Print / Save PDF` attempt, which opened blank `about:blank` tabs because the popup used `noopener,noreferrer`; the live `gh-pages` branch now uses `window.open('', '_blank')`.
- Added an admin-only payment verification status endpoint, protected by `ADMIN_API_KEY`, for recent payment verification attempts.
- Live free BONK analysis with force refresh confirmed Solana holder concentration works: holder concentration passed with `100/100`, top1 about `7.64%`, top5 about `25.55%`, and overall score `93/100`.
- Base token age now populates on live Base USDC analysis, but Base holder concentration remains `Unknown` on the free Etherscan API plan because `tokenholderlist` is a paid Standard/Pro endpoint.
- A fresh paid report unlock after the print-button deploy showed the `Print / Save PDF` button, but that specific test happened before the popup fix was fully verified in-browser.
- On April 30, 2026, a fresh live paid BONK report verified the `Print / Save PDF` popup fix from `https://zsinkala.github.io/launch-structure-verifier/?v=popup-fix-2`; Chrome opened the printable report in a new tab and generated the PDF view successfully.
- `cargo test` passed with 54 active tests and 11 ignored live-provider tests on April 30, 2026.

Remaining checklist:

1. Set `ADMIN_API_KEY` in Render before using `GET /api/v1/admin/payments`.
2. Consider upgrading Etherscan only later, when Base holder concentration is needed for demos or paying users.
3. Before new paid tests, remember each valid Base USDC payment tx can only unlock one report because it is persisted in Supabase.

Important warnings:

- Do not put `SUPABASE_SERVICE_ROLE_KEY` in any frontend file.
- The paid report flow is manual transaction-hash verification, not wallet connect checkout.
- Base USDC only. The official Base USDC contract used by the backend is `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913`.
- The tool sells structural risk clarity, not investment advice or price prediction.

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
export ADMIN_API_KEY="your-admin-only-random-key"
export SUPABASE_URL="https://your-project-ref.supabase.co"
export SUPABASE_SERVICE_ROLE_KEY="your-backend-secret-key"
export BASESCAN_API_KEY="your-basescan-key"
```

If `PORT` is not set, the server uses `3000`.

If `FRONTEND_ORIGIN` is set, browser CORS requests are restricted to that origin. If it is not set, the server allows requests from any origin, which is convenient for local testing but too open for production.

`PAYMENT_WALLET_ADDRESS` is the dedicated wallet that receives paid report payments. `PAID_REPORT_PRICE_USDC` defaults to `5` and is measured in Base USDC.

`SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY` enable persistent paid-report transaction storage. Keep the service role key only on the backend.

`ADMIN_API_KEY` enables admin-only operational endpoints. Keep it out of frontend files and send it only as `x-admin-api-key` or an `Authorization: Bearer ...` header.

`BASESCAN_API_KEY` enables Base holder concentration through Etherscan V2's `tokenholderlist` endpoint for Base (`chainid=8453`). Without it, Base holder concentration remains `Unknown` while the rest of the Base analysis still runs through Alchemy. Etherscan marks this holder-list endpoint as a paid Standard/Pro endpoint, so free API keys may still return `Unknown`.

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

Admin payment status:

```text
GET /api/v1/admin/payments
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
   - transaction hash has not already been used, using Supabase when configured or in-memory storage for local development

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

Used transaction hashes and report access records are stored in Supabase when `SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY` are configured. If those variables are missing, the server falls back to in-memory storage for local development.

Admin payment status request:

```bash
curl https://launch-structure-verifier.onrender.com/api/v1/admin/payments \
  -H "x-admin-api-key: $ADMIN_API_KEY"
```

The admin response is an in-memory rolling log of the latest 100 payment verification attempts since the server last started. It reports operational details such as tx hash, token address, status, HTTP status, amount, report ID, and message. It does not expose API keys, Supabase credentials, or wallet secrets.

Supabase table:

```sql
create table if not exists used_payment_txs (
  tx_hash text primary key,
  report_access_id text not null,
  token_address text not null,
  amount_usdc text,
  created_at timestamptz not null default now()
);
```

Security policies for this backend-only table are in `supabase/used_payment_txs_security.sql`. They explicitly deny `anon` and `authenticated` browser access so the table stays service-role-only.

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
- `FRONTEND_ORIGIN`
- `PAYMENT_WALLET_ADDRESS`
- `PAID_REPORT_PRICE_USDC`
- `ADMIN_API_KEY`
- `SUPABASE_URL`
- `SUPABASE_SERVICE_ROLE_KEY`

Do not manually set `PORT` on Render unless Render support explicitly asks for it. Render provides `PORT`; the server defaults to `3000` only for local development.

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
- Payment transaction replay protection is persistent when Supabase env vars are configured.
- There is no `dfx.json` or canister deployment config in this repository right now, even though `candid` and `ic-cdk` dependencies are present.

## Useful Maintenance Notes

- Keep check IDs stable because API clients and score components may depend on them.
- If adding a new check, give it a clear `id`, `severity`, `weight`, `score_component`, and evidence payload.
- Unknown data should usually return `CheckStatus::Unknown` with `score_component: None` so it does not unfairly penalize the weighted score.
- Critical failed checks should remain capable of forcing `Grade::Compromised`.
- If changing provider normalization, keep the checks robust too. For example, EVM ownership renunciation should handle both `None` and explicit zero/burn addresses.
- Run `cargo test` before deploying.
