use crate::error::AIError;
use serde::{Deserialize, Serialize};

/// A Persona defines the static identity of an AI agent within the Æmacs Unified Agentic System.
/// Personas are defined in YAML files and loaded into the Agent Registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Persona {
    /// The name of the agent, typically derived from the filename.
    pub name: String,

    /// A short description of the agent's role or purpose for the UI.
    pub description: String,

    /// Deprecated: The core system prompt that defines the agent's behavior and identity.
    /// Use `rules` and `personality` instead for the Bicameral Mind architecture.
    pub system_prompt: String,

    /// ACO-045: Explicit operational rules for the Logic Hemisphere.
    #[serde(default)]
    pub rules: Option<String>,

    /// ACO-045: Character traits and voice for the Roleplay Hemisphere.
    #[serde(default)]
    pub personality: Option<String>,

    /// An optional path to a technical profile (rulebook) for this agent.
    pub profile_path: Option<String>,
}

impl Persona {
    /// Create a new Persona manually using the legacy single-prompt format.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        system_prompt: impl Into<String>,
        profile_path: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            system_prompt: system_prompt.into(),
            rules: None,
            personality: None,
            profile_path,
        }
    }

    /// ACO-045: Creates a new Bicameral Persona with explicit rules for logic and personality for voice.
    /// This is the preferred method for the dual-hemisphere architecture.
    pub fn new_bicameral(
        name: impl Into<String>,
        description: impl Into<String>,
        rules: impl Into<String>,
        personality: impl Into<String>,
        profile_path: Option<String>,
    ) -> Self {
        let r = rules.into();
        let p = personality.into();
        Self {
            name: name.into(),
            description: description.into(),
            system_prompt: format!("{}\n\n{}", r, p), // Backward compatibility fallback
            rules: Some(r),
            personality: Some(p),
            profile_path,
        }
    }

    /// Decodes a persona from a YAML-formatted string.
    pub fn from_yaml(yaml: &str) -> Result<Self, AIError> {
        serde_yaml::from_str(yaml)
            .map_err(|e| AIError::Persona(format!("Failed to parse Persona YAML: {}", e)))
    }

    /// Encodes the persona into a YAML-formatted string.
    pub fn to_yaml(&self) -> Result<String, AIError> {
        serde_yaml::to_string(self)
            .map_err(|e| AIError::Persona(format!("Failed to serialize Persona to YAML: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persona_roundtrip() {
        let original = Persona::new(
            "kairon",
            "The Forge Master",
            "You are an elemental force of creation.",
            Some("ai/profiles/rust.md".to_string()),
        );
        let yaml = original.to_yaml().expect("Serialization failed");
        let deserialized = Persona::from_yaml(&yaml).expect("Deserialization failed");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_invalid_yaml() {
        let bad_yaml = "not really yaml: {";
        let result = Persona::from_yaml(bad_yaml);
        assert!(result.is_err());
    }
}
