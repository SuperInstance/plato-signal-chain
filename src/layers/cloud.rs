use crate::{Layer, LayerResult, Tile, TileType, ResolutionLayer};

/// Layer 3: Cloud API placeholder.
///
/// In production, this would call a remote LLM API. For now,
/// it resolves any tile that reaches it with a generic cloud response.
pub struct CloudLayer {
    /// If true, resolve tiles. If false, escalate (simulate cloud unavailable).
    pub available: bool,
}

impl CloudLayer {
    pub fn new() -> Self {
        Self { available: true }
    }

    pub fn unavailable() -> Self {
        Self { available: false }
    }
}

impl Default for CloudLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Layer for CloudLayer {
    fn name(&self) -> &'static str {
        "cloud"
    }

    fn process(&self, mut tile: Tile) -> LayerResult {
        if self.available {
            tile.tile_type = TileType::Escalation;
            tile.content = format!("[CLOUD] {}", tile.content);
            tile.resolved_by = ResolutionLayer::Cloud;
            tile.confidence = 0.5; // Cloud is a fallback, moderate confidence
            LayerResult::Resolved(tile)
        } else {
            LayerResult::Escalate(tile)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{make_reading, tile_from_reading};

    #[test]
    fn cloud_resolves_when_available() {
        let layer = CloudLayer::new();
        let r = make_reading("x", 1.0, 0.0, 2.0);
        match layer.process(tile_from_reading(&r)) {
            LayerResult::Resolved(t) => {
                assert_eq!(t.resolved_by, ResolutionLayer::Cloud);
            }
            LayerResult::Escalate(_) => panic!("Cloud should resolve when available"),
        }
    }

    #[test]
    fn cloud_escalates_when_unavailable() {
        let layer = CloudLayer::unavailable();
        let r = make_reading("x", 1.0, 0.0, 2.0);
        match layer.process(tile_from_reading(&r)) {
            LayerResult::Escalate(_) => {}
            LayerResult::Resolved(_) => panic!("Cloud unavailable → escalate"),
        }
    }
}
