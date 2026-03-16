# rs-feature-flags

[![CI](https://github.com/philiprehberger/rs-feature-flags/actions/workflows/ci.yml/badge.svg)](https://github.com/philiprehberger/rs-feature-flags/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/philiprehberger-feature-flags.svg)](https://crates.io/crates/philiprehberger-feature-flags)
[![License](https://img.shields.io/github/license/philiprehberger/rs-feature-flags)](LICENSE)

In-memory feature flag evaluation with rollout and environment support for Rust.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
philiprehberger-feature-flags = "0.1.4"
```

To enable JSON deserialization via serde:

```toml
[dependencies]
philiprehberger-feature-flags = { version = "0.1", features = ["serde"] }
```

## Usage

```rust
use philiprehberger_feature_flags::{FeatureFlags, FlagConfig, Context};

let mut flags = FeatureFlags::new();

flags.set("dark-mode", FlagConfig::new(true));
flags.set(
    "beta-feature",
    FlagConfig::new(true)
        .with_rollout(50)
        .with_environments(vec!["production".into()]),
);

// Simple check
assert!(flags.is_enabled("dark-mode"));

// Check with context
let ctx = Context::new()
    .with_user_id("user-123")
    .with_environment("production");

let enabled = flags.is_enabled_for("beta-feature", &ctx);
println!("beta-feature enabled: {enabled}");
```

## API

| Item | Description |
|------|-------------|
| `FlagConfig` | Configuration for a single flag: enabled state, optional rollout percentage, optional environment list |
| `FlagConfig::new(enabled)` | Create a new flag config |
| `.with_rollout(pct)` | Set rollout percentage (0-100) |
| `.with_environments(envs)` | Restrict flag to specific environments |
| `Context` | Evaluation context with user ID, environment, and custom attributes |
| `Context::new()` | Create an empty context |
| `.with_user_id(id)` | Set the user ID for rollout evaluation |
| `.with_environment(env)` | Set the environment for filtering |
| `.with_attribute(key, value)` | Add a custom attribute |
| `FeatureFlags` | In-memory flag store |
| `FeatureFlags::new()` | Create an empty store |
| `.set(name, config)` | Add or update a flag |
| `.remove(name)` | Remove a flag |
| `.is_enabled(name)` | Check if a flag is enabled (no context) |
| `.is_enabled_for(name, ctx)` | Evaluate a flag with full context |
| `.all_flags()` | Get a sorted list of all flag names |
| `FeatureFlags::from_json(json)` | Parse flags from JSON (requires `serde` feature) |

## License

MIT
