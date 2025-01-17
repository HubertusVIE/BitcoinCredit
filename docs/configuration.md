# Configuration

The application can be configured using command-line parameters, or environment variables

The following options are available:

* `P2P_PORT` / `--p2p-port` - the P2P port (default: 1908)
* `P2P_ADDRESS` / `--p2p-address` - the P2P address (default: 0.0.0.0)
* `HTTP_PORT` / `--http-port` - the HTTP address (default: 8000)
* `HTTP_ADDRESS` / `--http-address` - the HTTP address (default: 127.0.0.1)
* `RUST_LOG` - the log level, e.g.: info, trace, debug, error (default: error)
* `NOSTR_RELAY` - nostr relay endpoint (default: ws://localhost:8080)
* `MINT_URL` - cashu mint endpoint (default: http://127.0.0.1:3338)

## Example

```bash
RUST_LOG=info cargo run -- --http-port 8001

RUST_LOG=info HTTP_PORT=8001 cargo run
```

