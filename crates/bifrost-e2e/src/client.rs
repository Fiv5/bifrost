use reqwest::{Client, Proxy, Response};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

pub struct ProxyClient {
    client: Client,
    proxy_url: String,
}

impl ProxyClient {
    pub fn new(proxy_url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let proxy = Proxy::all(proxy_url)?;
        let client = Client::builder()
            .proxy(proxy)
            .timeout(Duration::from_secs(30))
            .danger_accept_invalid_certs(true)
            .build()?;

        Ok(Self {
            client,
            proxy_url: proxy_url.to_string(),
        })
    }

    pub fn proxy_url(&self) -> &str {
        &self.proxy_url
    }

    pub async fn get(&self, url: &str) -> Result<Response, reqwest::Error> {
        self.client.get(url).send().await
    }

    pub async fn get_with_headers(
        &self,
        url: &str,
        headers: HashMap<&str, &str>,
    ) -> Result<Response, reqwest::Error> {
        let mut builder = self.client.get(url);
        for (k, v) in headers {
            builder = builder.header(k, v);
        }
        builder.send().await
    }

    pub async fn post(&self, url: &str, body: &str) -> Result<Response, reqwest::Error> {
        self.client
            .post(url)
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .await
    }

    pub async fn get_json(&self, url: &str) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let resp = self.get(url).await?;
        let json: Value = resp.json().await?;
        Ok(json)
    }

    pub async fn get_text(&self, url: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let resp = self.get(url).await?;
        let text = resp.text().await?;
        Ok(text)
    }
}

pub struct DirectClient {
    client: Client,
}

impl DirectClient {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .danger_accept_invalid_certs(true)
            .build()?;

        Ok(Self { client })
    }

    pub async fn get(&self, url: &str) -> Result<Response, reqwest::Error> {
        self.client.get(url).send().await
    }

    pub async fn get_json(&self, url: &str) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let resp = self.get(url).await?;
        let json: Value = resp.json().await?;
        Ok(json)
    }
}
