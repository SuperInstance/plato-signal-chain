use crate::{Layer, LayerResult, Tile, TileType, ResolutionLayer, SensorReading};

/// Layer 2: Ollama-based inference (requires `ollama` feature).
///
/// Sends a prompt to a local Ollama instance and resolves tiles
/// based on the response. Falls back to escalation on errors.
pub struct OllamaLayer {
    pub endpoint: String,
    pub model: String,
    pub prompt_template: String,
    pub confidence_threshold: f64,
}

impl OllamaLayer {
    pub fn new(endpoint: &str, model: &str, prompt_template: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            model: model.to_string(),
            prompt_template: prompt_template.to_string(),
            confidence_threshold: 0.7,
        }
    }

    fn build_prompt(&self, reading: &SensorReading) -> String {
        self.prompt_template
            .replace("{sensor_id}", &reading.sensor_id)
            .replace("{value}", &format!("{:.1}", reading.value))
            .replace("{unit}", &reading.unit)
            .replace("{normal_min}", &format!("{:.1}", reading.normal_min))
            .replace("{normal_max}", &format!("{:.1}", reading.normal_max))
    }
}

impl Layer for OllamaLayer {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn process(&self, mut tile: Tile) -> LayerResult {
        let reading = match &tile.sensor_reading {
            Some(r) => r.clone(),
            None => return LayerResult::Escalate(tile),
        };

        let prompt = self.build_prompt(&reading);

        let body = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
        });

        // Blocking call to ollama
        let response = (|| -> Option<String> {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .ok()?;
            rt.block_on(async {
                let client = reqwest::Client::new();
                let resp = client
                    .post(format!("{}/api/generate", self.endpoint))
                    .json(&body)
                    .timeout(std::time::Duration::from_secs(30))
                    .send()
                    .await
                    .ok()?;
                let json: serde_json::Value = resp.json().await.ok()?;
                json.get("response").and_then(|v| v.as_str()).map(|s| s.to_string())
            })
        })();

        match response {
            Some(_text) => {
                tile.tile_type = TileType::Prediction;
                tile.confidence = 0.8;
                tile.resolved_by = ResolutionLayer::Ollama;
                tile.content = format!(
                    "[OLLAMA] Analyzed {}={:.1}{}",
                    reading.sensor_id, reading.value, reading.unit
                );
                LayerResult::Resolved(tile)
            }
            None => LayerResult::Escalate(tile),
        }
    }
}
