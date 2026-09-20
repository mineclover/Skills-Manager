use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use reqwest::Url;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn generate_code_verifier() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

pub fn build_web_auth_start_url(
    base_url: &str,
    provider: &str,
    state: &str,
    code_challenge: &str,
    nonce: &str,
    locale: Option<&str>,
) -> Result<Url, String> {
    let base = base_url.trim_end_matches('/');
    let mut url = Url::parse(&format!("{base}/auth/start"))
        .map_err(|e| format!("Invalid web auth base url: {e}"))?;
    {
        let mut query_pairs = url.query_pairs_mut();
        query_pairs.append_pair("provider", provider);
        query_pairs.append_pair("state", state);
        query_pairs.append_pair("code_challenge", code_challenge);
        query_pairs.append_pair("nonce", nonce);
        query_pairs.append_pair("client", "desktop");
        if let Some(locale) = locale.map(str::trim).filter(|value| !value.is_empty()) {
            query_pairs.append_pair("locale", locale);
        }
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    #[test]
    fn web_auth_start_url_uses_first_party_route() {
        let url = super::build_web_auth_start_url(
            "https://skillsmanager.freeourdays.com/",
            "google",
            "s1",
            "cc1",
            "n1",
            Some("zh-CN"),
        )
        .unwrap();
        assert_eq!(url.path(), "/auth/start");
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query.get("provider"), Some(&"google".to_string()));
        assert_eq!(query.get("client"), Some(&"desktop".to_string()));
        assert_eq!(query.get("locale"), Some(&"zh-CN".to_string()));
    }
}
