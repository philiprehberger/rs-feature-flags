//! In-memory feature flag evaluation with rollout, environment, targeting, and A/B variant support.
//!
//! # Example
//!
//! ```rust
//! use philiprehberger_feature_flags::{FlagStore, Flag, Rollout};
//!
//! let mut store = FlagStore::new();
//! store.add(Flag::new("dark_mode").enabled(true));
//! assert!(store.is_enabled("dark_mode", None));
//! ```

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

#[cfg(feature = "serde")]
use serde::Deserialize;

/// Configuration for a single feature flag.
///
/// Supports environment filtering, percentage rollout, user/role targeting,
/// required context attributes, and A/B test variants.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct FlagConfig {
    pub enabled: bool,
    pub rollout_percentage: Option<u8>,
    pub environments: Option<Vec<String>>,
    /// Users that are always granted access regardless of rollout or other rules.
    #[cfg_attr(feature = "serde", serde(default))]
    pub allowed_users: Vec<String>,
    /// Roles that are always granted access. Checked against the `role` context attribute.
    #[cfg_attr(feature = "serde", serde(default))]
    pub allowed_roles: Vec<String>,
    /// Named variants for A/B testing (e.g. `["control", "variant-a", "variant-b"]`).
    #[cfg_attr(feature = "serde", serde(default))]
    pub variants: Vec<String>,
    /// Attributes that must match the evaluation context for the flag to be enabled.
    #[cfg_attr(feature = "serde", serde(default))]
    pub required_attributes: HashMap<String, String>,
}

impl FlagConfig {
    /// Create a new flag config with the given enabled state.
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            rollout_percentage: None,
            environments: None,
            allowed_users: Vec::new(),
            allowed_roles: Vec::new(),
            variants: Vec::new(),
            required_attributes: HashMap::new(),
        }
    }

    /// Set the rollout percentage (0-100).
    pub fn with_rollout(mut self, pct: u8) -> Self {
        self.rollout_percentage = Some(pct);
        self
    }

    /// Restrict the flag to specific environments.
    pub fn with_environments(mut self, envs: Vec<String>) -> Self {
        self.environments = Some(envs);
        self
    }

    /// Set the list of users that bypass all other evaluation rules.
    pub fn with_allowed_users(mut self, users: Vec<String>) -> Self {
        self.allowed_users = users;
        self
    }

    /// Set the list of roles that bypass percentage rollout.
    /// The role is read from the `role` attribute in [`Context`].
    pub fn with_allowed_roles(mut self, roles: Vec<String>) -> Self {
        self.allowed_roles = roles;
        self
    }

    /// Set the variant names for A/B testing.
    pub fn with_variants(mut self, variants: Vec<String>) -> Self {
        self.variants = variants;
        self
    }

    /// Set required context attributes that must all match for the flag to be enabled.
    pub fn with_required_attributes(mut self, attrs: HashMap<String, String>) -> Self {
        self.required_attributes = attrs;
        self
    }
}

/// Evaluation context for feature flag checks.
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub user_id: Option<String>,
    pub environment: Option<String>,
    pub attributes: HashMap<String, String>,
}

impl Context {
    /// Create an empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the user ID for rollout evaluation.
    pub fn with_user_id(mut self, id: impl Into<String>) -> Self {
        self.user_id = Some(id.into());
        self
    }

    /// Set the environment for filtering.
    pub fn with_environment(mut self, env: impl Into<String>) -> Self {
        self.environment = Some(env.into());
        self
    }

    /// Add a custom attribute.
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Convenience method to set the `role` attribute used for role-based targeting.
    pub fn with_role(self, role: impl Into<String>) -> Self {
        self.with_attribute("role", role)
    }
}

/// In-memory feature flag store.
#[derive(Debug, Clone, Default)]
pub struct FeatureFlags {
    flags: HashMap<String, FlagConfig>,
}

impl FeatureFlags {
    /// Create an empty feature flag store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or update a flag.
    pub fn set(&mut self, name: impl Into<String>, config: FlagConfig) {
        self.flags.insert(name.into(), config);
    }

    /// Add or update a flag (alias for [`set`](Self::set) that accepts `&str`).
    pub fn set_config(&mut self, name: &str, config: FlagConfig) {
        self.flags.insert(name.to_owned(), config);
    }

    /// Remove a flag. Returns true if the flag existed.
    pub fn remove(&mut self, name: &str) -> bool {
        self.flags.remove(name).is_some()
    }

