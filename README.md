# BSV 1Sat Ordinals Marketplace Backend

A high-performance Rust backend for discovering, caching, and serving BSV 1Sat Ordinals.

## Quick Start

```bash
# Build
cargo build --release

# Run
cargo run

# Test (in another terminal)
curl http://localhost:3000/health | jq
curl http://localhost:3000/wallet/YOUR_BSV_ADDRESS | jq
```

## API Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /` | API info |
| `GET /health` | Health check + cache stats |
| `GET /wallet/:address` | Get all ordinals for a wallet |
| `GET /wallet/:address?refresh=true` | Force refresh |
| `GET /ordinal/:origin` | Get ordinal details |
| `GET /ordinal/:origin/content` | Get content (image/file) |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3000` | Server port |
| `API_RATE_LIMIT` | `10` | Requests/sec to GorillaPool |

## Architecture

- **Rate Limiting**: 10 req/sec to external APIs
- **Caching**: TTL-based (30s ownership, 24hr content)
- **Concurrent Requests**: Max 5 parallel to GorillaPool
