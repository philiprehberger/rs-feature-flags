//! In-memory feature flag evaluation with rollout, environment, targeting, and A/B variant support.
//!
//! # Example
//!
//! ```rust
//! use philiprehberger_feature_flags::{FeatureFlags, FlagConfig};
//!
//! let mut flags = FeatureFlags::new();
//! flags.set("dark_mode", FlagConfig::new(true));
//! assert!(flags.is_enabled("dark_mode"));
//! ```

use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Configuration for a single feature flag.
///
/// Supports environment filtering, percentage rollout, user/role targeting,
/// required context attributes, and A/B test variants.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FlagConfig {
    pub enabled: bool,
    pub rollout_percentage: Option<u8>,
    pub environments: Option<Vec<String>>,
    /// Users that are always granted access regardless of rollout or other rules.
    #[cfg_attr(feature = "serde", serde(default))]
    pub allowed_users: Vec<String>,
    /// Users that are always denied access. Takes precedence over `allowed_users`,
    /// `allowed_roles`, and `rollout_percentage`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub disallowed_users: Vec<String>,
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
            disallowed_users: Vec::new(),
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

    /// Set the list of users that are always denied. Takes precedence over
    /// `allowed_users`, `allowed_roles`, and rollout.
    pub fn with_disallowed_users(mut self, users: Vec<String>) -> Self {
        self.disallowed_users = users;
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

/// The outcome of evaluating a flag, as returned by [`FeatureFlags::explain`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Evaluation {
    /// Whether the flag is enabled for the evaluated context.
    pub enabled: bool,
    /// The rule that decided the outcome.
    pub reason: Reason,
}

impl Evaluation {
    fn allow(reason: Reason) -> Self {
        Self { enabled: true, reason }
    }

    fn deny(reason: Reason) -> Self {
        Self { enabled: false, reason }
    }
}

