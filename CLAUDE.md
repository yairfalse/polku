# POLKU: Lightweight Internal Message Pipeline

**Decouple your internal services without a message broker**

---

## PROJECT NATURE

**THIS IS A MESSAGE PIPELINE, NOT AN API GATEWAY**

- **Goal**: Transform, buffer, and route messages between internal services
- **Language**: 100% Rust
- **Positioning**: For when Kafka is overkill but direct coupling is brittle
- **NOT**: An API gateway (Envoy), a message broker (Kafka), or telemetry-specific (OTEL)

---

## THE PITCH

```
┌─────────────────────────────────────────────────────────────┐
│                                                              │
│  "I have internal services that need to talk,               │
│   but Kafka is overkill and direct calls are brittle"       │
│                                                              │
│   Solution: POLKU                                            │
│   • Single binary, 10-20MB RAM                               │
│   • Zero ops (no Zookeeper, no cluster)                      │
│   • Transform between formats                                │
│   • Buffer during slowdowns                                  │
│   • Fan-out to multiple destinations                         │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## ARCHITECTURE

```
┌─────────────────────────────────────────────────────────────┐
│                         POLKU                                │
│                                                              │
│  Inputs              Middleware           Outputs            │
│  ┌──────────┐       ┌──────────┐        ┌──────────┐        │
│  │ gRPC     │──────►│ Transform│───────►│ gRPC     │        │
│  │ REST     │       │ Auth     │        │ Kafka    │        │
│  │ Webhook  │       │ Route    │        │ S3       │        │
│  └──────────┘       └──────────┘        └──────────┘        │
│                          │                                   │
│                     ┌────▼────┐                              │
│                     │ Buffer  │                              │
│                     │ (Ring)  │                              │
│                     └─────────┘                              │
└─────────────────────────────────────────────────────────────┘
```

### Core Types

```rust
// The universal message - protocol agnostic, zero-copy
pub struct Message {
    pub id: String,                        // ULID
    pub timestamp: i64,                    // Unix nanos
    pub source: String,                    // Origin
    pub message_type: String,              // User-defined
    pub metadata: HashMap<String, String>, // Headers
    pub payload: Bytes,                    // Zero-copy payload
    pub route_to: Vec<String>,             // Output hints
}

// Input: receive from any protocol
#[async_trait]
pub trait Input: Send + Sync {
    fn name(&self) -> &'static str;
    async fn run(&self, tx: Sender<Message>) -> Result<(), Error>;
}

// Output: send to any destination
#[async_trait]
pub trait Output: Send + Sync {
    fn name(&self) -> &'static str;
    async fn send(&self, messages: &[Message]) -> Result<(), Error>;
    async fn health(&self) -> bool;
}

// Middleware: transform, filter, route
#[async_trait]
pub trait Middleware: Send + Sync {
    fn name(&self) -> &'static str;
    async fn process(&self, msg: Message) -> Option<Message>;
}
```

---

## RUST REQUIREMENTS

### Absolute Rules

1. **No `.unwrap()` in production** - Use `?` or proper error handling
2. **No `println!`** - Use `tracing::{info, warn, error, debug}`
3. **No TODOs or stubs** - Complete implementations only
4. **Use `Bytes` for payloads** - Zero-copy, reference counted

### Error Handling

```rust
// BAD
let msg = batch.messages.first().unwrap();

// GOOD
let msg = batch.messages.first()
    .ok_or(Error::EmptyBatch)?;
```

### Zero-Copy with Bytes

```rust
// BAD: cloning data
let data: Vec<u8> = input.read_all();
let copy = data.clone();  // Expensive!

// GOOD: zero-copy
let data: Bytes = input.read_all();
let reference = data.clone();  // Just increments refcount
```

---

## TDD WORKFLOW

**RED → GREEN → REFACTOR**

```rust
#[tokio::test]
async fn test_middleware_transforms_message() {
    let middleware = TransformMiddleware::new(|msg| {
        msg.metadata.insert("processed".into(), "true".into());
        msg
    });

    let msg = Message::new("test", Bytes::from("payload"));
    let result = middleware.process(msg).await;

    assert!(result.is_some());
    assert_eq!(result.unwrap().metadata.get("processed"), Some(&"true".into()));
}
```

---

## FILE LOCATIONS

| What | Where |
|------|-------|
| Entry point | `gateway/src/main.rs` |
| Message type | `gateway/src/message.rs` |
| Hub builder | `gateway/src/hub.rs` |
| Ring buffer | `gateway/src/buffer.rs` |
| Input trait | `gateway/src/input/mod.rs` |
| Output trait | `gateway/src/output/mod.rs` |
| Middleware trait | `gateway/src/middleware/mod.rs` |
| gRPC input | `gateway/src/input/grpc.rs` |
| Stdout output | `gateway/src/output/stdout.rs` |

---

## PHASE 2 IMPLEMENTATION

Current task: Generalize from Event to Message

### Changes Required

1. **Create `message.rs`** - Generic Message with Bytes payload
2. **Update `buffer.rs`** - Use Message instead of Event
3. **Create `middleware/mod.rs`** - Middleware trait
4. **Create `hub.rs`** - Builder pattern for pipeline setup
5. **Rename traits** - InputPlugin → Input, OutputPlugin → Output
6. **Update proto** - Simplify to generic envelope

### Builder API Goal

```rust
Hub::new()
    .input(GrpcInput::new("[::]:50051"))
    .middleware(Transform::new(|msg| { ... }))
    .output("backend", GrpcOutput::new("backend:50051"))
    .run()
    .await
```

---

## DEPENDENCIES

| Crate | Purpose |
|-------|---------|
| `tonic` | gRPC server/client |
| `prost` | Protobuf serialization |
| `bytes` | Zero-copy buffers |
| `tokio` | Async runtime |
| `tracing` | Structured logging |
| `parking_lot` | Fast mutex |
| `thiserror` | Error types |
| `async-trait` | Async trait support |

---

## VERIFICATION

Before every commit:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

---

## AGENT INSTRUCTIONS

1. **Read first** - Understand existing patterns
2. **TDD always** - Write failing test, implement, refactor
3. **Use Bytes** - Zero-copy for all payloads
4. **No YAML** - Programmatic configuration via Builder
5. **Run checks** - `cargo fmt && cargo clippy && cargo test`

---

## POSITIONING REMINDER

**POLKU is NOT:**
- An API gateway (use Envoy/Kong)
- A message broker (use Kafka/NATS)
- A service mesh (use Istio/Linkerd)
- Telemetry-specific (use OTEL Collector)

**POLKU IS:**
- A lightweight internal message pipeline
- For decoupling without infrastructure tax
- When Kafka is overkill
- Single binary, zero ops
