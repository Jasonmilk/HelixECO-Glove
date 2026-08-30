# Helix ECO Glove

> Native OS/device adapters for the Helix ecosystem. All gloves implement the standard `EcoGlove` trait, loadable by Helix-Tentacle with platform-aware gating.

**Current Status**: P1 in progress — EcoGlove trait + macOS Glove minimum viable (26 tests all green)

---

## What is Helix ECO Glove?

Helix ECO Glove is the **native adaptation layer** of the Helix ecosystem. It provides standardized, platform-specific tools (filesystem, process, scripting, sensors, etc.) that can be loaded and executed by Helix-Tentacle.

Each "glove" is a native adapter for a specific platform (macOS, Linux, HarmonyOS, Android, ROS2 robots, etc.), implementing the common `EcoGlove` trait. This allows Tentacle to implement a single plugin loader that works across all platforms.

### Design Philosophy

| Principle | Manifestation |
|---|---|
| **Maximum Decoupling** | Gloves only execute; Tentacle schedules; Anaphase orchestrates; Mind decides |
| **On-Demand Loading** | Only load gloves supported by the current platform (compile-time `cfg` + runtime whitelist) |
| **On-Demand Execution** | Event-driven, no polling. Zero energy consumption when not called |
| **Maximum Reuse** | All gloves share the `EcoGlove` trait; Tentacle implements one loader |
| **Physical Facts First** | Platform, risk level, and capabilities are physical facts, not forgeable |
| **Determinism First** | Same input produces same output; no hidden state or side effects (unless explicitly declared) |

---

## Project Structure

```
HelixECO-Glove/
├── Cargo.toml              # Workspace root
├── README.md               # English (this file)
├── README.zh-cn.md         # Chinese version
├── core/                   # Standard interface definition
│   ├── Cargo.toml
│   └── src/lib.rs          # EcoGlove trait + all standard types
├── gloves/                 # Platform-specific glove implementations
│   └── macos/              # macOS native glove
│       ├── Cargo.toml
│       └── src/lib.rs
├── tests/                  # Integration tests
└── docs/
    ├── DNA.md              # Constitution: 6 immutable principles
    ├── RNA.md              # Loading protocol: how AI reads this repo
    ├── PLAN.md             # Navigation: current phase + next phase preview
    ├── GROWTH.md           # Growth records: last 3 health snapshots
    └── decisions/          # Architecture Decision Records (ADRs)
```

---

## EcoGlove Trait Overview

The `EcoGlove` trait is the constitution of all gloves. Every glove must implement:

### Metadata (static, physical facts)

