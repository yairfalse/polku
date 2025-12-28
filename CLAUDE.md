# POLKU: Pluggable gRPC Event Gateway

**POLKU = The path events take from edge to brain**

---

## PROJECT NATURE

**THIS IS A PLUGGABLE EVENT GATEWAY**
- **Goal**: Transform and route events from edge agents to AHTI
- **Language**: 100% Rust
- **Status**: Phase 1 - Project setup
- **Approach**: Plugin in, plugin out - both sides are traits

---

## PROJECT MISSION

**Mission**: Build a high-performance gRPC gateway that transforms events from various sources into a unified format.

**Core Value Proposition:**

**"The universal translator between your observability agents and AHTI"**

**The Architecture:**
```
┌─────────────────────────────────────────────────────────────┐
│                         POLKU                                │
│                                                              │
│  Input Plugins        Core              Output Plugins       │
│  ┌──────────┐    ┌───────────┐        ┌──────────┐         │
│  │  Tapio   │───►│ Transform │───────►│   AHTI   │         │
│  │  Portti  │    │  Buffer   │        │   OTEL   │         │
│  │  Elava   │    │  Route    │        │   File   │         │
│  │  ...     │    └───────────┘        │   ...    │         │
│  └──────────┘                         └──────────┘         │
└─────────────────────────────────────────────────────────────┘
```

**Key Design Principles:**
- **Pluggable inputs** - Add new sources without changing core
- **Pluggable outputs** - Add new destinations without changing core
- **Backpressure** - Ring buffer with FIFO eviction
- **No state** - Stateless transformation, state lives in AHTI

---

## RUST REQUIREMENTS

### Absolute Rules

1. **No `.unwrap()` in production** - Use `?` or proper error handling
2. **No `println!`** - Use `tracing::{info, warn, error, debug}`
3. **No TODOs or stubs** - Complete implementations only

### Error Handling Pattern

```rust
// BAD
let event = batch.events.first().unwrap();

// GOOD
let event = batch.events.first()
    .ok_or(PolkuError::EmptyBatch)?;
```

### Tracing Pattern

```rust
// BAD
println!("Processing {} events", count);

// GOOD
info!(count = %count, source = %source, "Processing event batch");
warn!(error = ?e, "Failed to forward to AHTI (will retry)");
error!(error = ?e, "Plugin initialization failed");
```

---

## TDD WORKFLOW

**RED → GREEN → REFACTOR** - Always.

### RED: Write Failing Test First

```rust
#[tokio::test]
async fn test_buffer_drops_oldest_on_overflow() {
    let buffer = RingBuffer::new(3);

    // Push 5 events into buffer of size 3
    let events = (0..5).map(|i| make_event(&format!("e{i}"))).collect();
    let dropped = buffer.push(events);

    assert_eq!(dropped, 2);
    assert_eq!(buffer.len(), 3);
}
```

### GREEN: Minimal Implementation

Write just enough code to make the test pass. No more.

### REFACTOR: Clean Up

Extract helpers, improve naming, add edge cases. Tests must still pass.

---

## KEY PATTERNS

### Plugin Traits

```rust
// Input plugin - transforms raw bytes to Events
#[async_trait]
pub trait InputPlugin: Send + Sync {
    fn name(&self) -> &'static str;
    fn transform(&self, source: &str, data: &[u8]) -> Result<Vec<Event>, PluginError>;
}

// Output plugin - sends Events to destination
#[async_trait]
pub trait OutputPlugin: Send + Sync {
    fn name(&self) -> &'static str;
    async fn send(&self, events: &[Event]) -> Result<(), PluginError>;
    async fn health(&self) -> bool;
}
```

### Ring Buffer for Backpressure

```rust
impl RingBuffer {
    pub fn push(&self, events: Vec<Event>) -> usize; // returns dropped count
    pub fn drain(&self, n: usize) -> Vec<Event>;
    pub fn len(&self) -> usize;
}
```

---

## VERIFICATION CHECKLIST

Before every commit:

```bash
# Format
cargo fmt

# Lint
cargo clippy -- -D warnings

# Tests
cargo test

# No unwrap in production
grep -r "\.unwrap()" src/ --include="*.rs" | grep -v "#\[test\]" | grep -v "#\[cfg(test)\]"
```

---

## FILE LOCATIONS

| What | Where |
|------|-------|
| Main entry point | `gateway/src/main.rs` |
| gRPC server | `gateway/src/server.rs` |
| Ring buffer | `gateway/src/buffer.rs` |
| Input plugin trait | `gateway/src/input/mod.rs` |
| Output plugin trait | `gateway/src/output/mod.rs` |
| Tapio adapter | `gateway/src/input/tapio.rs` |
| AHTI output | `gateway/src/output/ahti.rs` |

---

## DEPENDENCIES

| Crate | Purpose |
|-------|---------|
| `tonic` | gRPC server/client |
| `prost` | Protobuf serialization |
| `tokio` | Async runtime |
| `tracing` | Structured logging |
| `prometheus` | Metrics |
| `thiserror` | Error types |
| `parking_lot` | Fast mutex for buffer |

---

## PROTO ARCHITECTURE

POLKU imports protos from central repo:

```
falsesystems/proto/           # Central proto repo
├── tapio/v1/raw.proto        # TAPIO → POLKU
├── portti/v1/raw.proto       # PORTTI → POLKU
├── ahti/v1/events.proto      # POLKU → AHTI (AhtiEvent)
└── polku/v1/gateway.proto    # POLKU service definition
```

POLKU transforms between schemas:
- `tapio::RawEbpfEvent` → `ahti::AhtiEvent`
- `portti::RawK8sEvent` → `ahti::AhtiEvent`
- `elava::RawCloudEvent` → `ahti::AhtiEvent`

---

## AGENT INSTRUCTIONS

When working on this codebase:

1. **Read first** - Understand existing patterns before changing
2. **TDD always** - Write failing test, implement, refactor
3. **Plugins are traits** - Both input and output are pluggable
4. **No state** - POLKU is stateless, AHTI has the graph
5. **Run checks** - `cargo fmt && cargo clippy && cargo test`

---

**False Systems** 🇫🇮
