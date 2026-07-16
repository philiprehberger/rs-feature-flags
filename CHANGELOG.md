# Changelog

## 0.4.0 (2026-07-15)

- Add `FeatureFlags::variant_for()` to resolve an A/B variant from a flag's stored `variants` (set via `with_variants`) using the user ID from a `Context`, closing the gap where configured variants were previously unused by evaluation
- Add store introspection and mutation: `get()`, `contains()`, `set_enabled()`, `len()`, `is_empty()`, and `clear()`
- Add `FeatureFlags::to_json()` and derive `Serialize` on `FlagConfig` for round-tripping flags to JSON (requires `serde` feature)

## 0.3.0 (2026-06-14)

- Add `FlagConfig::disallowed_users` denylist and `with_disallowed_users()` builder — takes precedence over `allowed_users`, `allowed_roles`, and rollout
- Fix stale install version in README install snippet

## 0.2.5 (2026-03-31)

- Standardize README to 3-badge format with emoji Support section
- Update CI checkout action to v5 for Node.js 24 compatibility

## 0.2.4 (2026-03-27)

- Add GitHub issue templates, PR template, and dependabot configuration
- Update README badges and add Support section

## 0.2.3 (2026-03-22)

- Fix stale version in serde install snippet

## 0.2.2 (2026-03-22)

- Fix CHANGELOG compliance

## 0.2.1 (2026-03-17)

- Add crate-level documentation with usage examples

## 0.2.0 (2026-03-17)

- Add user targeting (`allowed_users`) and role targeting (`allowed_roles`) to `FlagConfig`
- Add attribute-based evaluation with `required_attributes` on `FlagConfig`
- Add A/B test variant support via `get_variant()` with deterministic hashing
- Add `set_config()` and `evaluate_with_config()` methods on `FeatureFlags`
- Add `with_role()` convenience builder on `Context`
- Evaluation order: environment -> required attributes -> allowed users -> allowed roles -> rollout -> enabled

## 0.1.5 (2026-03-17)

- Add readme, rust-version, documentation to Cargo.toml
- Add Development section to README

## 0.1.4 (2026-03-16)

- Update install snippet to use full version

## 0.1.3 (2026-03-16)

- Add README badges
- Synchronize version across Cargo.toml, README, and CHANGELOG

## 0.1.0 (2026-03-13)

- Initial release
- In-memory feature flag store
- Environment-based flag filtering
- Percentage-based rollout with deterministic hashing
- Optional serde support behind `serde` feature flag
- Context builder pattern for evaluation
