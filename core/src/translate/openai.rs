use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent, CreateChatCompletionRequestArgs,
    },
    Client,
};
use std::env;

use super::prompt::PROMPT;

/// Builds the system and user messages sent to the translation model.
fn messages_for(input: &str) -> Vec<ChatCompletionRequestMessage> {
    vec![
        ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
            content: ChatCompletionRequestSystemMessageContent::Text(PROMPT.to_owned()),
            name: None,
        }),
        ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
            content: ChatCompletionRequestUserMessageContent::Text(input.to_owned()),
            name: None,
        }),
    ]
}

/// Translates `input` with the OpenAI chat completion API.
pub async fn translate_with_openai(input: &str) -> Result<String, String> {
    // A missing .env file is fine when the variables are provided by the process.
    let _ = dotenvy::dotenv();

    let api_key = env::var("OPENAI_API_KEY")
        .map_err(|_| "OPENAI_API_KEY environment variable is not set".to_string())?;
    let mut config = OpenAIConfig::new().with_api_key(api_key);
    if let Ok(api_base) = env::var("OPENAI_API_BASE") {
        config = config.with_api_base(api_base);
    }
    let client = Client::with_config(config);

    let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages_for(input))
        .build()
        .map_err(|err| err.to_string())?;

    let response = client
        .chat()
        .create(request)
        .await
        .map_err(|err| err.to_string())?;

    response
        .choices
        .into_iter()
        .next()
        .and_then(|choice| choice.message.content)
        .ok_or_else(|| "OpenAI returned an empty translation".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        messages_for, ChatCompletionRequestMessage, ChatCompletionRequestUserMessageContent,
    };

    #[test]
    fn request_keeps_translation_input_separate_from_instructions() {
        let messages = messages_for("sample Wikipedia text");

        assert_eq!(messages.len(), 2);
        assert!(matches!(
            messages[0],
            ChatCompletionRequestMessage::System(_)
        ));
        assert!(matches!(
            &messages[1],
            ChatCompletionRequestMessage::User(message)
                if message.content == ChatCompletionRequestUserMessageContent::Text("sample Wikipedia text".to_string())
        ));
    }
}
