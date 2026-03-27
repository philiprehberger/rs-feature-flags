# rs-feature-flags

[![CI](https://github.com/philiprehberger/rs-feature-flags/actions/workflows/ci.yml/badge.svg)](https://github.com/philiprehberger/rs-feature-flags/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/philiprehberger-feature-flags.svg)](https://crates.io/crates/philiprehberger-feature-flags)
[![License](https://img.shields.io/github/license/philiprehberger/rs-feature-flags)](LICENSE)
[![Sponsor](https://img.shields.io/badge/sponsor-GitHub%20Sponsors-ec6cb9)](https://github.com/sponsors/philiprehberger)

In-memory feature flag evaluation with rollout, environment, targeting, and A/B variant support for Rust

## Installation

```toml
[dependencies]
philiprehberger-feature-flags = "0.2.3"
```

To enable JSON deserialization via serde:

```toml
[dependencies]
philiprehberger-feature-flags = { version = "0.2.3", features = ["serde"] }
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

### Targeting

Grant access to specific users or roles, bypassing percentage rollout:

```rust
use philiprehberger_feature_flags::{FeatureFlags, FlagConfig, Context};

let mut flags = FeatureFlags::new();
flags.set(
    "internal-tool",
    FlagConfig::new(true)
        .with_rollout(0) // 0% general rollout
        .with_allowed_users(vec!["alice".into(), "bob".into()])
        .with_allowed_roles(vec!["admin".into()]),
);

// Alice gets access via allowed_users
let ctx = Context::new().with_user_id("alice");
assert!(flags.is_enabled_for("internal-tool", &ctx));

// Any admin gets access via allowed_roles
let ctx = Context::new().with_user_id("charlie").with_role("admin");
assert!(flags.is_enabled_for("internal-tool", &ctx));
```

You can also require context attributes to match:

```rust
use std::collections::HashMap;
use philiprehberger_feature_flags::{FeatureFlags, FlagConfig, Context};

let mut attrs = HashMap::new();
attrs.insert("plan".to_owned(), "enterprise".to_owned());

let mut flags = FeatureFlags::new();
flags.set(
    "enterprise-only",
    FlagConfig::new(true).with_required_attributes(attrs),
);

let ctx = Context::new().with_attribute("plan", "enterprise");
assert!(flags.is_enabled_for("enterprise-only", &ctx));
```

### Variants

Deterministic A/B test variant assignment based on user ID hashing:

```rust
use philiprehberger_feature_flags::FeatureFlags;

let flags = FeatureFlags::new();
let variant = flags.get_variant("experiment", "user-42", &["control", "variant-a", "variant-b"]);
// Always returns the same variant for the same flag + user pair
println!("assigned: {}", variant.unwrap());
```

## API

| Item | Description |
|------|-------------|
| `FlagConfig` | Configuration for a single flag: enabled, rollout, environments, targeting, variants, attributes |
| `FlagConfig::new(enabled)` | Create a new flag config |
| `.with_rollout(pct)` | Set rollout percentage (0-100) |
| `.with_environments(envs)` | Restrict flag to specific environments |
| `.with_allowed_users(users)` | Set users that bypass rollout |
| `.with_allowed_roles(roles)` | Set roles that bypass rollout (matched against `role` attribute) |
| `.with_variants(variants)` | Set variant names for A/B testing |
| `.with_required_attributes(attrs)` | Set required context attributes |
| `Context` | Evaluation context with user ID, environment, and custom attributes |
| `Context::new()` | Create an empty context |
| `.with_user_id(id)` | Set the user ID for rollout evaluation |
| `.with_environment(env)` | Set the environment for filtering |
| `.with_attribute(key, value)` | Add a custom attribute |
| `.with_role(role)` | Set the role attribute (shorthand) |
| `FeatureFlags` | In-memory flag store |
| `FeatureFlags::new()` | Create an empty store |
| `.set(name, config)` | Add or update a flag |
| `.set_config(name, config)` | Add or update a flag (accepts `&str`) |
| `.remove(name)` | Remove a flag |
| `.is_enabled(name)` | Check if a flag is enabled (no context) |
| `.is_enabled_for(name, ctx)` | Evaluate a flag with full context |
| `.evaluate_with_config(name, ctx)` | Evaluate a stored flag with context |
| `.get_variant(flag, user_id, variants)` | Get a deterministic A/B variant |
| `.all_flags()` | Get a sorted list of all flag names |
| `FeatureFlags::from_json(json)` | Parse flags from JSON (requires `serde` feature) |


## Development

```bash
cargo test
cargo clippy -- -D warnings
```

## License

MIT