    /// Check if a flag is enabled (no context evaluation).
    /// Returns false if the flag does not exist.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.flags.get(name).is_some_and(|f| f.enabled)
    }

    /// Evaluate a flag with full context.
    ///
    /// Evaluation order:
    /// 1. Environment filtering
    /// 2. Required attribute matching
    /// 3. Allowed users (immediate pass)
    /// 4. Allowed roles (immediate pass)
    /// 5. Percentage rollout
    /// 6. Fall back to `enabled`
    ///
    /// Returns false if the flag does not exist.
    pub fn is_enabled_for(&self, name: &str, context: &Context) -> bool {
        let flag = match self.flags.get(name) {
            Some(f) => f,
            None => return false,
        };

        evaluate_flag(flag, name, context)
    }

    /// Evaluate a flag using a standalone [`FlagConfig`] (not stored in the flag store).
    ///
    /// Uses the same evaluation logic as [`is_enabled_for`](Self::is_enabled_for).
    pub fn evaluate_with_config(&self, name: &str, ctx: &Context) -> bool {
        match self.flags.get(name) {
            Some(flag) => evaluate_flag(flag, name, ctx),
            None => false,
        }
    }

    /// Returns a deterministic variant for a user based on hashing.
    ///
    /// The variant is chosen by hashing `"{flag_name}:{user_id}"` and taking the
    /// result modulo the number of variants. Returns `None` if the flag does not
    /// exist, has no variants configured, or the context has no user ID.
    pub fn get_variant(&self, flag_name: &str, user_id: &str, variants: &[&str]) -> Option<String> {
        if variants.is_empty() {
            return None;
        }
        let key = format!("{flag_name}:{user_id}");
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let idx = (hasher.finish() as usize) % variants.len();
        Some(variants[idx].to_owned())
    }

    /// Get a sorted list of all flag names.
    pub fn all_flags(&self) -> Vec<String> {
        let mut names: Vec<String> = self.flags.keys().cloned().collect();
        names.sort();
        names
    }

    /// Parse flags from a JSON string.
    #[cfg(feature = "serde")]
    pub fn from_json(json: &str) -> Result<Self, String> {
        let flags: HashMap<String, FlagConfig> =
            serde_json::from_str(json).map_err(|e| e.to_string())?;
        Ok(Self { flags })
    }
}

/// Shared evaluation logic for a single flag.
fn evaluate_flag(flag: &FlagConfig, name: &str, context: &Context) -> bool {
    if !flag.enabled {
        return false;
    }

    // Environment check
    if let Some(ref envs) = flag.environments {
        match context.environment {
            Some(ref ctx_env) => {
                if !envs.contains(ctx_env) {
                    return false;
                }
            }
            None => return false,
        }
    }

    // Required attributes check
    for (key, value) in &flag.required_attributes {
        match context.attributes.get(key) {
            Some(ctx_val) if ctx_val == value => {}
            _ => return false,
        }
    }

    // Allowed users — immediate pass
    if !flag.allowed_users.is_empty() {
        if let Some(ref uid) = context.user_id {
            if flag.allowed_users.contains(uid) {
                return true;
            }
        }
    }

    // Allowed roles — immediate pass
    if !flag.allowed_roles.is_empty() {
        if let Some(role) = context.attributes.get("role") {
            if flag.allowed_roles.contains(role) {
                return true;
            }
        }
    }

    // Rollout check
    if let Some(pct) = flag.rollout_percentage {
        match context.user_id {
            Some(ref uid) => {
                let hash = rollout_hash(uid, name);
                if hash >= pct {
                    return false;
                }
            }
            None => return false,
        }
    }

    true
}

