# XML File Watcher Docker Container

A Nix flake that builds a Docker container which monitors a directory tree for new XML files and triggers webhooks when they appear. Written in Rust for performance and reliability.

## Features

- Recursive directory monitoring using the `notify` Rust crate
- Triggers webhook on new XML files (created or moved into watched directory)
- Configurable webhook URL, method, and payload options
- Lightweight container built with Nix
- High-performance Rust implementation with async I/O

## Building

```bash
# Build the Docker image
nix build .#docker

# Load the image into Docker
docker load < result
```

## Running

### Docker

```bash
docker run -d \
  -v /path/to/watch:/watch \
  -e WEBHOOK_URL="https://your-webhook.example.com/endpoint" \
  -e WEBHOOK_METHOD="POST" \
  -e INCLUDE_CONTENT="false" \
  xml-watcher:latest
```

### Docker Compose

```yaml
version: '3.8'
services:
  xml-watcher:
    image: xml-watcher:latest
    volumes:
      - ./watched-folder:/watch
    environment:
      - WEBHOOK_URL=https://your-webhook.example.com/endpoint
      - WEBHOOK_METHOD=POST
      - INCLUDE_CONTENT=false
    restart: unless-stopped
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `WATCH_DIR` | `/watch` | Directory to monitor for XML files |
| `WEBHOOK_URL` | (required) | URL to send webhook requests to |
| `WEBHOOK_METHOD` | `POST` | HTTP method for webhook requests |
| `INCLUDE_FILENAME` | `true` | Include filename in payload |
| `INCLUDE_CONTENT` | `false` | Include full XML file content in payload |

## Webhook Payload

The webhook sends a JSON payload like this:

```json
{
  "event": "new_xml_file",
  "filepath": "/watch/subdir/example.xml",
  "filename": "example.xml",
  "timestamp": "2024-01-15T10:30:00+00:00"
}
```

With `INCLUDE_CONTENT=true`:

```json
{
  "event": "new_xml_file",
  "filepath": "/watch/subdir/example.xml",
  "filename": "example.xml",
  "content": "<?xml version=\"1.0\"?>...",
  "timestamp": "2024-01-15T10:30:00+00:00"
}
```

## Development

### Run locally (without Docker)

```bash
# Enter dev shell
nix develop

# Build with cargo
cargo build --release

# Or run directly with Nix
RUST_LOG=info WATCH_DIR=/tmp/watch WEBHOOK_URL=http://localhost:8080/hook nix run

# Or run with cargo
RUST_LOG=info WATCH_DIR=/tmp/watch WEBHOOK_URL=http://localhost:8080/hook cargo run
```

### Test the watcher

```bash
# Terminal 1: Start a simple webhook receiver
python3 -m http.server 8080

# Terminal 2: Run the watcher
RUST_LOG=info WATCH_DIR=/tmp/watch WEBHOOK_URL=http://localhost:8080/ nix run

# Terminal 3: Create a test XML file
mkdir -p /tmp/watch
echo '<?xml version="1.0"?><test/>' > /tmp/watch/test.xml
```

The `RUST_LOG` environment variable controls logging levels (trace, debug, info, warn, error).

## License

MIT