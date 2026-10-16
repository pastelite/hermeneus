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
use wasm_bindgen::prelude::wasm_bindgen;

const PROMPT: &str = "
Translate the following formatted wikipedia text into Thai, according to this information:
- templates are stubbed as `` `[template:Infobox]` ``; keep each stub, including its backticks, exactly where it belongs.
- references are stubbed as `[^source]`; keep each reference stub exactly where it belongs.
- the system will transform these template and reference stubs into Wikipedia content, so do not alter, move, or remove them.
- links use Markdown format. If a Thai Wikipedia equivalent exists, its target is already stubbed; otherwise, translate both the link text and target into Thai, preferably using the same text for both. Preserve `<...>` around any angle-bracketed link destination; it keeps destinations with spaces or parentheses valid in Markdown.
- wrap the complete output in a Markdown code block (```md ... ```).

Thai is not English: Translate this into Thai in a natural, readable, encyclopedia-like style. You may rephrase, rewrite, or reorder sentences freely; prioritize natural Thai over literal translation. คำสร้อย and other expressions that improve fluency are allowed, as long as the tone remains encyclopedic. For difficult scientific or technical terms, include the original English term in parentheses when helpful. Preserve the original meaning and factual accuracy.
";

#[wasm_bindgen]
pub fn get_full_prompt(original_text: &str) -> String {
    format!("{}\n{}", PROMPT.to_owned(), original_text)
}

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
