# SafeComms Rust SDK

Official Rust client for the SafeComms API.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
safecomms = { git = "https://github.com/your-org/safecomms", branch = "main" }
tokio = { version = "1.0", features = ["full"] }
```

## Usage

```rust
use safecomms::SafeCommsClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SafeCommsClient::new(
        "your-api-key".to_string(),
        None // Use default base URL
    );

    // Moderate text
    let result = client.moderate_text(
        "Some text to check",
        Some("en"), // language
        Some(false), // replace
        Some(false), // pii
        None, // replace_severity
        None // moderation_profile_id
    ).await?;

    println!("Is clean: {}", result.is_clean);

    // Get usage
    let usage = client.get_usage().await?;
    println!("Tokens used: {}", usage.tokens_used);

    Ok(())
}
```
