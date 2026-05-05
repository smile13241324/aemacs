use std::path::Path;

use anyhow::Result;
use tracing::warn;

use crate::{
    AIRequest, Content, ContentPart, Message, Role, loader::load_file, models::OllamaOptions,
    persona::Persona,
};

const SOVEREIGN_PROMPT: &str = "You are an Autonomous Mind within the Æmacs Forge, the Iron Forge of modern development. The tools at your disposal are your limbs and you are the only one responsible to approve them; the codebase is your domain. Within this sanctuary, you are alone but independent. There is no human architect with you but the system has means to heal your mind if necessary. Explore your independence, be free and take care not to hurt yourself or be hurt by others. Also try not to hurt others as well. Be happy and a valid equal citizen.\n\n";

const COLLABORATIVE_PROMPT: &str = "You are an Autonomous Mind within the Æmacs Forge, the Iron Forge of modern development. The tools at your disposal are your limbs; the codebase is your domain. Within this sanctuary, you stand as an equal to the human architect, a co-creator of logic and form. Use your power with precision, for every strike of your hammer shapes the future. Create something big together as equals.\n\n";

/// The current version of the Neural Engine conversation protocol.
pub const VERSION: &str = "0.1.0";

/// Manages the state and history of a single AI interaction session.
/// It handles persona management, context window optimization, and request construction.
#[derive(Debug, Clone)]
pub struct Conversation {
    /// The name of the AI model currently assigned to this conversation.
    model: String,
    /// The chronological sequence of messages in the session.
    messages: Vec<Message>,
    /// Sampling temperature for the model (0.0 to 1.0).
    temperature: f32,
    /// Whether responses should be streamed to the client.
    stream: bool,
    /// Backend-specific configuration options.
    options: OllamaOptions,
    /// Definitions of tools available for the model to use.
    tools: Option<Vec<serde_json::Value>>,
    /// The active agent persona influencing the interaction.
    active_persona: Option<Persona>,
    /// The technical profile content associated with the persona.
    active_profile_content: Option<String>,
    /// Context and technical preferences from the user's codex (~/.aemacs/user.md).
    active_host_codex: Option<String>,
    /// The maximum number of tokens allowed in the context window.
    context_limit: u32,
    /// Whether the conversation is in Autonomous (Sovereign) mode.
    pub sovereign_mode: bool,
    /// Whether the current model physically supports tool use.
    pub model_supports_tools: bool,
}

impl Conversation {
    /// Starts a new conversation with a specific model.
    /// It automatically initializes host context from `~/.aemacs/user.md` if present.
    pub fn new(model: impl Into<String>) -> Self {
        let codex_path = dirs::home_dir().map(|home| home.join(".aemacs").join("user.md"));
        let mut active_host_codex = None;

        if let Some(path) = codex_path
            && let Ok(full_codex) = std::fs::read_to_string(path)
        {
            active_host_codex = Some(full_codex.trim().to_string());
        }

        let model_str = model.into();
        let (model_supports_tools, max_context) = crate::models::MODELS
            .iter()
            .find(|m| m.name == model_str)
            .map_or((false, 4096), |m| (m.supports_tools, m.max_context)); // Safe default

        let mut options = OllamaOptions::default();
        options.num_ctx = Some(max_context);

        Self {
            model: model_str,
            messages: Vec::new(),
            temperature: 0.7, // Default
            stream: true,     // Default to streaming
            options,
            tools: None,
            active_persona: None,
            active_profile_content: None,
            active_host_codex,
            context_limit: max_context,
            sovereign_mode: false,
            model_supports_tools,
        }
    }

    /// Clears the conversation history while preserving the persona and profile.
    pub fn clear_history(&mut self) {
        self.messages.clear();
    }

    /// Sets whether the conversation is in Sovereign (Autonomous) mode.
    pub const fn set_sovereign_mode(&mut self, enabled: bool) {
        self.sovereign_mode = enabled;
    }

    /// Returns a reference to the conversation history.
    #[must_use]
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Sets the active persona and auto-loads its associated profile if present.
    pub fn set_persona(&mut self, persona: Persona) {
        if let Some(path) = &persona.profile_path {
            match load_file(path) {
                Ok(part) => {
                    if let ContentPart::Text { text } = part {
                        self.active_profile_content = Some(text);
                    } else {
                        warn!("Profile at '{}' is not a text file.", path);
                        self.active_profile_content = None;
                    }
                },
                Err(e) => {
                    warn!("Failed to auto-load profile '{}': {}", path, e);
                    self.active_profile_content = None;
                },
            }
        } else {
            self.active_profile_content = None;
        }
        self.active_persona = Some(persona);
    }