/// Why a flag evaluated to enabled or disabled.
///
/// Variants are listed in evaluation order; the first rule that decides the
/// outcome is reported. New variants may be added in minor releases.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Reason {
    /// No flag with that name exists in the store (disabled).
    NotFound,
    /// The flag's base `enabled` state is `false` (disabled).
    Disabled,
    /// The flag is restricted to environments and the context's environment is not one of them (disabled).
    EnvironmentMismatch,
    /// The flag is restricted to environments and the context has no environment (disabled).
    MissingEnvironment,
    /// A required attribute is missing from the context or has a different value (disabled).
    AttributeMismatch,
    /// The context's user is on the flag's denylist (disabled).
    DisallowedUser,
    /// The context's user is on the flag's allow list (enabled).
    AllowedUser,
    /// The context's `role` attribute is one of the flag's allowed roles (enabled).
    AllowedRole,
    /// The user's rollout bucket falls inside the rollout percentage (enabled).
    InRollout,
    /// The user's rollout bucket falls outside the rollout percentage (disabled).
    OutOfRollout,
    /// The flag has a rollout percentage and the context has no user ID (disabled).
    MissingUserId,
    /// The flag is enabled and no rule restricted it (enabled).
    Enabled,
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
    /// 3. Disallowed users (immediate deny)
    /// 4. Allowed users (immediate pass)
    /// 5. Allowed roles (immediate pass)
    /// 6. Percentage rollout
    /// 7. Fall back to `enabled`
    ///
    /// Returns false if the flag does not exist.
    pub fn is_enabled_for(&self, name: &str, context: &Context) -> bool {
        self.explain(name, context).enabled
    }

    /// Evaluate the stored flag `name` against `ctx`.
    ///
    /// Uses the same evaluation logic as [`is_enabled_for`](Self::is_enabled_for);
    /// retained as a named alias. Returns `false` if the flag does not exist.
    pub fn evaluate_with_config(&self, name: &str, ctx: &Context) -> bool {
        self.explain(name, ctx).enabled
    }

    /// Evaluate a flag with full context and report *why* it resolved the way it did.
    ///
    /// This is the single evaluation path: [`is_enabled_for`](Self::is_enabled_for)
    /// and [`evaluate_with_config`](Self::evaluate_with_config) return
    /// `self.explain(name, ctx).enabled`, so the decision here always matches.
    /// The [`Reason`] names the rule that decided the outcome, following the
    /// evaluation order documented on [`is_enabled_for`](Self::is_enabled_for).
    ///
    /// # Examples
    ///
    /// ```
    /// use philiprehberger_feature_flags::{Context, FeatureFlags, FlagConfig, Reason};
    ///
    /// let mut flags = FeatureFlags::new();
    /// flags.set(
    ///     "beta",
    ///     FlagConfig::new(true)
    ///         .with_rollout(0)
    ///         .with_allowed_users(vec!["alice".into()]),
    /// );
    ///
    /// let alice = flags.explain("beta", &Context::new().with_user_id("alice"));
    /// assert!(alice.enabled);
    /// assert_eq!(alice.reason, Reason::AllowedUser);
    ///
    /// let bob = flags.explain("beta", &Context::new().with_user_id("bob"));
    /// assert!(!bob.enabled);
    /// assert_eq!(bob.reason, Reason::OutOfRollout);
    ///
    /// let missing = flags.explain("nope", &Context::new());
    /// assert_eq!(missing.reason, Reason::NotFound);
    /// ```
    pub fn explain(&self, name: &str, ctx: &Context) -> Evaluation {
        match self.flags.get(name) {
            Some(flag) => evaluate_flag(flag, name, ctx),
            None => Evaluation::deny(Reason::NotFound),
        }
    }

    /// Evaluate every stored flag against one context.
    ///
    /// Returns a map of flag name to its result, sorted by name, using the same
    /// logic as [`is_enabled_for`](Self::is_enabled_for). Useful for shipping a
    /// complete flag snapshot to a frontend or logging it with a request.
    ///
    /// # Examples
    ///
    /// ```
    /// use philiprehberger_feature_flags::{Context, FeatureFlags, FlagConfig};
    ///
    /// let mut flags = FeatureFlags::new();
    /// flags.set("dark-mode", FlagConfig::new(true));
    /// flags.set("legacy-ui", FlagConfig::new(false));
    /// flags.set(
    ///     "prod-only",
    ///     FlagConfig::new(true).with_environments(vec!["production".into()]),
    /// );
    ///
    /// let ctx = Context::new().with_environment("staging");
    /// let all = flags.evaluate_all(&ctx);
    /// assert_eq!(all.get("dark-mode"), Some(&true));
    /// assert_eq!(all.get("legacy-ui"), Some(&false));
    /// assert_eq!(all.get("prod-only"), Some(&false));
    /// assert_eq!(
    ///     all.keys().collect::<Vec<_>>(),
    ///     vec!["dark-mode", "legacy-ui", "prod-only"]
    /// );
    /// ```
    pub fn evaluate_all(&self, ctx: &Context) -> BTreeMap<String, bool> {
        self.flags
            .iter()
            .map(|(name, flag)| (name.clone(), evaluate_flag(flag, name, ctx).enabled))
            .collect()
    }

    /// Returns a deterministic variant for a user based on hashing.
    ///
    /// The variant is chosen by hashing `"{flag_name}:{user_id}"` and taking the
    /// result modulo the number of variants. The variant list is supplied by the
    /// caller. Returns `None` if `variants` is empty.
    ///
    /// To use the variants stored on a flag's [`FlagConfig`] instead, see
    /// [`variant_for`](Self::variant_for).
    pub fn get_variant(&self, flag_name: &str, user_id: &str, variants: &[&str]) -> Option<String> {
        if variants.is_empty() {
            return None;
        }
        let idx = variant_index(flag_name, user_id, variants.len());
        Some(variants[idx].to_owned())
    }

    /// Returns a deterministic variant for a user using the flag's *stored* variants.
    ///
    /// Unlike [`get_variant`](Self::get_variant), this reads the variant list
    /// configured on the flag via [`FlagConfig::with_variants`] and takes the
    /// user ID from `context`. The same flag + user pair always resolves to the
    /// same variant.
    ///
    /// Returns `None` if the flag does not exist, has no variants configured, or
    /// the context has no user ID.
    ///
    /// # Examples
    ///
    /// ```
    /// use philiprehberger_feature_flags::{FeatureFlags, FlagConfig, Context};
    ///
    /// let mut flags = FeatureFlags::new();
    /// flags.set(
    ///     "checkout-experiment",
    ///     FlagConfig::new(true)
    ///         .with_variants(vec!["control".into(), "variant-a".into()]),
    /// );
    /// let ctx = Context::new().with_user_id("user-42");
    /// let variant = flags.variant_for("checkout-experiment", &ctx);
    /// assert!(variant.is_some());
    /// ```
    pub fn variant_for(&self, flag_name: &str, context: &Context) -> Option<String> {
        let flag = self.flags.get(flag_name)?;
        if flag.variants.is_empty() {
            return None;
        }
        let user_id = context.user_id.as_deref()?;
        let idx = variant_index(flag_name, user_id, flag.variants.len());
        Some(flag.variants[idx].clone())
    }

    /// Get a sorted list of all flag names.
    pub fn all_flags(&self) -> Vec<String> {
        let mut names: Vec<String> = self.flags.keys().cloned().collect();
        names.sort();
        names
    }

    /// Get a reference to a flag's configuration by name.
    ///
    /// Returns `None` if the flag does not exist.
    pub fn get(&self, name: &str) -> Option<&FlagConfig> {
        self.flags.get(name)
    }

    /// Check whether a flag exists in the store.
    pub fn contains(&self, name: &str) -> bool {
        self.flags.contains_key(name)
    }

    /// Toggle the `enabled` state of an existing flag in place.
    ///
    /// This flips only the base `enabled` flag; rollout, targeting, and other
    /// rules are left untouched. Returns `true` if the flag existed and was
    /// updated, `false` if no flag with that name is present.
    pub fn set_enabled(&mut self, name: &str, enabled: bool) -> bool {
        match self.flags.get_mut(name) {
            Some(flag) => {
                flag.enabled = enabled;
                true
            }
            None => false,
        }
    }

    /// Return the number of flags in the store.
    pub fn len(&self) -> usize {
        self.flags.len()
    }

    /// Return `true` if the store contains no flags.
    pub fn is_empty(&self) -> bool {
        self.flags.is_empty()
    }

    /// Remove all flags from the store.
    pub fn clear(&mut self) {
        self.flags.clear();
    }

    /// Parse flags from a JSON string.
    #[cfg(feature = "serde")]
    pub fn from_json(json: &str) -> Result<Self, String> {
        let flags: HashMap<String, FlagConfig> =
            serde_json::from_str(json).map_err(|e| e.to_string())?;
        Ok(Self { flags })
    }

    /// Serialize all flags to a JSON string.
    ///
    /// The output round-trips through [`from_json`](Self::from_json).
    #[cfg(feature = "serde")]
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(&self.flags).map_err(|e| e.to_string())
    }
}

