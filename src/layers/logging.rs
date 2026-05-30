use std::sync::Mutex;
use std::time::SystemTime;

use crate::{Layer, LayerResult, Tile, ResolutionLayer};

/// A single log entry from the logging layer.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp_ms: u64,
    pub layer_name: String,
    pub tile_id: uuid::Uuid,
    pub room_id: String,
    pub confidence: f64,
    pub resolved_by: ResolutionLayer,
    pub content_summary: String,
}

/// Layer 4 (or any position): Logs all tile decisions with timestamps.
///
/// This layer always escalates — it never resolves. It's meant to be
/// inserted at any point in the chain to record decisions for audit.
pub struct LoggingLayer {
    pub entries: Mutex<Vec<LogEntry>>,
    pub max_entries: usize,
}

impl LoggingLayer {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Mutex::new(Vec::with_capacity(max_entries.min(1024))),
            max_entries,
        }
    }

    /// Read all log entries.
    pub fn get_entries(&self) -> Vec<LogEntry> {
        self.entries.lock().map(|e| e.clone()).unwrap_or_default()
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

impl Default for LoggingLayer {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl Layer for LoggingLayer {
    fn name(&self) -> &'static str {
        "logging"
    }

    fn process(&self, tile: Tile) -> LayerResult {
        let entry = LogEntry {
            timestamp_ms: Self::now_ms(),
            layer_name: self.name().to_string(),
            tile_id: tile.id,
            room_id: tile.room_id.clone(),
            confidence: tile.confidence,
            resolved_by: tile.resolved_by,
            content_summary: {
                let s = &tile.content;
                if s.len() > 80 {
                    s[..80].to_string()
                } else {
                    s.clone()
                }
            },
        };

        if let Ok(mut entries) = self.entries.lock() {
            if entries.len() >= self.max_entries {
                entries.remove(0);
            }
            entries.push(entry);
        }

        // Logging never resolves — always pass through
        LayerResult::Escalate(tile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{make_reading, tile_from_reading};

    #[test]
    fn logging_always_escalates() {
        let layer = LoggingLayer::new(100);
        let r = make_reading("x", 1.0, 0.0, 2.0);
        match layer.process(tile_from_reading(&r)) {
            LayerResult::Escalate(_) => {}
            LayerResult::Resolved(_) => panic!("Logging should always escalate"),
        }
    }

    #[test]
    fn logging_records_entry() {
        let layer = LoggingLayer::new(100);
        let r = make_reading("x", 1.0, 0.0, 2.0);
        layer.process(tile_from_reading(&r));
        let entries = layer.get_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].room_id, "test-room");
    }

    #[test]
    fn logging_respects_max_entries() {
        let layer = LoggingLayer::new(3);
        for i in 0..5 {
            let r = make_reading("x", i as f64, 0.0, 100.0);
            layer.process(tile_from_reading(&r));
        }
        let entries = layer.get_entries();
        assert_eq!(entries.len(), 3); // Evicts oldest
    }

    #[test]
    fn logging_truncates_long_content() {
        let layer = LoggingLayer::new(100);
        let mut tile = tile_from_reading(&make_reading("x", 1.0, 0.0, 2.0));
        tile.content = "x".repeat(200);
        layer.process(tile);
        let entries = layer.get_entries();
        assert_eq!(entries[0].content_summary.len(), 80);
    }
}
