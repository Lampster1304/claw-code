use crate::error::ApiError;
use crate::prompt_cache::{PromptCache, PromptCacheRecord, PromptCacheStats};
use crate::providers::anthropic::{self, AuthSource};
use crate::providers::ollama::{self, OllamaClient};
use crate::providers::openai_compat::{self, OpenAiCompatClient, OpenAiCompatConfig};
use crate::providers::{self, ProviderKind};
use crate::types::{MessageRequest, MessageResponse, StreamEvent};

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum ProviderClient {
    Local(OllamaClient),
    CloudAnthropic(anthropic::AnthropicClient),
    CloudOpenAi(OpenAiCompatClient),
}

impl ProviderClient {
    pub fn from_model(model: &str) -> Result<Self, ApiError> {
        Self::from_model_with_anthropic_auth(model, None)
    }

    pub fn from_model_with_anthropic_auth(
        model: &str,
        anthropic_auth: Option<AuthSource>,
    ) -> Result<Self, ApiError> {
        let resolved_model = providers::resolve_model_alias(model);
        match providers::detect_provider_kind(&resolved_model) {
            ProviderKind::Local => Ok(Self::Local(OllamaClient::from_model(&resolved_model))),
            ProviderKind::Cloud => {
                if resolved_model.starts_with("claude") {
                    let auth = match anthropic_auth {
                        Some(auth) => auth,
                        None => AuthSource::from_env_or_saved()?,
                    };
                    Ok(Self::CloudAnthropic(
                        anthropic::AnthropicClient::from_auth(auth)
                            .with_base_url(anthropic::read_base_url()),
                    ))
                } else if resolved_model.starts_with("grok") {
                    Ok(Self::CloudOpenAi(OpenAiCompatClient::from_env(
                        OpenAiCompatConfig::xai(),
                    )?))
                } else if resolved_model.starts_with("qwen/") || resolved_model.starts_with("qwen-")
                {
                    Ok(Self::CloudOpenAi(OpenAiCompatClient::from_env(
                        OpenAiCompatConfig::dashscope(),
                    )?))
                } else {
                    Ok(Self::CloudOpenAi(OpenAiCompatClient::from_env(
                        OpenAiCompatConfig::openai(),
                    )?))
                }
            }
        }
    }

    #[must_use]
    pub const fn provider_kind(&self) -> ProviderKind {
        match self {
            Self::Local(_) => ProviderKind::Local,
            Self::CloudAnthropic(_) | Self::CloudOpenAi(_) => ProviderKind::Cloud,
        }
    }

    #[must_use]
    pub fn with_prompt_cache(self, prompt_cache: PromptCache) -> Self {
        match self {
            Self::Local(client) => Self::Local(client.with_prompt_cache(prompt_cache)),
            Self::CloudAnthropic(client) => {
                Self::CloudAnthropic(client.with_prompt_cache(prompt_cache))
            }
            Self::CloudOpenAi(client) => Self::CloudOpenAi(client),
        }
    }

    #[must_use]
    pub fn prompt_cache_stats(&self) -> Option<PromptCacheStats> {
        match self {
            Self::Local(client) => client.prompt_cache_stats(),
            Self::CloudAnthropic(client) => client.prompt_cache_stats(),
            Self::CloudOpenAi(_) => None,
        }
    }

    #[must_use]
    pub fn take_last_prompt_cache_record(&self) -> Option<PromptCacheRecord> {
        match self {
            Self::Local(client) => client.take_last_prompt_cache_record(),
            Self::CloudAnthropic(client) => client.take_last_prompt_cache_record(),
            Self::CloudOpenAi(_) => None,
        }
    }

    pub async fn send_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageResponse, ApiError> {
        match self {
            Self::Local(client) => client.send_message(request).await,
            Self::CloudAnthropic(client) => client.send_message(request).await,
            Self::CloudOpenAi(client) => client.send_message(request).await,
        }
    }

    pub async fn stream_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageStream, ApiError> {
        match self {
            Self::Local(client) => client
                .stream_message(request)
                .await
                .map(MessageStream::Local),
            Self::CloudAnthropic(client) => client
                .stream_message(request)
                .await
                .map(MessageStream::Anthropic),
            Self::CloudOpenAi(client) => client
                .stream_message(request)
                .await
                .map(MessageStream::OpenAiCompat),
        }
    }
}

#[derive(Debug)]
pub enum MessageStream {
    Local(ollama::MessageStream),
    Anthropic(anthropic::MessageStream),
    OpenAiCompat(openai_compat::MessageStream),
}

impl MessageStream {
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Local(stream) => stream.request_id(),
            Self::Anthropic(stream) => stream.request_id(),
            Self::OpenAiCompat(stream) => stream.request_id(),
        }
    }

    pub async fn next_event(&mut self) -> Result<Option<StreamEvent>, ApiError> {
        match self {
            Self::Local(stream) => stream.next_event().await,
            Self::Anthropic(stream) => stream.next_event().await,
            Self::OpenAiCompat(stream) => stream.next_event().await,
        }
    }
}

pub use anthropic::{
    oauth_token_is_expired, resolve_saved_oauth_token, resolve_startup_auth_source, OAuthTokenSet,
};
#[must_use]
pub fn read_base_url() -> String {
    anthropic::read_base_url()
}

#[must_use]
pub fn read_xai_base_url() -> String {
    openai_compat::read_base_url(OpenAiCompatConfig::xai())
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::ProviderClient;
    use crate::providers::{detect_provider_kind, resolve_model_alias, ProviderKind};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn resolves_existing_and_grok_aliases() {
        assert_eq!(resolve_model_alias("opus"), "claude-opus-4-6");
        assert_eq!(resolve_model_alias("grok"), "grok-3");
        assert_eq!(resolve_model_alias("grok-mini"), "grok-3-mini");
    }

    #[test]
    fn provider_detection_prefers_local_when_local_env_and_openai_key_both_set() {
        let _lock = env_lock();
        let _local_provider = EnvVarGuard::set("AGCLI_LOCAL_PROVIDER", Some("ollama"));
        let _openai = EnvVarGuard::set("OPENAI_API_KEY", Some("test-openai-key"));

        assert_eq!(detect_provider_kind("llama3.2"), ProviderKind::Local);
    }

    #[test]
    fn provider_client_routes_openai_prefixed_model_to_cloud() {
        let _lock = env_lock();
        let _openai = EnvVarGuard::set("OPENAI_API_KEY", Some("test-openai-key"));

        let client = ProviderClient::from_model("openai/gpt-4.1-mini")
            .expect("cloud provider client should be constructed");

        assert_eq!(client.provider_kind(), ProviderKind::Cloud);
    }

    struct EnvVarGuard {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: Option<&str>) -> Self {
            let original = std::env::var_os(key);
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match self.original.take() {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn provider_client_defaults_to_cloud_openai_config() {
        let _lock = env_lock();
        let _openai = EnvVarGuard::set("OPENAI_API_KEY", Some("test-openai-key"));

        let client = ProviderClient::from_model("gpt-4o-mini");

        assert!(
            client.is_ok(),
            "gpt-4o-mini with OPENAI_API_KEY set should build successfully, got: {:?}",
            client.err()
        );

        match client.unwrap() {
            ProviderClient::CloudOpenAi(openai_client) => {
                assert!(
                    openai_client.base_url().contains("api.openai.com"),
                    "gpt-4o-mini should route to OpenAI base URL, got: {}",
                    openai_client.base_url()
                );
            }
            other => panic!(
                "Expected ProviderClient::CloudOpenAi for gpt-4o-mini, got: {:?}",
                other
            ),
        }
    }
}