| Method | Description |
|---|---|
| `platform()` | Platform identifier (macOS, Linux, Harmony, Robot, etc.) |
| `host_os()` | List of supported host OSes (for Tentacle's whitelist filtering) |
| `version()` | Semantic version |
| `name()` | Glove name (e.g., "macos-glove") |
| `description()` | Glove description |

### Tool Definitions

| Method | Description |
|---|---|
| `tools()` | List of `ToolDefinition` — name, description, domain, risk level, parameters, return description |
| `capabilities()` | List of capability domains (derived from tools by default) |
| `get_tool(name)` | Look up a tool by name |

### Execution

| Method | Description |
|---|---|
| `execute(tool, params, dry_run)` | Execute a tool asynchronously. Returns `ToolResult` |
| `validate_parameters(tool, params)` | Validate parameters before execution (default implementation provided) |

### Lifecycle

| Method | Description |
|---|---|
| `initialize()` | Initialize the glove (load config, establish connections) |
| `shutdown()` | Clean up resources |

### Platform Gating

| Method | Description |
|---|---|
| `is_supported_on_current_platform()` | Check if the glove is supported on the current platform |

---

## Tool Naming Convention

All tools use **dot-separated namespaces**:

```
<platform>.<domain>.<action>
```

Examples:

| Tool Name | Platform | Domain | Action | Risk |
|---|---|---|---|---|
| `macos.fs.read_file` | macOS | filesystem | read_file | LOW |
| `macos.fs.write_file` | macOS | filesystem | write_file | MEDIUM |
| `macos.fs.list_directory` | macOS | filesystem | list_directory | LOW |
| `macos.process.execute` | macOS | process | execute | CRITICAL |
| `macos.process.list` | macOS | process | list | LOW |
| `macos.script.applescript` | macOS | script | applescript | CRITICAL |

---

## Risk Levels

Aligned with CI-144 PFP (Physical Feature Protocol):

| Level | Meaning | Examples | Requires Confirmation |
|---|---|---|---|
| **LOW** | Read-only operations | read_file, list_directory, list_processes | No |
| **MEDIUM** | Write operations | write_file, create_directory | No |
| **CRITICAL** | May affect system stability | execute_command, delete_file | Yes |
| **CATASTROPHIC** | May cause irreversible damage | system_update, hardware_control | Yes (multi-factor) |

---

## macOS Glove (Current Implementation)

6 native macOS tools:

| Tool | Description | Risk | dry_run |
|---|---|---|---|
| `macos.fs.read_file` | Read a file from the filesystem | LOW | ✅ |
| `macos.fs.write_file` | Write content to a file | MEDIUM | ✅ |
| `macos.fs.list_directory` | List files in a directory | LOW | ✅ |
| `macos.process.execute` | Execute a shell command | CRITICAL | ❌ |
| `macos.process.list` | List running processes | LOW | ✅ |
| `macos.script.applescript` | Execute an AppleScript | CRITICAL | ❌ |

### Quick Example

```rust
use helix_eco_glove_core::*;
use helix_eco_glove_macos::MacOSGlove;

#[tokio::main]
async fn main() -> Result<(), GloveError> {
    let glove = MacOSGlove::new();
    glove.initialize()?;

    // Read a file
    let result = glove
        .execute(
            "macos.fs.read_file",
            serde_json::json!({"path": "/etc/hosts"}),
            false,
        )
        .await?;

    println!("Success: {}", result.success);
    println!("Content: {}", result.data.unwrap()["content"]);

    glove.shutdown()?;
    Ok(())
}
```

---

## Platform Isolation (Dual-Layer Gating)

### Layer 1: Compile-Time Exclusion

```rust
// In Tentacle's Cargo.toml or code:
#[cfg(target_os = "macos")]
use helix_eco_glove_macos::MacOSGlove;

#[cfg(target_os = "linux")]
use helix_eco_glove_linux::LinuxGlove;
```

### Layer 2: Runtime Whitelist

```rust
// Tentacle's plugin loader:
let current_os = std::env::consts::OS;
for glove in discovered_gloves {
    if glove.is_supported_on_current_platform() {
        registry.register(glove);
    } else {
        tracing::info!("Skipping glove {}: only supports {:?}", glove.name(), glove.host_os());
    }
}
```

---

## Testing

```bash
# Build the entire workspace
cargo build

# Run all tests
cargo test

# Test only core
cargo test -p helix-eco-glove-core

# Test only macOS glove
cargo test -p helix-eco-glove-macos

# Check code formatting
cargo fmt --check

# Run clippy static analysis
cargo clippy --all-targets -- -D warnings
```

**Current test results**: 26 tests all green (12 core unit + 13 macOS unit + 1 doc test)

---

## Helix Ecosystem

| Project | Role | Status |
|---|---|---|
| [Helix-Mind](https://github.com/Jasonmilk/Helix-Mind) | Brain (memory/cognition) | ✅ Core complete |
| [Anaphase-Helix](https://github.com/Jasonmilk/Anaphase-Helix) | Torso (orchestration/execution) | ✅ Pending decision |
| [Helix-Tentacle](https://github.com/Jasonmilk/Helix-Tentacle) | Hand (tool execution) | ✅ Complete |
| [Tuck](https://github.com/Jasonmilk/Tuck) | Immune System (security gate) | ✅ Complete |
| [Cellrix](https://github.com/Jasonmilk/Cellrix) | Skin (UI/display) | ✅ Complete |
| [BIND-19](https://github.com/CommonIntents/BIND-19) | Nervous System (CI-144 protocol) | ✅ Complete |
| [Helix-MCP-Learner](https://github.com/Jasonmilk/Helix-MCP-Learner) | Translator (MCP → CI-144) | ✅ P2 Complete |
| **HelixECO-Glove** | **Native Adapters (OS/device gloves)** | **🚧 P1 in progress** |

---

## Governance

This project follows **phyt-DNA Methodology v1.0**.

| Document | Purpose |
|---|---|
| [docs/DNA.md](docs/DNA.md) | Constitution: 6 immutable principles |
| [docs/RNA.md](docs/RNA.md) | Loading protocol: how AI reads this repository |
| [docs/PLAN.md](docs/PLAN.md) | Navigation: current phase + next phase preview |
| [docs/GROWTH.md](docs/GROWTH.md) | Growth records: last 3 health snapshots |
| [docs/decisions/](docs/decisions/) | Architecture Decision Records (ADRs) |

---

## Roadmap

| Phase | Content | Status |
|---|---|---|
| **P1** | **Project init + EcoGlove trait + macOS Glove minimum** | **🚧 in progress** |
| P2 | Tentacle integration + platform-aware loader | ⏳ Preview |
| P3 | MCP-Learner deprecation + Review System L1 (static checks) | ⏳ Preview |
| P4 | Review System L2 (dry_run sandbox) + rule self-evolution | ⏳ Preview |
| P5 | New platform gloves (Linux/Harmony/Robot) + Review System L3 | ⏳ Preview |

---

## License

MIT OR Apache-2.0

---

**Author**: Jasonmilk / CommonIntents
**Created**: 2026-08-31
