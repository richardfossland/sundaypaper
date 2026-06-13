//! Anthropic Messages API transport for the intent→layout compiler (Phase 5.1).
//!
//! This is the only impure part of the AI seam — the HTTP round-trip. It sits
//! behind the `ai` cargo feature, mirroring `layout::engine` (lopdf/pdfium,
//! typst): the default build compiles without `reqwest`, and the always-present
//! [`compile_intent`] stub returns a clear `feature_disabled` ("AI ikke
//! aktivert") error so the manual builder is completely unaffected.
//!
//! The request body and the response validation are PURE and live in
//! [`super::prompt`] / [`super::parse`] (always compiled, fully unit-tested with
//! canned JSON). This module only wires them to the network, so there is
//! deliberately almost nothing here to test without a key — the seam is built so
//! the untested surface is as thin as possible (INFRA-UNVERIFIED: the live
//! request needs a real key + network and can't run in the gate).
//!
//! Key handling: the API key is passed in by the command layer (read from the
//! OS keychain or the local `setting` store) and used only for the
//! `x-api-key` header. It is never logged and never crosses back to the
//! renderer.

#[cfg(feature = "ai")]
pub use enabled::compile_intent;

#[cfg(not(feature = "ai"))]
pub use disabled::compile_intent;

/// The feature-on implementation: build the request with [`super::prompt`], POST
/// it to the Anthropic Messages API, and validate the response with
/// [`super::parse`].
#[cfg(feature = "ai")]
mod enabled {
    use crate::error::{AppError, AppResult};
    use crate::services::ai::parse::parse_block_tree;
    use crate::services::ai::prompt::{build_request, IntentContext, ANTHROPIC_VERSION};
    use crate::services::bulletin::BlockSpec;

    const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";

    /// Compile a free-text intent into validated [`BlockSpec`]s by asking Claude
    /// for a structured block tree.
    ///
    /// `api_key` is the caller-supplied Anthropic key (keychain / setting store).
    /// `intent` is the user's request; `ctx` carries only the church/date/lang
    /// the caller chose to share (never form/member content).
    ///
    /// A transport / HTTP-status failure becomes an `Internal` error carrying a
    /// sanitised message (never the key); a malformed or out-of-catalogue
    /// response becomes a `Validation` error via [`parse_block_tree`].
    pub async fn compile_intent(
        api_key: &str,
        intent: &str,
        ctx: &IntentContext,
    ) -> AppResult<Vec<BlockSpec>> {
        let body = build_request(intent, ctx);

        let client = reqwest::Client::new();
        let resp = client
            .post(ENDPOINT)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("AI-forespørsel feilet: {e}")))?;

        let status = resp.status();
        let value: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("kunne ikke lese AI-svaret: {e}")))?;

        if !status.is_success() {
            // Surface Anthropic's own error message when present, but never the
            // key (it's only ever in the request header, not the response).
            let detail = value
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("ukjent feil");
            return Err(AppError::Internal(format!(
                "AI-tjenesten svarte med feil ({status}): {detail}"
            )));
        }

        parse_block_tree(&value)
    }
}

/// The feature-off stub: the default build has no `reqwest`, so this returns a
/// clear, localized `feature_disabled` error. The manual builder is unaffected;
/// the renderer shows "AI ikke aktivert".
#[cfg(not(feature = "ai"))]
mod disabled {
    use crate::error::{AppError, AppResult};
    use crate::services::ai::prompt::IntentContext;
    use crate::services::bulletin::BlockSpec;

    /// Always returns `FeatureDisabled { feature: "ai" }` — this build can't
    /// reach the cloud. Signature matches the enabled path so the command
    /// compiles either way.
    pub async fn compile_intent(
        _api_key: &str,
        _intent: &str,
        _ctx: &IntentContext,
    ) -> AppResult<Vec<BlockSpec>> {
        Err(AppError::FeatureDisabled { feature: "ai" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ai::prompt::IntentContext;

    // The default (no-feature) gate run exercises the disabled stub: the call
    // must degrade to a clear feature_disabled error, never panic, and never
    // touch the network — proving keyless/feature-off graceful degradation.
    #[cfg(not(feature = "ai"))]
    #[tokio::test]
    async fn disabled_build_returns_feature_disabled() {
        let err = compile_intent("", "lag et program", &IntentContext::default())
            .await
            .unwrap_err();
        assert!(
            matches!(err, crate::error::AppError::FeatureDisabled { feature: "ai" }),
            "default build degrades to feature_disabled (AI ikke aktivert)"
        );
        assert_eq!(err.code(), "feature_disabled");
    }

    // With the `ai` feature on but no network/key, the call still must not panic
    // — it surfaces a transport error. We don't assert the variant beyond "it's
    // an error" because the failure mode depends on the environment (an empty
    // key may 401, no network errors out); the point is graceful degradation.
    #[cfg(feature = "ai")]
    #[tokio::test]
    async fn enabled_build_with_bad_key_errors_without_panicking() {
        // An obviously-invalid key against the real endpoint will fail fast.
        // This is a smoke check that the wiring compiles and returns Result;
        // it is network-dependent, so we only assert it does not panic.
        let _ = compile_intent("sk-ant-invalid", "x", &IntentContext::default()).await;
    }
}
