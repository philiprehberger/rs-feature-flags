use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

#[cfg(feature = "serde")]
use serde::Deserialize;

/// Configuration for a single feature flag.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct FlagConfig {
    pub enabled: bool,
    pub rollout_percentage: Option<u8>,
    pub environments: Option<Vec<String>>,
}

impl FlagConfig {
    /// Create a new flag config with the given enabled state.
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            rollout_percentage: None,
            environments: None,
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

    /// Remove a flag. Returns true if the flag existed.
    pub fn remove(&mut self, name: &str) -> bool {
        self.flags.remove(name).is_some()
    }

    /// Check if a flag is enabled (no context evaluation).
    /// Returns false if the flag does not exist.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.flags.get(name).is_some_and(|f| f.enabled)
    }

    /// Evaluate a flag with full context (environment filtering and rollout).
    /// Returns false if the flag does not exist.
    pub fn is_enabled_for(&self, name: &str, context: &Context) -> bool {
        let flag = match self.flags.get(name) {
            Some(f) => f,
            None => return false,
        };

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
}