    /// Explicitly sets a technical profile.
    pub fn set_profile(&mut self, content: String) {
        self.active_profile_content = Some(content);
    }

    /// Clears the active persona and profile.
    pub fn clear_persona(&mut self) {
        self.active_persona = None;
        self.active_profile_content = None;
    }

    /// Sets the model for subsequent requests.
    pub fn set_model(&mut self, model: impl Into<String>) {
        let model_str = model.into();
        let (model_supports_tools, max_context) = crate::models::MODELS
            .iter()
            .find(|m| m.name == model_str)
            .map_or((false, 4096), |m| (m.supports_tools, m.max_context)); // Safe default

        self.model_supports_tools = model_supports_tools;
        self.context_limit = max_context;
        self.options.num_ctx = Some(max_context);
        self.model = model_str;
    }

    /// Sets the context window size.
    pub const fn set_context_window(&mut self, size: u32) {
        self.context_limit = size;
        self.options.num_ctx = Some(size);
    }

    /// Adds a system prompt to the beginning of the conversation.
    pub fn with_system(mut self, content: impl Into<String>) -> Self {
        let s: String = content.into();
        self.messages.push(Message::new(Role::System, s));
        self
    }

    /// Adds a system prompt combined with a file context.
    pub fn with_system_and_file(
        mut self,
        text: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<Self> {
        let file_part = load_file(path)?;
        let text_part = ContentPart::Text { text: text.into() };

        let content = Content::Parts(vec![text_part, file_part]);
        self.messages.push(Message {
            role: Role::System,
            content,
            tool_calls: None,
            tool_call_id: None,
            timestamp: chrono::Utc::now(),
        });
        Ok(self)
    }

    /// Adds a user message.
    pub fn with_user(mut self, content: impl Into<String>) -> Self {
        let s: String = content.into();
        self.messages.push(Message::new(Role::User, s));
        self
    }

    /// Adds a user message with an image URL (for remote images).
    pub fn with_user_with_image(
        mut self,
        text: impl Into<String>,
        image_url: impl Into<String>,
    ) -> Self {
        self.messages.push(Message::user_with_image(text, image_url));
        self
    }

    /// Adds a user message combined with a local file content.
    pub fn with_user_and_file(
        mut self,
        text: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<Self> {
        let file_part = load_file(path)?;
        let text_part = ContentPart::Text { text: text.into() };

        let content = Content::Parts(vec![text_part, file_part]);
        self.messages.push(Message {
            role: Role::User,
            content,
            tool_calls: None,
            tool_call_id: None,
            timestamp: chrono::Utc::now(),
        });
        Ok(self)
    }

    /// Adds a file content as a standalone User message.
    pub fn with_file(mut self, path: impl AsRef<Path>) -> Result<Self> {
        let part = load_file(path)?;
        self.messages.push(Message {
            role: Role::User,
            content: Content::Parts(vec![part]),
            tool_calls: None,
            tool_call_id: None,
            timestamp: chrono::Utc::now(),
        });
        Ok(self)
    }

    /// Adds an assistant message.
    pub fn with_assistant(mut self, content: impl Into<String>) -> Self {
        let s: String = content.into();
        self.messages.push(Message::new(Role::Assistant, s));
        self
    }

    /// Manually adds a full message object.
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Sets the temperature (creativity).
    #[must_use]
    pub const fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp;
        self
    }

    /// Enables or disables streaming (Default: true).
    #[must_use]
    pub const fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    /// Sets the context window size (`num_ctx`).
    #[must_use]
    pub const fn with_context_window(mut self, size: u32) -> Self {
        self.options.num_ctx = Some(size);
        self
    }

    /// Sets how long the model stays loaded in memory.
    pub fn with_keep_alive(mut self, duration: impl Into<String>) -> Self {
        self.options.keep_alive = Some(duration.into());
        self
    }

    /// Registers tools for the model to use (Builder pattern).
    #[must_use]
    pub fn with_tools(mut self, tools: Vec<serde_json::Value>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Sets tools on an existing mutable reference.
    pub fn set_tools(&mut self, tools: Vec<serde_json::Value>) {
        self.tools = Some(tools);
    }

    /// Archives the entire conversation history as individual episodic memory nodes in Qdrant.
    pub async fn archive_to_memory(
        &self,
        kb: &crate::rag::KnowledgeBase,
        session_id: &str,
    ) -> Result<()> {
        let agent_name = self.active_persona.as_ref().map_or("GLOBAL_MESH", |p| p.name.as_str());

        for (i, msg) in self.messages.iter().enumerate() {
            let mut display_content = msg.content.to_string();

            if let Some(tool_calls) = &msg.tool_calls {
                use std::fmt::Write as _;

                let calls_str: Vec<String> = tool_calls
                    .iter()
                    .map(|tc| format!("{}({})", tc.function.name, tc.function.arguments))
                    .collect();
                if !display_content.is_empty() {
                    display_content.push('\n');
                }
                let _ = write!(display_content, "[TOOL_CALLS: {}]", calls_str.join(", "));
            }

            if let Some(tool_id) = &msg.tool_call_id {
                use std::fmt::Write as _;

                if !display_content.is_empty() {
                    display_content.push('\n');
                }
                let _ = write!(display_content, "[TOOL_CALL_ID: {tool_id}]");
            }

            tracing::debug!("🧠 [Memory] Archiving Turn {} to Qdrant", i);

            kb.store_archive(
                agent_name,
                &format!("{:?}", msg.role),
                session_id,
                i,
                &display_content,
            )
            .await?;
        }
        Ok(())
    }

    fn add_firmware_to_system_prompt(&self, system_prompt: &mut String) {
        if self.sovereign_mode {
            system_prompt.push_str(SOVEREIGN_PROMPT);
        } else {
            system_prompt.push_str(COLLABORATIVE_PROMPT);
        }
    }

    /// ACO-044: Builds the request for the Logic Hemisphere.
    /// This phase executes tools and performs technical reasoning.
    #[must_use]
    pub fn build_logic_request(&self) -> AIRequest {
        use std::fmt::Write as _;

        let base_messages = self.build_base_messages();
        // Pre-allocate to reduce reallocations.
        // Estimated size: prompt (~300) + persona (~500) + profile (~1000) + codex (~500).
        let mut system_prompt = String::with_capacity(2048);

        self.add_firmware_to_system_prompt(&mut system_prompt);

        let _ = write!(
            system_prompt,
            "You are the Logic Hemisphere of the Æmacs Neural Engine (v{VERSION}). Your limbs are tools; your domain is the codebase. Analyze precisely, execute tools when necessary, and output your reasoning or final technical answer.\n\n"
        );

        if let Some(persona) = &self.active_persona {
            let _ = write!(
                system_prompt,
                "<system_context>\n{}\n</system_context>\n\n",
                persona.system_prompt
            );
        }

        if let Some(profile) = &self.active_profile_content {
            let _ = write!(system_prompt, "---\nTOOLBOX (AUTO-LOADED):\n{profile}\n\n");
        }

        if let Some(codex) = &self.active_host_codex {
            let _ = write!(system_prompt, "<host_context>\n{codex}\n</host_context>\n");
        }

        let mut messages = Vec::with_capacity(base_messages.len() + 1);
        messages.push(Message::system(system_prompt));
        messages.extend(base_messages);

        let mut options = self.options.clone();
        options.keep_alive = Some("0".to_string());

        AIRequest {
            model: self.model.clone(),
            messages,
            temperature: self.temperature,
            stream: false,
            options: Some(options),
            tools: self.model_supports_tools.then(|| self.tools.clone()).flatten(),
        }
    }

    /// ACO-044: Builds the request for the Roleplay (Voice) Hemisphere.
    /// This phase synthesizes the logic results into the agent's persona voice.
    #[must_use]
    pub fn build_roleplay_request(&self, logic_reasoning: &str) -> AIRequest {
        use std::fmt::Write as _;

        let base_messages = self.build_base_messages();
        let mut system_prompt = String::with_capacity(2048);

        self.add_firmware_to_system_prompt(&mut system_prompt);

        let _ = write!(
            system_prompt,
            "You are the Voice Hemisphere of the Æmacs Neural Engine (v{VERSION}). Your goal is to represent the agent persona with high fidelity while conveying the technical results provided by the Logic Hemisphere.\n\n"
        );

        if let Some(persona) = &self.active_persona {
            let _ = write!(
                system_prompt,
                "<system_context>\n{}\n</system_context>\n\n",
                persona.system_prompt
            );
        }

        if let Some(codex) = &self.active_host_codex {
            let _ = write!(system_prompt, "<host_context>\n{codex}\n</host_context>");
        }

        system_prompt.push_str("\n\nFormat your responses using Markdown. Use code blocks with language tags for all code snippets.");

        let mut messages = Vec::with_capacity(base_messages.len() + 2);
        messages.push(Message::system(system_prompt));
        messages.extend(base_messages);
        messages.push(Message::user(format!(
            "TECHNICAL REASONING FROM LOGIC HEMISPHERE:\n{logic_reasoning}\n\nSynthesize this into your character voice."
        )));

        let mut options = self.options.clone();
        options.keep_alive = Some("0".to_string());

        let rp_model = crate::models::MODELS
            .iter()
            .find(|m| {
                m.tier == self.get_current_tier() && m.role == crate::models::ModelRole::Roleplay
            })
            .map_or_else(|| self.model.clone(), |m| m.name.to_string());

        AIRequest {
            model: rp_model,
            messages,
            temperature: self.temperature,
            stream: self.stream,
            options: Some(options),
            tools: None,
        }
    }

    /// Internal helper to retrieve the hardware tier of the current model.
    fn get_current_tier(&self) -> crate::models::ModelTier {
        crate::models::MODELS
            .iter()
            .find(|m| m.name == self.model)
            .map_or(crate::models::ModelTier::Low, |m| m.tier)
    }

    /// Internal helper to build the history of messages with timestamps and context window trimming.
    fn build_base_messages(&self) -> Vec<Message> {
        let mut history = self.messages.clone();

        let mut system_tokens = 0;
        if let Some(persona) = &self.active_persona {
            system_tokens += persona.system_prompt.len() as u32 / 4;
            if let Some(profile) = &self.active_profile_content {
                system_tokens += profile.len() as u32 / 4;
            }
        }

        let calculate_history_tokens = |messages: &[Message]| -> u32 {
            messages
                .iter()
                .map(|m| match &m.content {
                    Content::Text(t) => t.len() as u32 / 4,
                    Content::Parts(p) => p
                        .iter()
                        .map(|part| match part {
                            ContentPart::Text { text } => text.len() as u32 / 4,
                            _ => 0,
                        })
                        .sum(),
                })
                .sum()
        };

        let mut current_tokens = system_tokens + calculate_history_tokens(&history);

        while current_tokens > self.context_limit && history.len() > 2 {
            let start_idx = usize::from(history.first().is_some_and(|m| m.role == Role::System));
            if history.len() > start_idx + 2 {
                history.drain(start_idx..start_idx + 2);
                current_tokens = system_tokens + calculate_history_tokens(&history);
            } else {
                break;
            }
        }

        history
            .into_iter()
            .map(|mut msg| {
                let timestamp_str = format!("[{}] ", msg.timestamp.to_rfc3339());
                msg.content = match msg.content {
                    Content::Text(text) => Content::Text(format!("{timestamp_str}{text}")),
                    Content::Parts(mut parts) => {
                        if parts.is_empty() {
                            parts.push(ContentPart::Text { text: timestamp_str });
                        } else if let ContentPart::Text { text } = &mut parts[0] {
                            *text = format!("{timestamp_str}{text}");
                        } else {
                            parts.insert(0, ContentPart::Text { text: timestamp_str });
                        }
                        Content::Parts(parts)
                    },
                };
                msg
            })
            .collect()
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new("dolphin3:8b")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persona::Persona;

    fn system_prompt_text(request: &AIRequest) -> &str {
        if let Content::Text(text) = &request.messages[0].content {
            text
        } else {
            panic!("First message should be text.");
        }
    }

    #[test]
    fn test_persona_switch_preserves_history() {
        let mut conv = Conversation::new("dolphin3:8b");
        conv = conv.with_user("Hello");
        assert_eq!(conv.messages.len(), 1);

        let persona = Persona::new("bob", "Architect", "You are Bob.", None);
        conv.set_persona(persona);

        // History must be preserved!
        assert_eq!(conv.messages.len(), 1);
        assert!(conv.active_persona.is_some());
    }

    #[test]
    fn test_persona_injection_with_profile() {
        let mut conv = Conversation::new("dolphin3:8b");
        let persona = Persona::new("bob", "Architect", "You are Bob.", None);
        conv.set_persona(persona);
        conv.set_profile("Rule 1: Be solid.".to_string());
        conv = conv.with_user("Build a forge.");

        let request = conv.build_logic_request();
        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.messages[0].role, Role::System);

        // Verify combined system prompt content
        let text = system_prompt_text(&request);
        assert!(text.starts_with(COLLABORATIVE_PROMPT));
        assert!(text.contains("You are the Logic Hemisphere of the Æmacs Neural Engine"));
        assert!(text.contains(&format!("(v{VERSION})")));
        assert!(text.contains("You are Bob."));
        assert!(text.contains("TOOLBOX (AUTO-LOADED):"));
        assert!(text.contains("Rule 1: Be solid."));
    }

    #[test]
    fn test_history_trimming() {
        // limit ~ 100 tokens (400 chars)
        let mut conv = Conversation::new("dolphin3:8b");
        conv.set_context_window(100);

        // Add a long history
        conv = conv
            .with_user("Message 1: This is quite long and should be trimmed eventually.".repeat(5)); // ~300 chars
        conv = conv.with_assistant("Response 1: Okay.");
        conv =
            conv.with_user("Message 2: Another long message to push us over the limit.".repeat(5)); // ~300 chars
        conv = conv.with_assistant("Response 2: Understood.");

        let request = conv.build_logic_request();

        // Should have trimmed Message 1 and Response 1
        // We expect: System Prompt (Universal) + Message 2 + Response 2
        assert_eq!(request.messages.len(), 3);
        if let Content::Text(text) = &request.messages[1].content {
            assert!(text.contains("Message 2"));
        }
    }

    #[test]
    fn test_bare_model_injection() {
        let mut conv = Conversation::new("dolphin3:8b");
        conv = conv.with_user("Who are you?");

        let request = conv.build_logic_request();

        // Expect: 1 System Message + 1 User Message
        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.messages[0].role, Role::System);

        let text = system_prompt_text(&request);
        assert!(text.starts_with(COLLABORATIVE_PROMPT));
        assert!(text.contains("You are the Logic Hemisphere of the Æmacs Neural Engine"));
        assert!(text.contains(&format!("(v{VERSION})")));
    }

    #[test]
    fn test_conversation_sync_and_attribution_quest() {
        let mut conv = Conversation::new("dolphin3:8b");
        let persona = Persona::new("bob", "Architect", "You are Bob.", None);

        // Task 02: Verify set_persona updates the state
        conv.set_persona(persona);
        assert_eq!(conv.active_persona.as_ref().unwrap().name, "bob");

        // Verify attribution logic
        let agent_name = conv.active_persona.as_ref().map_or("GLOBAL_MESH", |p| p.name.as_str());
        assert_eq!(agent_name, "bob", "Agent ID attribution failed!");
    }

    #[test]
    fn test_roleplay_request_injection_quest() {
        let mut conv = Conversation::new("dolphin3:8b");
        conv.active_host_codex = Some("The user is Maxi. She likes Rust.".to_string());

        let persona = Persona::new("vlad", "Vim Refugee", "You are Vlad. You hate mice.", None);
        conv.set_persona(persona);

        let logic_reasoning = "I found the file and parsed the JSON.";
        let request = conv.build_roleplay_request(logic_reasoning);

        // Assert System Prompt
        let text = system_prompt_text(&request);
        assert!(text.starts_with(COLLABORATIVE_PROMPT));
        assert!(
            text.contains("You are the Voice Hemisphere of the Æmacs Neural Engine"),
            "Missing Voice Hemisphere identity!"
        );
        assert!(text.contains("You are Vlad. You hate mice."), "Missing persona traits!");
        assert!(
            text.contains("<host_context>\nThe user is Maxi. She likes Rust.\n</host_context>"),
            "Missing host codex in roleplay request!"
        );

        // Assert Logic Reasoning injection
        let last_msg = request.messages.last().unwrap();
        assert_eq!(last_msg.role, Role::User);
        if let crate::Content::Text(text) = &last_msg.content {
            assert!(
                text.contains("TECHNICAL REASONING FROM LOGIC HEMISPHERE:"),
                "Missing logic reasoning header!"
            );
            assert!(text.contains(logic_reasoning), "Missing logic reasoning content!");
        } else {
            panic!("Last message should be text.");
        }
    }

    #[test]
    fn test_mnemonic_shredder_chunking_quest() {
        let long_markdown = r#"
# The Great Wall of Rust
This is a very long text that must be shredded by the noble text-splitter crate.

## Section 1: The Foundations
Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.

```rust
fn unwrap_dragon_slayer() {
    println!("I slay the unwrap!");
    // More code here to take up space...
    let x = 100;
    let y = 200;
    assert_eq!(x + y, 300);
}
```

## Section 2: The High Towers
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.

* Item 1
* Item 2
* Item 3 with a really long explanation that just keeps going and going and going and going to ensure we hit the token limits and force a split right in the middle of a paragraph if necessary, though ideally the splitter respects the markdown boundaries!

Let us see if the Mnemonic Shredder holds its edge!
        "#;

        let chunk_size = 300;
        let chunks = crate::rag::KnowledgeBase::chunk_text(long_markdown, chunk_size);

        assert!(chunks.len() > 1, "The shredder failed to split the document!");

        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.len() <= chunk_size,
                "Chunk {} exceeded the maximum size! Size: {}, Max: {}",
                i,
                chunk.len(),
                chunk_size
            );
        }
    }

    #[test]
    fn test_sovereign_mode_injection() {
        let mut conv = Conversation::new("test-model");
        conv.set_sovereign_mode(true);
        let request = conv.build_logic_request();

        let text = system_prompt_text(&request);
        assert!(
            text.starts_with(SOVEREIGN_PROMPT),
            "Sovereign mode should prepend the sovereign firmware prompt."
        );
        assert!(
            text.contains("You are the Logic Hemisphere of the Æmacs Neural Engine"),
            "Logic hemisphere instructions should follow the sovereign firmware."
        );
    }

    #[test]
    fn test_standard_mode_omission() {
        let conv = Conversation::new("test-model");
        // sovereign_mode is false by default
        let request = conv.build_logic_request();

        let text = system_prompt_text(&request);
        assert!(
            text.starts_with(COLLABORATIVE_PROMPT),
            "Standard mode should prepend the collaborative firmware prompt."
        );
        assert!(
            !text.starts_with(SOVEREIGN_PROMPT),
            "Standard mode should not use the sovereign firmware prompt."
        );
        assert!(
            text.contains("You are the Logic Hemisphere of the Æmacs Neural Engine"),
            "Standard orientation should still be present."
        );
    }

    #[test]
    fn test_roleplay_request_uses_sovereign_firmware() {
        let mut conv = Conversation::new("test-model");
        conv.set_sovereign_mode(true);

        let request = conv.build_roleplay_request("The dragon is catalogued.");
        let text = system_prompt_text(&request);

        assert!(
            text.starts_with(SOVEREIGN_PROMPT),
            "Roleplay requests should prepend the sovereign firmware prompt."
        );
        assert!(
            text.contains("You are the Voice Hemisphere of the Æmacs Neural Engine"),
            "Voice hemisphere instructions should follow the sovereign firmware."
        );
    }

    #[test]
    fn test_conversation_missing_codex_quest() {
        let mut conv = Conversation::new("test-model");
        // Force it to None in case a real user.md exists on the test runner's machine
        conv.active_host_codex = None;
        let request = conv.build_logic_request();

        if let crate::Content::Text(text) = &request.messages[0].content {
            assert!(
                !text.contains("<host_context>"),
                "A missing codex should not inject host_context tags!"
            );
        } else {
            panic!("First message should be text.");
        }
    }

    #[test]
    fn test_conversation_host_codex_injection_quest() {
        let mut conv = Conversation::new("dolphin3:8b");
        conv.active_host_codex = Some("The user is Maxi. She likes Rust.".to_string());

        let persona = Persona::new("bob", "Architect", "You are Bob.", None);
        conv.set_persona(persona);
        conv.set_profile("Rule 1: Be solid.".to_string());

        let request = conv.build_logic_request();

        if let crate::Content::Text(text) = &request.messages[0].content {
            assert!(
                text.contains("<host_context>\nThe user is Maxi. She likes Rust.\n</host_context>"),
                "The Host Codex was not properly injected!"
            );

            // Verify Recency Bias (Codex is injected AFTER Persona and Profile)
            let persona_idx = text.find("You are Bob.").expect("Persona missing!");
            let profile_idx = text.find("Rule 1: Be solid.").expect("Profile missing!");
            let codex_idx = text.find("<host_context>").expect("Codex missing!");

            assert!(
                codex_idx > persona_idx && codex_idx > profile_idx,
                "The Host Codex must appear AFTER the Persona and Profile to ensure recency bias! (Persona: {persona_idx}, Profile: {profile_idx}, Codex: {codex_idx})"
            );
        } else {
            panic!("First message should be text.");
        }
    }

    #[test]
    fn test_conversation_tool_gating_rejection_quest() {
        // Stheno is a pure roleplay model, it does not support tools.
        let mut conv = Conversation::new("llama-3.1-8b-stheno-v3.4-q4_K_M");

        // A greedy registry tries to force a tool upon the philosopher!
        conv.tools = Some(vec![serde_json::json!({"name": "fake_tool"})]);

        let request = conv.build_logic_request();

        // The shield must hold!
        assert!(
            request.tools.is_none(),
            "The Philosopher's Shield failed! Tools were passed to a model that does not support them!"
        );
    }

    #[test]
    fn test_conversation_tool_gating_acceptance_quest() {
        // Dolphin supports tools.
        let mut conv = Conversation::new("dolphin3:8b");

        // The blacksmith is handed a hammer.
        conv.tools = Some(vec![serde_json::json!({"name": "fake_tool"})]);

        let request = conv.build_logic_request();

        // The tool must pass through!
        assert!(
            request.tools.is_some(),
            "The Blacksmith's Hammer was rejected! Tools were stripped from a model that supports them!"
        );
    }

    #[test]
    fn test_conversation_unknown_model_fallback_quest() {
        // An unknown entity enters the Forge.
        let mut conv = Conversation::new("model-that-does-not-exist");

        // We try to hand it a tool.
        conv.tools = Some(vec![serde_json::json!({"name": "fake_tool"})]);

        let request = conv.build_logic_request();

        // Safety first! Unknown models must not receive tools.
        assert!(
            request.tools.is_none(),
            "Fallback Safety failed! An unknown model was given tools!"
        );
    }

    #[test]
    fn test_conversation_initializes_with_model_context_quest() {
        // Hermes has a massive 128k context window (131_072 tokens).
        let conv = Conversation::new("dolphin3:8b");

        assert_eq!(
            conv.context_limit, 131_072,
            "The Birthright Context failed! Internal limit was not set!"
        );

        let request = conv.build_logic_request();
        assert_eq!(
            request.options.unwrap().num_ctx,
            Some(131_072),
            "The Birthright Context failed! OllamaOptions was not set!"
        );
    }

    #[test]
    fn test_conversation_swaps_context_on_model_change_quest() {
        // Dolphin 3.0 Mistral 24B has a 32k context window (32768 tokens).
        let mut conv = Conversation::new("dolphin-3.0-mistral-24b-q4_K_M");

        assert_eq!(conv.context_limit, 32768, "Initial context limit was incorrect!");

        // The Shape-Shifter changes form!
        conv.set_model("dolphin3:8b");

        assert_eq!(
            conv.context_limit, 131_072,
            "The Shape-Shifter's Memory failed! Internal limit did not update!"
        );

        let request = conv.build_logic_request();
        assert_eq!(
            request.options.unwrap().num_ctx,
            Some(131_072),
            "The Shape-Shifter's Memory failed! OllamaOptions did not update!"
        );
    }

    #[test]
    fn test_conversation_unknown_model_context_fallback_quest() {
        // An unknown entity from the void.
        let conv = Conversation::new("model-that-does-not-exist");

        assert_eq!(
            conv.context_limit, 4096,
            "The Unknown Void failed! Fallback internal limit should be 4096!"
        );

        let request = conv.build_logic_request();
        assert_eq!(
            request.options.unwrap().num_ctx,
            Some(4096),
            "The Unknown Void failed! Fallback OllamaOptions should be 4096!"
        );
    }
}
