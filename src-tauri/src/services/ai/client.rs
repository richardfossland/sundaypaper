//! Anthropic Messages API client — the only impure part of the intent→layout
//! compiler, behind the `ai` cargo feature. Mirrors `layout::engine`'s
//! enabled/disabled split exactly.
//!
//! The pure halves do the real work: [`prompt::build_request`] builds the body
//! and [`parse::parse_block_tree`] validates the response. This module only adds
//! the HTTPS round trip in between, with the API key read at call time (never
//! bundled, never logged). A build WITHOUT the `ai` feature compiles fine and
//! [`compile_intent`] returns `FeatureDisabled` so the command can surface
//! "AI ikke aktivert" and the manual builder is unaffected.

#[cfg(feature = "ai")]
pub use enabled::compile_intent;

#[cfg(not(feature = "ai"))]
pub use disabled::compile_intent;

/// The Anthropic Messages API endpoint. Shared so the (future) test transport
/// and the live client agree.
pub const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";

/// The feature-on implementation: build → POST → parse.
#[cfg(feature = "ai")]
mod enabled {
    use std::time::Duration;

    use crate::error::{AppError, AppResult};
    use crate::services::ai::parse::{parse_block_tree, IntentCompileResult};
    use crate::services::ai::prompt::{build_request, IntentRequest, ANTHROPIC_VERSION};

    use super::MESSAGES_URL;

    /// Compile a free-text intent into a validated block tree by calling the
    /// Anthropic Messages API.
    ///
    /// `api_key` is the caller's Anthropic key, read from the OS keychain / local
    /// setting just before the call — it is never stored in this crate and never
    /// written to a log. The consent gate and intent validation live in the pure
    /// `build_request`, so a request is never sent without opt-in.
    ///
    /// Network / HTTP / decode failures map to a `Pdf`-free `Internal` error
    /// carrying a short reason (we don't have a dedicated `Ai` variant, and the
    /// renderer switches on `code()` which stays stable). Anthropic API error
    /// bodies surface their `error.message` so the user sees what went wrong.
    pub async fn compile_intent(
        req: &IntentRequest,
        api_key: &str,
    ) -> AppResult<IntentCompileResult> {
        // Pure: validates consent + intent and builds the exact JSON body.
        let body = build_request(req)?;

        let key = api_key.trim();
        if key.is_empty() {
            // No key reached us despite the feature being on — same UX as off.
            return Err(AppError::FeatureDisabled { feature: "ai" });
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| AppError::Internal(format!("AI client init failed: {e}")))?;

        let resp = client
            .post(MESSAGES_URL)
            .header("x-api-key", key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("AI request failed: {e}")))?;

        let status = resp.status();
        let payload: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("AI response was not JSON: {e}")))?;

        if !status.is_success() {
            // Surface Anthropic's own error message when present.
            let msg = payload
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown error");
            return Err(AppError::Internal(format!(
                "AI request returned {}: {msg}",
                status.as_u16()
            )));
        }

        // Pure: re-validates and sanitises into ordered BlockSpecs.
        parse_block_tree(&payload)
    }
}

/// The feature-off stub: no HTTP client in this build, so report it plainly.
/// Mirrors `layout::engine`'s `disabled` module.
#[cfg(not(feature = "ai"))]
mod disabled {
    use crate::error::{AppError, AppResult};
    use crate::services::ai::parse::IntentCompileResult;
    use crate::services::ai::prompt::IntentRequest;

    /// Compile an intent — unavailable without the `ai` feature. Returns
    /// `FeatureDisabled` so the command can show "AI ikke aktivert" and leave the
    /// manual builder untouched.
    pub async fn compile_intent(
        _req: &IntentRequest,
        _api_key: &str,
    ) -> AppResult<IntentCompileResult> {
        Err(AppError::FeatureDisabled { feature: "ai" })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::error::AppError;

        #[tokio::test]
        async fn compile_without_feature_reports_feature_disabled() {
            let req = IntentRequest {
                intent: "lag et program".into(),
                consent: true,
                purpose: None,
                lang: None,
            };
            let err = compile_intent(&req, "sk-ant-whatever")
                .await
                .expect_err("disabled build must not call the API");
            assert!(matches!(err, AppError::FeatureDisabled { feature: "ai" }));
            assert_eq!(err.code(), "feature_disabled");
        }
    }
}
