use reqwest::{Client as HttpClient, multipart};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

const DEFAULT_BASE_URL: &str = "https://api.safecomms.dev";

#[derive(Error, Debug)]
pub enum SafeCommsError {
    #[error("HTTP request failed")]
    RequestError(#[from] reqwest::Error),
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Serialization error")]
    SerializationError(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct SafeCommsClient {
    client: HttpClient,
    base_url: String,
    api_key: String,
}

#[derive(Serialize)]
pub struct TextModerationRequest<'a> {
    pub content: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replace: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pii: Option<bool>,
    #[serde(rename = "replaceSeverity", skip_serializing_if = "Option::is_none")]
    pub replace_severity: Option<&'a str>,
    #[serde(rename = "moderationProfileId", skip_serializing_if = "Option::is_none")]
    pub moderation_profile_id: Option<&'a str>,
}

#[derive(Serialize)]
pub struct ImageModerationRequest<'a> {
    pub image: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<&'a str>,
    #[serde(rename = "moderationProfileId", skip_serializing_if = "Option::is_none")]
    pub moderation_profile_id: Option<&'a str>,
    #[serde(rename = "enableOcr", skip_serializing_if = "Option::is_none")]
    pub enable_ocr: Option<bool>,
    #[serde(rename = "enhancedOcr", skip_serializing_if = "Option::is_none")]
    pub enhanced_ocr: Option<bool>,
    #[serde(rename = "extractMetadata", skip_serializing_if = "Option::is_none")]
    pub extract_metadata: Option<bool>,
}

#[derive(Deserialize, Debug)]
pub struct ModerationResponse {
    #[serde(rename = "isClean")]
    pub is_clean: bool,
    pub severity: Option<String>,
    #[serde(rename = "categoryScores")]
    pub category_scores: Option<HashMap<String, String>>,
    pub issues: Option<Vec<ModerationIssue>>,
    pub reason: Option<String>,
    #[serde(rename = "isBypassAttempt")]
    pub is_bypass_attempt: bool,
    #[serde(rename = "safeContent")]
    pub safe_content: Option<String>,
    pub addons: Option<AddonUsage>,
}

#[derive(Deserialize, Debug)]
pub struct ModerationIssue {
    pub term: Option<String>,
    pub context: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct AddonUsage {
    #[serde(rename = "replacedUnsafe")]
    pub replaced_unsafe: bool,
    #[serde(rename = "replacedPii")]
    pub replaced_pii: bool,
}

#[derive(Deserialize, Debug)]
pub struct UsageResponse {
    pub tier: String,
    #[serde(rename = "rateLimit")]
    pub rate_limit: i32,
    #[serde(rename = "tokenLimit")]
    pub token_limit: Option<i32>,
    #[serde(rename = "tokensUsed")]
    pub tokens_used: i32,
    #[serde(rename = "remainingTokens")]
    pub remaining_tokens: i32,
}

#[derive(Deserialize, Debug)]
struct ProblemDetails {
    detail: Option<String>,
    title: Option<String>,
}

impl SafeCommsClient {
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        Self {
            client: HttpClient::new(),
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
                .trim_end_matches('/')
                .to_string(),
            api_key,
        }
    }

    pub async fn moderate_text(
        &self,
        content: &str,
        language: Option<&str>,
        replace: Option<bool>,
        pii: Option<bool>,
        replace_severity: Option<&str>,
        moderation_profile_id: Option<&str>,
    ) -> Result<ModerationResponse, SafeCommsError> {
        let request = TextModerationRequest {
            content,
            language,
            replace,
            pii,
            replace_severity,
            moderation_profile_id,
        };

        let response = self.client
            .post(format!("{}/moderation/text", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            
            // Try to parse ProblemDetails
            if let Ok(problem) = serde_json::from_str::<ProblemDetails>(&error_text) {
                return Err(SafeCommsError::ApiError(
                    problem.detail.or(problem.title).unwrap_or_else(|| status.to_string())
                ));
            }
            
            return Err(SafeCommsError::ApiError(format!("{} - {}", status, error_text)));
        }

        let result = response.json::<ModerationResponse>().await?;
        Ok(result)
    }

    pub async fn moderate_image(
        &self,
        request: ImageModerationRequest<'_>,
    ) -> Result<ModerationResponse, SafeCommsError> {
        let response = self.client
            .post(format!("{}/moderation/image", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            
            if let Ok(problem) = serde_json::from_str::<ProblemDetails>(&error_text) {
                return Err(SafeCommsError::ApiError(
                    problem.detail.or(problem.title).unwrap_or_else(|| status.to_string())
                ));
            }
            
            return Err(SafeCommsError::ApiError(format!("{} - {}", status, error_text)));
        }

        let result = response.json::<ModerationResponse>().await?;
        Ok(result)
    }

    pub async fn moderate_image_file(
        &self,
        file_path: &str,
        language: Option<&str>,
        moderation_profile_id: Option<&str>,
        enable_ocr: Option<bool>,
        enhanced_ocr: Option<bool>,
        extract_metadata: Option<bool>,
    ) -> Result<ModerationResponse, SafeCommsError> {
        let file_bytes = tokio::fs::read(file_path).await
            .map_err(|e| SafeCommsError::ApiError(format!("Failed to read file: {}", e)))?;
        
        let file_name = Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("image.jpg")
            .to_string();

        let mut form = multipart::Form::new()
            .part("image", multipart::Part::bytes(file_bytes).file_name(file_name));

        if let Some(lang) = language {
            form = form.text("language", lang.to_string());
        }
        
        if let Some(profile_id) = moderation_profile_id {
            form = form.text("moderationProfileId", profile_id.to_string());
        }

        if let Some(enable) = enable_ocr {
            form = form.text("enableOcr", enable.to_string());
        }

        if let Some(enhanced) = enhanced_ocr {
            form = form.text("enhancedOcr", enhanced.to_string());
        }

        if let Some(extract) = extract_metadata {
            form = form.text("extractMetadata", extract.to_string());
        }

        let response = self.client
            .post(format!("{}/moderation/image/upload", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            
            if let Ok(problem) = serde_json::from_str::<ProblemDetails>(&error_text) {
                return Err(SafeCommsError::ApiError(
                    problem.detail.or(problem.title).unwrap_or_else(|| status.to_string())
                ));
            }
            
            return Err(SafeCommsError::ApiError(format!("{} - {}", status, error_text)));
        }

        let result = response.json::<ModerationResponse>().await?;
        Ok(result)
    }

    pub async fn get_usage(&self) -> Result<UsageResponse, SafeCommsError> {
        let response = self.client
            .get(format!("{}/usage", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
             if let Ok(problem) = serde_json::from_str::<ProblemDetails>(&error_text) {
                return Err(SafeCommsError::ApiError(
                    problem.detail.or(problem.title).unwrap_or_else(|| status.to_string())
                ));
            }
            return Err(SafeCommsError::ApiError(format!("{} - {}", status, error_text)));
        }

        let result = response.json::<UsageResponse>().await?;
        Ok(result)
    }
}
