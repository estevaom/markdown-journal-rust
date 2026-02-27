use anyhow::Result;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize)]
struct EmbedRequest {
    texts: Vec<String>,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

pub struct EmbeddingGenerator {
    client: Client,
    service_url: String,
}

impl EmbeddingGenerator {
    pub fn new() -> Result<Self> {
        let service_url = std::env::var("EMBEDDING_SERVICE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8000".to_string());

        println!("🔗 Connecting to embedding service at {}", service_url);

        let client = Client::builder().timeout(Duration::from_secs(120)).build()?;

        let health_url = format!("{}/health", service_url);
        let response = client.get(&health_url).send();

        match response {
            Ok(resp) if resp.status().is_success() => {
                println!("✅ Embedding service is healthy");
            }
            Ok(resp) => {
                return Err(anyhow::anyhow!(
                    "Embedding service returned status: {}. Is it running?",
                    resp.status()
                ));
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Cannot connect to embedding service at {}: {}\n\
                     Start it with: ./start-server.sh",
                    service_url,
                    e
                ));
            }
        }

        let info_url = format!("{}/info", service_url);
        let info: serde_json::Value = client.get(&info_url).send()?.json()?;

        let dimension = info["dimensions"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Failed to get dimension from service"))?
            as usize;

        println!("📊 Model info:");
        println!("  Name: {}", info["model_name"].as_str().unwrap_or("unknown"));
        println!("  Dimensions: {}", dimension);
        println!("  Device: {}", info["device"].as_str().unwrap_or("unknown"));

        Ok(Self { client, service_url })
    }

    pub fn generate_embeddings(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let request = EmbedRequest { texts };

        let response = self
            .client
            .post(&format!("{}/embed", self.service_url))
            .json(&request)
            .send()?
            .error_for_status()?
            .json::<EmbedResponse>()?;

        Ok(response.embeddings)
    }

    pub fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.generate_embeddings(vec![text.to_string()])?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No embedding returned"))
    }

    // rag-search doesn't need to store the model dimension; it uses the returned embedding length.
}
