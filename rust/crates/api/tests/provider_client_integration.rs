use std::ffi::OsString;
use std::sync::{Mutex, OnceLock};

use api::{ApiError, ProviderClient, ProviderKind};

#[test]
fn provider_client_routes_openai_prefixed_model_to_cloud() {
    let _lock = env_lock();
    let _openai_api_key = EnvVarGuard::set("OPENAI_API_KEY", Some("openai-test-key"));

    let client =
        ProviderClient::from_model("openai/gpt-4.1-mini").expect("cloud route should resolve");

    assert_eq!(client.provider_kind(), ProviderKind::Cloud);
}

#[test]
fn provider_client_prefers_local_when_ollama_host_is_set() {
    let _lock = env_lock();
    let _openai_api_key = EnvVarGuard::set("OPENAI_API_KEY", Some("openai-test-key"));
    let _ollama_host = EnvVarGuard::set("OLLAMA_HOST", Some("http://127.0.0.1:11434"));

    let client = ProviderClient::from_model("llama3.2").expect("local route should resolve");

    assert_eq!(client.provider_kind(), ProviderKind::Local);
}

#[test]
fn provider_client_reports_missing_openai_credentials_for_cloud_models() {
    let _lock = env_lock();
    let _openai_api_key = EnvVarGuard::set("OPENAI_API_KEY", None);
    let _ollama_host = EnvVarGuard::set("OLLAMA_HOST", None);

    let error = ProviderClient::from_model("openai/gpt-4.1-mini")
        .expect_err("cloud requests without OPENAI_API_KEY should fail fast");

    match error {
        ApiError::MissingCredentials {
            provider, env_vars, ..
        } => {
            assert_eq!(provider, "OpenAI");
            assert_eq!(env_vars, &["OPENAI_API_KEY"]);
        }
        other => panic!("expected missing OpenAI credentials, got {other:?}"),
    }
}

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct EnvVarGuard {
    key: &'static str,
    original: Option<OsString>,
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
        match &self.original {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}
