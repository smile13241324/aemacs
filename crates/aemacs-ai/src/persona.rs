use serde::{Deserialize, Serialize};

use crate::error::AIError;

/// A Persona defines the static identity of an AI agent within the Æmacs Unified Agentic System.
/// Personas are defined in YAML files and loaded into the Agent Registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Persona {
    /// The name of the agent, typically derived from the filename.
    pub name: String,

    /// A short description of the agent's role or purpose for the UI.
    pub description: String,

    /// The core system prompt that defines the agent's behavior and identity.
    pub system_prompt: String,

    /// An optional path to a technical profile (rulebook) for this agent.
    pub profile_path: Option<String>,

    /// An optional firmware override that takes precedence over system firmware directives.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub firmware_override: Option<String>,
}

impl Persona {
    /// Create a new Persona manually.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        system_prompt: impl Into<String>,
        profile_path: Option<String>,
        firmware_override: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            system_prompt: system_prompt.into(),
            profile_path,
            firmware_override,
        }
    }

    /// Decodes a persona from a YAML-formatted string.
    ///
    /// # Errors
    /// Returns an error if the YAML document cannot be parsed into a valid `Persona`.
    pub fn from_yaml(yaml: &str) -> Result<Self, AIError> {
        serde_yaml::from_str(yaml)
            .map_err(|e| AIError::Persona(format!("Failed to parse Persona YAML: {e}")))
    }

    /// Encodes the persona into a YAML-formatted string.
    ///
    /// # Errors
    /// Returns an error if the persona cannot be serialized into YAML.
    pub fn to_yaml(&self) -> Result<String, AIError> {
        serde_yaml::to_string(self)
            .map_err(|e| AIError::Persona(format!("Failed to serialize Persona to YAML: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persona_roundtrip() -> Result<(), AIError> {
        let original = Persona::new(
            "kairon",
            "The Forge Master",
            "You are an elemental force of creation.",
            Some("ai/profiles/rust.md".to_string()),
            Some("Custom rules".to_string()),
        );
        let yaml = original.to_yaml()?;
        let deserialized = Persona::from_yaml(&yaml)?;
        assert_eq!(original, deserialized);
        Ok(())
    }

    #[test]
    fn test_invalid_yaml() {
        let bad_yaml = "not really yaml: {";
        let result = Persona::from_yaml(bad_yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_legacy_yaml_parsing() -> Result<(), AIError> {
        let legacy_yaml = r#"
name: "old_agent"
description: "An agent without firmware override"
system_prompt: "You are old."
"#;
        let deserialized = Persona::from_yaml(legacy_yaml)?;
        assert_eq!(deserialized.name, "old_agent");
        assert_eq!(deserialized.firmware_override, None);
        Ok(())
    }
}