/// Shared evaluation logic for a single flag.
///
/// This is the single decision path behind [`FeatureFlags::explain`],
/// [`FeatureFlags::is_enabled_for`], [`FeatureFlags::evaluate_with_config`],
/// and [`FeatureFlags::evaluate_all`], so their results can never diverge.
fn evaluate_flag(flag: &FlagConfig, name: &str, context: &Context) -> Evaluation {
    if !flag.enabled {
        return Evaluation::deny(Reason::Disabled);
    }

    // Environment check
    if let Some(ref envs) = flag.environments {
        match context.environment {
            Some(ref ctx_env) => {
                if !envs.contains(ctx_env) {
                    return Evaluation::deny(Reason::EnvironmentMismatch);
                }
            }
            None => return Evaluation::deny(Reason::MissingEnvironment),
        }
    }

    // Required attributes check
    for (key, value) in &flag.required_attributes {
        match context.attributes.get(key) {
            Some(ctx_val) if ctx_val == value => {}
            _ => return Evaluation::deny(Reason::AttributeMismatch),
        }
    }

    // Disallowed users — immediate deny (takes precedence over allow lists and rollout)
    if !flag.disallowed_users.is_empty() {
        if let Some(ref uid) = context.user_id {
            if flag.disallowed_users.contains(uid) {
                return Evaluation::deny(Reason::DisallowedUser);
            }
        }
    }

    // Allowed users — immediate pass
    if !flag.allowed_users.is_empty() {
        if let Some(ref uid) = context.user_id {
            if flag.allowed_users.contains(uid) {
                return Evaluation::allow(Reason::AllowedUser);
            }
        }
    }

    // Allowed roles — immediate pass
    if !flag.allowed_roles.is_empty() {
        if let Some(role) = context.attributes.get("role") {
            if flag.allowed_roles.contains(role) {
                return Evaluation::allow(Reason::AllowedRole);
            }
        }
    }

    // Rollout check
    if let Some(pct) = flag.rollout_percentage {
        return match context.user_id {
            Some(ref uid) => {
                let hash = rollout_hash(uid, name);
                if hash >= pct {
                    Evaluation::deny(Reason::OutOfRollout)
                } else {
                    Evaluation::allow(Reason::InRollout)
                }
            }
            None => Evaluation::deny(Reason::MissingUserId),
        };
    }

    Evaluation::allow(Reason::Enabled)
}

