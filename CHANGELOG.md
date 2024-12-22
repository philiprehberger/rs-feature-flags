# Changelog

## 0.2.0 (2026-03-17)

- Add user targeting (`allowed_users`) and role targeting (`allowed_roles`) to `FlagConfig`
- Add attribute-based evaluation with `required_attributes` on `FlagConfig`
- Add A/B test variant support via `get_variant()` with deterministic hashing
- Add `set_config()` and `evaluate_with_config()` methods on `FeatureFlags`
- Add `with_role()` convenience builder on `Context`
- Evaluation order: environment -> required attributes -> allowed users -> allowed roles -> rollout -> enabled

## 0.1.5

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
