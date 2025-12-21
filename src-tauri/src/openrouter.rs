use crate::config::Config;
use crate::prompt;
use crate::ModelInfo;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, error, info};

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    reasoning: Reasoning,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct Reasoning {
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

pub async fn translate(config: &Config, input: &str) -> Result<String> {
    if config.api_key.trim().is_empty() {
        return Err(anyhow!("API key is empty"));
    }

    let prompt = prompt::build_prompt(input, &config.target_language);
    info!(
        model = %config.model,
        target_language = %config.target_language,
        reasoning = config.reasoning_enabled,
        input_len = input.len(),
        prompt_len = prompt.len(),
        input_preview = %preview(input, 200),
        "OpenRouter request prepared"
    );
    let request = ChatRequest {
        model: config.model.clone(),
        messages: vec![Message {
            role: "user".to_string(),
            content: prompt,
        }],
        reasoning: Reasoning {
            enabled: config.reasoning_enabled,
        },
    };

    let client = reqwest::Client::new();
    let start = Instant::now();
    let response = client
        .post(OPENROUTER_URL)
        .bearer_auth(&config.api_key)
        .json(&request)
        .send()
        .await
        .context("send OpenRouter request");

    let response = match response {
        Ok(response) => response,
        Err(e) => {
            error!(
                error = %e,
                elapsed_ms = start.elapsed().as_millis(),
                "OpenRouter request failed"
            );
            return Err(e);
        }
    };

    let status = response.status();
    let body = response.text().await.context("read response body");
    let body = match body {
        Ok(body) => body,
        Err(e) => {
            error!(
                error = %e,
                status = %status,
                elapsed_ms = start.elapsed().as_millis(),
                "OpenRouter response read failed"
            );
            return Err(e);
        }
    };
    let duration_ms = start.elapsed().as_millis();

    if !status.is_success() {
        error!(
            status = %status,
            duration_ms,
            body_preview = %preview(&body, 400),
            "OpenRouter request failed"
        );
        return Err(anyhow!("OpenRouter error {}: {}", status, body));
    }

    info!(status = %status, duration_ms, "OpenRouter response received");

    let parsed: ChatResponse = match serde_json::from_str(&body).context("parse response json") {
        Ok(parsed) => parsed,
        Err(e) => {
            error!(
                error = %e,
                body_preview = %preview(&body, 400),
                "OpenRouter response parse failed"
            );
            return Err(e);
        }
    };
    let content = match parsed.choices.first() {
        Some(choice) => choice.message.content.as_str(),
        None => {
            error!(
                body_preview = %preview(&body, 400),
                "OpenRouter response missing choices"
            );
            return Err(anyhow!("OpenRouter response missing choices"));
        }
    };

    debug!(
        response_len = content.len(),
        response_preview = %preview(content, 400),
        "OpenRouter response parsed"
    );

    let extracted = match prompt::extract_translation(content) {
        Some(extracted) => extracted,
        None => {
            error!(
                response_preview = %preview(content, 400),
                "OpenRouter response missing translation markers"
            );
            return Err(anyhow!("Missing translation markers in response"));
        }
    };

    info!(
        translated_len = extracted.len(),
        translated_preview = %preview(&extracted, 200),
        "OpenRouter translation extracted"
    );

    Ok(extracted)
}

fn preview(input: &str, limit: usize) -> String {
    let cleaned = input.replace('\n', " ").replace('\r', " ");
    let mut out = String::new();
    let mut chars = cleaned.chars();
    for _ in 0..limit {
        if let Some(ch) = chars.next() {
            out.push(ch);
        } else {
            return out;
        }
    }
    if chars.next().is_some() {
        out.push_str("...");
    }
    out
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelData>,
}

#[derive(Debug, Deserialize)]
struct ModelData {
    id: String,
    name: String,
}

pub async fn fetch_models(api_key: &str) -> Result<Vec<ModelInfo>> {
    let client = reqwest::Client::new();
    let start = Instant::now();

    debug!("Fetching models from OpenRouter");

    let response = client
        .get(OPENROUTER_MODELS_URL)
        .bearer_auth(api_key)
        .send()
        .await
        .context("send OpenRouter models request")?;

    let status = response.status();
    let body = response
        .text()
        .await
        .context("read models response body")?;

    let duration_ms = start.elapsed().as_millis();

    if !status.is_success() {
        error!(
            status = %status,
            duration_ms,
            body_preview = %preview(&body, 400),
            "OpenRouter models request failed"
        );
        return Err(anyhow!("OpenRouter error {}: {}", status, body));
    }

    info!(status = %status, duration_ms, "OpenRouter models response received");

    let parsed: ModelsResponse = serde_json::from_str(&body).context("parse models response")?;

    let models: Vec<ModelInfo> = parsed
        .data
        .into_iter()
        .map(|m| ModelInfo {
            id: m.id,
            name: m.name,
        })
        .collect();

    info!(count = models.len(), "Models parsed successfully");
    Ok(models)
}