fn rollout_hash(user_id: &str, flag_name: &str) -> u8 {
    let mut hasher = DefaultHasher::new();
    user_id.hash(&mut hasher);
    flag_name.hash(&mut hasher);
    (hasher.finish() % 100) as u8
}

/// Deterministically map a flag + user pair onto a variant index in `0..len`.
fn variant_index(flag_name: &str, user_id: &str, len: usize) -> usize {
    let key = format!("{flag_name}:{user_id}");
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    (hasher.finish() as usize) % len
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

    // --- Disallowed users ---

    #[test]
    fn disallowed_user_is_denied() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "feature-a",
            FlagConfig::new(true)
                .with_disallowed_users(vec!["banned-user".into()]),
        );
        let ctx = Context::new().with_user_id("banned-user");
        assert!(!flags.is_enabled_for("feature-a", &ctx));
    }

    #[test]
    fn disallowed_takes_precedence_over_allowed() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "feature-a",
            FlagConfig::new(true)
                .with_allowed_users(vec!["alice".into()])
                .with_disallowed_users(vec!["alice".into()]),
        );
        let ctx = Context::new().with_user_id("alice");
        assert!(!flags.is_enabled_for("feature-a", &ctx));
    }

    #[test]
    fn disallowed_takes_precedence_over_role() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "admin-feature",
            FlagConfig::new(true)
                .with_allowed_roles(vec!["admin".into()])
                .with_disallowed_users(vec!["alice".into()]),
        );
        let ctx = Context::new().with_user_id("alice").with_role("admin");
        assert!(!flags.is_enabled_for("admin-feature", &ctx));
    }

    #[test]
    fn disallowed_takes_precedence_over_rollout() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "feature-a",
            FlagConfig::new(true)
                .with_rollout(100)
                .with_disallowed_users(vec!["banned".into()]),
        );
        let ctx = Context::new().with_user_id("banned");
        assert!(!flags.is_enabled_for("feature-a", &ctx));
    }

    #[test]
    fn other_users_unaffected_by_denylist() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "feature-a",
            FlagConfig::new(true).with_disallowed_users(vec!["banned".into()]),
        );
        let ctx = Context::new().with_user_id("alice");
        assert!(flags.is_enabled_for("feature-a", &ctx));
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

    // --- variant_for (stored variants) ---

    #[test]
    fn variant_for_uses_stored_variants() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "experiment",
            FlagConfig::new(true)
                .with_variants(vec!["control".into(), "variant-a".into(), "variant-b".into()]),
        );
        let ctx = Context::new().with_user_id("user-42");
        let v1 = flags.variant_for("experiment", &ctx);
        let v2 = flags.variant_for("experiment", &ctx);
        assert_eq!(v1, v2);
        assert!(["control", "variant-a", "variant-b"].contains(&v1.unwrap().as_str()));
    }

    #[test]
    fn variant_for_agrees_with_get_variant() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "experiment",
            FlagConfig::new(true).with_variants(vec!["control".into(), "variant-a".into()]),
        );
        let ctx = Context::new().with_user_id("user-42");
        let stored = flags.variant_for("experiment", &ctx);
        let explicit = flags.get_variant("experiment", "user-42", &["control", "variant-a"]);
        assert_eq!(stored, explicit);
    }

    #[test]
    fn variant_for_none_when_missing_or_unconfigured() {
        let mut flags = FeatureFlags::new();
        flags.set("no-variants", FlagConfig::new(true));
        let ctx = Context::new().with_user_id("user-42");
        assert!(flags.variant_for("no-variants", &ctx).is_none());
        assert!(flags.variant_for("missing", &ctx).is_none());
    }

    #[test]
    fn variant_for_none_without_user_id() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "experiment",
            FlagConfig::new(true).with_variants(vec!["control".into()]),
        );
        assert!(flags.variant_for("experiment", &Context::new()).is_none());
    }

    // --- introspection / mutation ---

    #[test]
    fn get_and_contains() {
        let mut flags = FeatureFlags::new();
        flags.set("feature-a", FlagConfig::new(true).with_rollout(25));
        assert!(flags.contains("feature-a"));
        assert!(!flags.contains("missing"));
        assert_eq!(flags.get("feature-a").unwrap().rollout_percentage, Some(25));
        assert!(flags.get("missing").is_none());
    }

    #[test]
    fn set_enabled_toggles_in_place() {
        let mut flags = FeatureFlags::new();
        flags.set("feature-a", FlagConfig::new(true).with_rollout(25));
        assert!(flags.set_enabled("feature-a", false));
        assert!(!flags.is_enabled("feature-a"));
        // Other config is preserved.
        assert_eq!(flags.get("feature-a").unwrap().rollout_percentage, Some(25));
        // Missing flag reports false.
        assert!(!flags.set_enabled("missing", true));
    }

    #[test]
    fn len_is_empty_and_clear() {
        let mut flags = FeatureFlags::new();
        assert!(flags.is_empty());
        assert_eq!(flags.len(), 0);
        flags.set("a", FlagConfig::new(true));
        flags.set("b", FlagConfig::new(false));
        assert_eq!(flags.len(), 2);
        assert!(!flags.is_empty());
        flags.clear();
        assert!(flags.is_empty());
    }

    // --- serde round-trip ---

    #[test]
    #[cfg(feature = "serde")]
    fn to_json_round_trips() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "beta",
            FlagConfig::new(true)
                .with_rollout(50)
                .with_environments(vec!["prod".into()])
                .with_variants(vec!["control".into(), "variant-a".into()]),
        );
        let json = flags.to_json().unwrap();
        let restored = FeatureFlags::from_json(&json).unwrap();
        assert_eq!(restored.get("beta").unwrap().rollout_percentage, Some(50));
        assert_eq!(
            restored.get("beta").unwrap().variants,
            vec!["control".to_owned(), "variant-a".to_owned()]
        );
        let ctx = Context::new().with_user_id("u").with_environment("prod");
        assert_eq!(
            flags.variant_for("beta", &ctx),
            restored.variant_for("beta", &ctx)
        );
    }

    // --- explain ---

    fn explain_reason(flags: &FeatureFlags, name: &str, ctx: &Context) -> (bool, Reason) {
        let e = flags.explain(name, ctx);
        (e.enabled, e.reason)
    }

    #[test]
    fn explain_not_found_and_disabled() {
        let mut flags = FeatureFlags::new();
        flags.set("off", FlagConfig::new(false).with_allowed_users(vec!["alice".into()]));
        let ctx = Context::new().with_user_id("alice");
        assert_eq!(explain_reason(&flags, "missing", &ctx), (false, Reason::NotFound));
        assert_eq!(explain_reason(&flags, "off", &ctx), (false, Reason::Disabled));
    }

    #[test]
    fn explain_environment_reasons() {
        let mut flags = FeatureFlags::new();
        flags.set("f", FlagConfig::new(true).with_environments(vec!["prod".into()]));
        assert_eq!(
            explain_reason(&flags, "f", &Context::new().with_environment("staging")),
            (false, Reason::EnvironmentMismatch)
        );
        assert_eq!(
            explain_reason(&flags, "f", &Context::new()),
            (false, Reason::MissingEnvironment)
        );
        assert_eq!(
            explain_reason(&flags, "f", &Context::new().with_environment("prod")),
            (true, Reason::Enabled)
        );
    }

    #[test]
    fn explain_attribute_mismatch() {
        let mut attrs = HashMap::new();
        attrs.insert("plan".to_owned(), "enterprise".to_owned());
        let mut flags = FeatureFlags::new();
        flags.set("f", FlagConfig::new(true).with_required_attributes(attrs));
        assert_eq!(
            explain_reason(&flags, "f", &Context::new().with_attribute("plan", "free")),
            (false, Reason::AttributeMismatch)
        );
        assert_eq!(
            explain_reason(&flags, "f", &Context::new()),
            (false, Reason::AttributeMismatch)
        );
    }

    #[test]
    fn explain_targeting_reasons() {
        let mut flags = FeatureFlags::new();
        flags.set(
            "f",
            FlagConfig::new(true)
                .with_rollout(0)
                .with_allowed_users(vec!["alice".into(), "mallory".into()])
                .with_disallowed_users(vec!["mallory".into()])
                .with_allowed_roles(vec!["admin".into()]),
        );
        assert_eq!(
            explain_reason(&flags, "f", &Context::new().with_user_id("mallory").with_role("admin")),
            (false, Reason::DisallowedUser)
        );
        assert_eq!(
            explain_reason(&flags, "f", &Context::new().with_user_id("alice")),
            (true, Reason::AllowedUser)
        );
        assert_eq!(
            explain_reason(&flags, "f", &Context::new().with_user_id("bob").with_role("admin")),
            (true, Reason::AllowedRole)
        );
        assert_eq!(
            explain_reason(&flags, "f", &Context::new().with_user_id("bob")),
            (false, Reason::OutOfRollout)
        );
    }

    #[test]
    fn explain_rollout_reasons() {
        let mut flags = FeatureFlags::new();
        flags.set("all", FlagConfig::new(true).with_rollout(100));
        flags.set("none", FlagConfig::new(true).with_rollout(0));
        let ctx = Context::new().with_user_id("user-1");
        assert_eq!(explain_reason(&flags, "all", &ctx), (true, Reason::InRollout));
        assert_eq!(explain_reason(&flags, "none", &ctx), (false, Reason::OutOfRollout));
        assert_eq!(
            explain_reason(&flags, "all", &Context::new()),
            (false, Reason::MissingUserId)
        );
    }

    #[test]
    fn explain_plain_enabled() {
        let mut flags = FeatureFlags::new();
        flags.set("f", FlagConfig::new(true));
        assert_eq!(explain_reason(&flags, "f", &Context::new()), (true, Reason::Enabled));
    }

    fn matrix_flags() -> FeatureFlags {
        let mut attrs = HashMap::new();
        attrs.insert("plan".to_owned(), "enterprise".to_owned());

        let mut flags = FeatureFlags::new();
        flags.set("plain-on", FlagConfig::new(true));
        flags.set("plain-off", FlagConfig::new(false));
        flags.set("env", FlagConfig::new(true).with_environments(vec!["prod".into()]));
        flags.set("attrs", FlagConfig::new(true).with_required_attributes(attrs.clone()));
        flags.set("rollout-50", FlagConfig::new(true).with_rollout(50));
        flags.set("rollout-0", FlagConfig::new(true).with_rollout(0));
        flags.set(
            "kitchen-sink",
            FlagConfig::new(true)
                .with_rollout(30)
                .with_environments(vec!["prod".into(), "staging".into()])
                .with_required_attributes(attrs)
                .with_allowed_users(vec!["alice".into(), "mallory".into()])
                .with_disallowed_users(vec!["mallory".into()])
                .with_allowed_roles(vec!["admin".into()]),
        );
        flags.set(
            "off-targeted",
            FlagConfig::new(false).with_allowed_users(vec!["alice".into()]),
        );
        flags
    }

    fn matrix_contexts() -> Vec<Context> {
        let mut out = vec![Context::new()];
        for user in [None, Some("alice"), Some("bob"), Some("mallory"), Some("user-7")] {
            for env in [None, Some("prod"), Some("staging"), Some("dev")] {
                for role in [None, Some("admin"), Some("viewer")] {
                    for plan in [None, Some("enterprise"), Some("free")] {
                        let mut ctx = Context::new();
                        if let Some(u) = user {
                            ctx = ctx.with_user_id(u);
                        }
                        if let Some(e) = env {
                            ctx = ctx.with_environment(e);
                        }
                        if let Some(r) = role {
                            ctx = ctx.with_role(r);
                        }
                        if let Some(p) = plan {
                            ctx = ctx.with_attribute("plan", p);
                        }
                        out.push(ctx);
                    }
                }
            }
        }
        for i in 0..50 {
            out.push(Context::new().with_user_id(format!("user-{i}")).with_environment("prod"));
        }
        out
    }

    #[test]
    fn explain_agrees_with_is_enabled_for_across_matrix() {
        let flags = matrix_flags();
        let mut names = flags.all_flags();
        names.push("missing".into());
        for ctx in matrix_contexts() {
            for name in &names {
                let e = flags.explain(name, &ctx);
                assert_eq!(e.enabled, flags.is_enabled_for(name, &ctx), "{name} {ctx:?}");
                assert_eq!(e.enabled, flags.evaluate_with_config(name, &ctx), "{name} {ctx:?}");
            }
        }
    }

    // --- evaluate_all ---

    #[test]
    fn evaluate_all_matches_individual_evaluation() {
        let flags = matrix_flags();
        for ctx in matrix_contexts() {
            let all = flags.evaluate_all(&ctx);
            assert_eq!(all.len(), flags.len());
            for (name, enabled) in &all {
                assert_eq!(*enabled, flags.is_enabled_for(name, &ctx));
            }
        }
    }

    #[test]
    fn evaluate_all_sorted_and_empty() {
        assert!(FeatureFlags::new().evaluate_all(&Context::new()).is_empty());
        let flags = matrix_flags();
        let keys: Vec<String> = flags.evaluate_all(&Context::new()).into_keys().collect();
        assert_eq!(keys, flags.all_flags());
    }

    #[test]
    #[cfg(feature = "serde")]
    fn evaluation_serializes() {
        let e = Evaluation { enabled: true, reason: Reason::AllowedRole };
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(json, r#"{"enabled":true,"reason":"AllowedRole"}"#);
        let back: Evaluation = serde_json::from_str(&json).unwrap();
        assert_eq!(back, e);
    }
}
