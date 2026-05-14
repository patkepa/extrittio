use anyhow::{Context, Result, anyhow, bail};
use reqwest::{Method, StatusCode, multipart};
use serde::de::DeserializeOwned;
use serde_json::Value;

#[derive(Clone)]
pub(crate) struct ApiClient {
    http: reqwest::Client,
    base_url: String,
    token: Option<String>,
}

impl ApiClient {
    pub(crate) fn new(base_url: String, token: Option<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
            token,
        }
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    pub(crate) fn has_token(&self) -> bool {
        self.token.is_some()
    }

    pub(crate) async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.request(Method::GET, path, None, true).await
    }

    pub(crate) async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        auth: bool,
    ) -> Result<T> {
        let response = self.send(method, path, body, auth).await?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .context("failed to read response body")?;

        if !status.is_success() {
            bail!("{}", api_error(status, &bytes));
        }

        serde_json::from_slice(&bytes).with_context(|| {
            format!(
                "failed to parse response from {} as JSON",
                path.trim_start_matches('/')
            )
        })
    }

    pub(crate) async fn request_empty(&self, method: Method, path: &str) -> Result<()> {
        let response = self.send(method, path, None, true).await?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .context("failed to read response body")?;

        if !status.is_success() {
            bail!("{}", api_error(status, &bytes));
        }

        Ok(())
    }

    pub(crate) async fn request_empty_json(
        &self,
        method: Method,
        path: &str,
        body: Value,
    ) -> Result<()> {
        let response = self.send(method, path, Some(body), true).await?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .context("failed to read response body")?;

        if !status.is_success() {
            bail!("{}", api_error(status, &bytes));
        }

        Ok(())
    }

    pub(crate) async fn request_multipart<T: DeserializeOwned>(
        &self,
        path: &str,
        form: multipart::Form,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let token = self
            .token
            .as_deref()
            .ok_or_else(|| anyhow!("not authenticated; run `extrittio auth login` first"))?;
        let response = self
            .http
            .post(url)
            .bearer_auth(token)
            .multipart(form)
            .send()
            .await
            .context("request failed")?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .context("failed to read response body")?;

        if !status.is_success() {
            bail!("{}", api_error(status, &bytes));
        }

        serde_json::from_slice(&bytes).with_context(|| {
            format!(
                "failed to parse response from {} as JSON",
                path.trim_start_matches('/')
            )
        })
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        auth: bool,
    ) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url, path);
        let mut request = self.http.request(method, url);

        if auth {
            let token = self
                .token
                .as_deref()
                .ok_or_else(|| anyhow!("not authenticated; run `extrittio auth login` first"))?;
            request = request.bearer_auth(token);
        }

        if let Some(body) = body {
            request = request.json(&body);
        }

        request.send().await.context("request failed")
    }
}

fn api_error(status: StatusCode, bytes: &[u8]) -> String {
    let message = serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
        .or_else(|| String::from_utf8(bytes.to_vec()).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| status.to_string());

    format!("API error {status}: {message}")
}
