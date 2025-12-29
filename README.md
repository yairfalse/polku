# POLKU

**Lightweight Internal Message Pipeline**

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.83%2B-orange.svg)](https://www.rust-lang.org)

Decouple your internal services without a message broker. For when Kafka is overkill.

---

## What is POLKU?

POLKU is a **lightweight message pipeline** that sits between your internal services. It transforms, buffers, and routes messages - without the operational overhead of a full pub/sub system.

```
┌─────────────────────────────────────────────────────────────┐
│                         POLKU                                │
│                                                              │
│  Inputs              Pipeline            Outputs             │
│  ┌──────────┐    ┌───────────┐        ┌──────────┐         │
│  │ gRPC     │───►│ Transform │───────►│ gRPC     │         │
│  │ REST     │    │ Buffer    │        │ Kafka    │         │
│  │ Webhook  │    │ Route     │        │ S3       │         │
│  │ ...      │    └───────────┘        │ ...      │         │
│  └──────────┘                         └──────────┘         │
└─────────────────────────────────────────────────────────────┘
```

## When to Use POLKU

| Scenario | Kafka | POLKU | Direct Call |
|----------|-------|-------|-------------|
| Internal agents → backend | Overkill | ✅ | Brittle |
| Format transformation | ❌ | ✅ | Manual |
| Buffering during slowdowns | Overkill | ✅ | ❌ |
| Fan-out to 2-3 services | Overkill | ✅ | Spaghetti |
| Need replay/persistence | ✅ | ❌ | ❌ |
| 100+ consumers | ✅ | ❌ | ❌ |

**POLKU is not a message broker.** It's a pipeline for when you need decoupling without the infrastructure tax.

---

## The Pitch

| Kafka/NATS | POLKU |
|------------|-------|
| JVM, 1GB+ RAM | Rust, 10-20MB RAM |
| Zookeeper/KRaft cluster | Single binary |
| Persistent storage required | In-memory buffer |
| Complex operations | Zero ops |
| Consumer groups, offsets, partitions | Just send it |

---

## Quick Start

```bash
# Build
cargo build --release

# Run
./target/release/polku-gateway

# Test
cargo test
```

**Environment Variables:**
```bash
POLKU_GRPC_ADDR=0.0.0.0:50051      # gRPC server address
POLKU_BUFFER_CAPACITY=100000       # Event buffer size
POLKU_LOG_LEVEL=info               # Logging level
```

---

## Architecture

### Message Flow

```
1. Input receives data (gRPC stream, REST webhook, etc.)
   └── Converts to internal Message format

2. Pipeline processes the message
   ├── Middleware: auth, rate limit, transform
   └── Buffer: absorb backpressure

3. Outputs receive the message (fan-out)
   └── Each output sends to its destination
```

### The Message Envelope

```rust
pub struct Message {
    pub id: String,                        // Unique ID (ULID)
    pub timestamp: i64,                    // Unix nanos
    pub source: String,                    // Origin identifier
    pub message_type: String,              // User-defined type
    pub metadata: HashMap<String, String>, // Headers, context
    pub payload: Bytes,                    // Opaque payload (zero-copy)
    pub route_to: Vec<String>,             // Output routing hints
}
```

POLKU doesn't interpret your payload - it just carries it. Your Input plugins deserialize, your Output plugins serialize.

### Writing Plugins

**Input Plugin** - receive from a protocol:

```rust
#[async_trait]
pub trait Input: Send + Sync {
    fn name(&self) -> &'static str;
    async fn run(&self, tx: Sender<Message>) -> Result<(), Error>;
}
```

**Output Plugin** - send to a destination:

```rust
#[async_trait]
pub trait Output: Send + Sync {
    fn name(&self) -> &'static str;
    async fn send(&self, messages: &[Message]) -> Result<(), Error>;
    async fn health(&self) -> bool;
}
```

**Middleware** - transform, filter, route:

```rust
#[async_trait]
pub trait Middleware: Send + Sync {
    fn name(&self) -> &'static str;
    async fn process(&self, msg: Message) -> Option<Message>;
}
```

---

## Example: Internal Agent Pipeline

```
┌──────────────────────────────────────────────────────────────┐
│                                                               │
│   Your Edge Agents                Your Backend               │
│   ┌─────────┐                     ┌─────────┐               │
│   │ Agent A │──┐                  │ Backend │               │
│   └─────────┘  │    ┌────────┐    │         │               │
│   ┌─────────┐  ├───►│ POLKU  │───►│         │               │
│   │ Agent B │──┤    │        │    └─────────┘               │
│   └─────────┘  │    │ • buffer during restart               │
│   ┌─────────┐  │    │ • transform formats                   │
│   │ Agent C │──┘    │ • fan-out to OTEL                     │
│   └─────────┘       └────────┘                              │
│                          │                                   │
│                          ▼                                   │
│                     ┌─────────┐                              │
│                     │  OTEL   │                              │
│                     └─────────┘                              │
│                                                               │
│   No Kafka. No NATS. Just a lightweight internal pipeline.   │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

---

## Project Structure

```
polku/
├── Cargo.toml                # Workspace
├── CLAUDE.md                 # Development guide
├── gateway/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           # Entry point
│       ├── lib.rs            # Library exports
│       ├── message.rs        # Message type
│       ├── hub.rs            # Hub builder
│       ├── buffer.rs         # Ring buffer
│       ├── input/            # Input plugins
│       ├── output/           # Output plugins
│       └── middleware/       # Middleware
└── proto/                    # Proto definitions
```

---

## Roadmap

| Phase | Status | Description |
|-------|--------|-------------|
| 1 | ✅ | gRPC event gateway, basic plugins |
| 2 | 🚧 | Generic Message, Middleware, Hub builder |
| 3 | 📋 | REST input, Kafka output |
| 4 | 📋 | WebSocket, MQTT inputs |

---

## Comparison

### vs Envoy/Kong
They're L7 proxies for routing requests. POLKU is a message pipeline for transforming content. They work together - Envoy routes the connection, POLKU handles the payload.

### vs Kafka/NATS
They're full pub/sub systems with persistence, consumer groups, replay. POLKU is a lightweight pipeline for internal routing without the ops burden.

### vs OTEL Collector
It's specifically for telemetry (traces, metrics, logs). POLKU is format-agnostic for any internal messages.

### vs Vector
It's focused on observability data. POLKU is a general-purpose internal pipeline.

---

## Tech Stack

| Crate | Purpose |
|-------|---------|
| `tonic` | gRPC server/client |
| `axum` | REST server (planned) |
| `bytes` | Zero-copy buffers |
| `tokio` | Async runtime |
| `tracing` | Structured logging |
| `parking_lot` | Fast mutex |

---

## Naming

**Polku** (Finnish: "path") - The path messages take through your system.

---

## License

Apache 2.0