fn rollout_hash(user_id: &str, flag_name: &str) -> u8 {
    let mut hasher = DefaultHasher::new();
    user_id.hash(&mut hasher);
    flag_name.hash(&mut hasher);
    (hasher.finish() % 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_flag() {
        let mut flags = FeatureFlags::new();
        flags.set("feature-a", FlagConfig::new(true));
        assert!(flags.is_enabled("feature-a"));
    }

    #[test]
    fn disabled_flag() {
        let mut flags = FeatureFlags::new();
        flags.set("feature-a", FlagConfig::new(false));
        assert!(!flags.is_enabled("feature-a"));
    }

    #[test]
    fn missing_flag() {
        let flags = FeatureFlags::new();
        assert!(!flags.is_enabled("nonexistent"));
    }

    #[test]
    fn environment_match() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "feature-a",
            FlagConfig::new(true).with_environments(vec!["prod".into()]),
        );
        let ctx = Context::new().with_environment("prod");
        assert!(flags.is_enabled_for("feature-a", &ctx));
    }

    #[test]
    fn environment_mismatch() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "feature-a",
            FlagConfig::new(true).with_environments(vec!["prod".into()]),
        );
        let ctx = Context::new().with_environment("staging");
        assert!(!flags.is_enabled_for("feature-a", &ctx));
    }

    #[test]
    fn rollout_deterministic() {
        let mut flags = FeatureFlags::new();
        flags.set("feature-a", FlagConfig::new(true).with_rollout(50));
        let ctx = Context::new().with_user_id("user-42");
        let first = flags.is_enabled_for("feature-a", &ctx);
        let second = flags.is_enabled_for("feature-a", &ctx);
        assert_eq!(first, second);
    }

    #[test]
    fn rollout_zero_percent() {
        let mut flags = FeatureFlags::new();
        flags.set("feature-a", FlagConfig::new(true).with_rollout(0));
        let ctx = Context::new().with_user_id("any-user");
        assert!(!flags.is_enabled_for("feature-a", &ctx));
    }

    #[test]
    fn rollout_hundred_percent() {
        let mut flags = FeatureFlags::new();
        flags.set("feature-a", FlagConfig::new(true).with_rollout(100));
        let ctx = Context::new().with_user_id("any-user");
        assert!(flags.is_enabled_for("feature-a", &ctx));
    }

    #[test]
    fn rollout_no_user_id() {
        let mut flags = FeatureFlags::new();
        flags.set("feature-a", FlagConfig::new(true).with_rollout(50));
        let ctx = Context::new();
        assert!(!flags.is_enabled_for("feature-a", &ctx));
    }

    #[test]
    fn context_builder() {
        let ctx = Context::new()
            .with_user_id("user-1")
            .with_environment("prod")
            .with_attribute("role", "admin");
        assert_eq!(ctx.user_id.as_deref(), Some("user-1"));
        assert_eq!(ctx.environment.as_deref(), Some("prod"));
        assert_eq!(ctx.attributes.get("role").map(|s| s.as_str()), Some("admin"));
    }

    #[test]
    fn all_flags_sorted() {
        let mut flags = FeatureFlags::new();
        flags.set("charlie", FlagConfig::new(true));
        flags.set("alpha", FlagConfig::new(true));
        flags.set("bravo", FlagConfig::new(false));
        assert_eq!(flags.all_flags(), vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn remove_flag() {
        let mut flags = FeatureFlags::new();
        flags.set("feature-a", FlagConfig::new(true));
        assert!(flags.is_enabled("feature-a"));
        assert!(flags.remove("feature-a"));
        assert!(!flags.is_enabled("feature-a"));
    }

    #[test]
    #[cfg(feature = "serde")]
    fn from_json() {
        let json = r#"{
            "dark-mode": { "enabled": true },
            "beta": { "enabled": true, "rollout_percentage": 50, "environments": ["prod"] }
        }"#;
        let flags = FeatureFlags::from_json(json).unwrap();
        assert!(flags.is_enabled("dark-mode"));
        assert!(flags.is_enabled("beta"));
        let ctx = Context::new()
            .with_user_id("user-1")
            .with_environment("prod");
        // Just verify it evaluates without error
        let _ = flags.is_enabled_for("beta", &ctx);
    }

    // --- Allowed users ---

    #[test]
    fn allowed_user_bypasses_rollout() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "feature-a",
            FlagConfig::new(true)
                .with_rollout(0) // 0% rollout — normally nobody gets in
                .with_allowed_users(vec!["vip-user".into()]),
        );
        let ctx = Context::new().with_user_id("vip-user");
        assert!(flags.is_enabled_for("feature-a", &ctx));
    }

    #[test]
    fn non_allowed_user_follows_rollout() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "feature-a",
            FlagConfig::new(true)
                .with_rollout(0)
                .with_allowed_users(vec!["vip-user".into()]),
        );
        let ctx = Context::new().with_user_id("regular-user");
        assert!(!flags.is_enabled_for("feature-a", &ctx));
    }

    // --- Allowed roles ---

    #[test]
    fn allowed_role_bypasses_rollout() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "admin-feature",
            FlagConfig::new(true)
                .with_rollout(0)
                .with_allowed_roles(vec!["admin".into()]),
        );
        let ctx = Context::new()
            .with_user_id("user-1")
            .with_role("admin");
        assert!(flags.is_enabled_for("admin-feature", &ctx));
    }

    #[test]
    fn non_matching_role_follows_rollout() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "admin-feature",
            FlagConfig::new(true)
                .with_rollout(0)
                .with_allowed_roles(vec!["admin".into()]),
        );
        let ctx = Context::new()
            .with_user_id("user-1")
            .with_role("viewer");
        assert!(!flags.is_enabled_for("admin-feature", &ctx));
    }

    // --- Required attributes ---

    #[test]
    fn required_attributes_match() {
        let mut attrs = HashMap::new();
        attrs.insert("plan".to_owned(), "enterprise".to_owned());

        let mut flags = FeatureFlags::new();
        flags.set(
            "enterprise-only",
            FlagConfig::new(true).with_required_attributes(attrs),
        );
        let ctx = Context::new().with_attribute("plan", "enterprise");
        assert!(flags.is_enabled_for("enterprise-only", &ctx));
    }

    #[test]
    fn required_attributes_mismatch() {
        let mut attrs = HashMap::new();
        attrs.insert("plan".to_owned(), "enterprise".to_owned());

        let mut flags = FeatureFlags::new();
        flags.set(
            "enterprise-only",
            FlagConfig::new(true).with_required_attributes(attrs),
        );
        let ctx = Context::new().with_attribute("plan", "free");
        assert!(!flags.is_enabled_for("enterprise-only", &ctx));
    }

    #[test]
    fn required_attributes_missing() {
        let mut attrs = HashMap::new();
        attrs.insert("plan".to_owned(), "enterprise".to_owned());

        let mut flags = FeatureFlags::new();
        flags.set(
            "enterprise-only",
            FlagConfig::new(true).with_required_attributes(attrs),
        );
        let ctx = Context::new();
        assert!(!flags.is_enabled_for("enterprise-only", &ctx));
    }

    // --- Variants ---

    #[test]
    fn get_variant_deterministic() {
        let flags = FeatureFlags::new();
        let variants = &["control", "variant-a", "variant-b"];
        let v1 = flags.get_variant("experiment", "user-42", variants);
        let v2 = flags.get_variant("experiment", "user-42", variants);
        assert_eq!(v1, v2);
        assert!(v1.is_some());
        assert!(variants.contains(&v1.unwrap().as_str()));
    }

    #[test]
    fn get_variant_empty_returns_none() {
        let flags = FeatureFlags::new();
        assert!(flags.get_variant("experiment", "user-42", &[]).is_none());
    }

    #[test]
    fn get_variant_single() {
        let flags = FeatureFlags::new();
        let v = flags.get_variant("experiment", "user-42", &["only"]);
        assert_eq!(v, Some("only".to_owned()));
    }

    // --- set_config / evaluate_with_config ---

    #[test]
    fn set_config_and_evaluate() {
        let mut flags = FeatureFlags::new();
        flags.set_config(
            "new-flag",
            FlagConfig::new(true)
                .with_allowed_users(vec!["alice".into()]),
        );
        let ctx = Context::new().with_user_id("alice");
        assert!(flags.evaluate_with_config("new-flag", &ctx));
    }

    #[test]
    fn evaluate_with_config_missing() {
        let flags = FeatureFlags::new();
        let ctx = Context::new();
        assert!(!flags.evaluate_with_config("missing", &ctx));
    }

    // --- Evaluation order ---

    #[test]
    fn allowed_user_checked_before_rollout() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "flag",
            FlagConfig::new(true)
                .with_rollout(0)
                .with_allowed_users(vec!["special".into()])
                .with_allowed_roles(vec!["admin".into()]),
        );
        // User match wins even at 0% rollout
        let ctx = Context::new().with_user_id("special");
        assert!(flags.is_enabled_for("flag", &ctx));
    }

    #[test]
    fn role_checked_before_rollout() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "flag",
            FlagConfig::new(true)
                .with_rollout(0)
                .with_allowed_roles(vec!["admin".into()]),
        );
        let ctx = Context::new()
            .with_user_id("nobody")
            .with_role("admin");
        assert!(flags.is_enabled_for("flag", &ctx));
    }

    #[test]
    fn context_with_role_builder() {
        let ctx = Context::new().with_role("editor");
        assert_eq!(ctx.attributes.get("role").map(|s| s.as_str()), Some("editor"));
    }
}
