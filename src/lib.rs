//! Plato Signal Chain — Composable 5-Layer Pipeline
//!
//! Each sensor reading flows through up to 5 resolution layers:
//!   L0: Deadband (algorithmic thresholds)
//!   L1: Rules (condition-based classification)
//!   L2: Ollama (local LLM inference, behind feature flag)
//!   L3: Cloud (remote API, placeholder)
//!   L4: Logging (always runs, records decisions)
//!
//! Layers that resolve early stop the chain. Layers that can't handle
//! a reading escalate to the next layer.

pub mod builder;
pub mod layers;
pub mod stats;

pub use builder::SignalChainBuilder;
pub use layers::*;
pub use stats::ChainStats;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Re-export core PLATO types (self-contained, no plato-nervous dep) ─

/// A sensor reading from a room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    pub sensor_id: String,
    pub room_id: String,
    pub value: f64,
    pub unit: String,
    pub timestamp_ms: u64,
    pub normal_min: f64,
    pub normal_max: f64,
}

/// The fundamental unit of processed information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tile {
    pub id: Uuid,
    pub room_id: String,
    pub tile_type: TileType,
    pub content: String,
    pub confidence: f64,
    pub resolved_by: ResolutionLayer,
    pub timestamp_ms: u64,
    pub sensor_reading: Option<SensorReading>,
}

/// What kind of tile this is.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TileType {
    Status,
    Alert,
    Prediction,
    Anomaly,
    Coordination,
    Escalation,
}

/// Which layer resolved this tile.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ResolutionLayer {
    Algorithmic,
    Rules,
    Ollama,
    Cloud,
    Unresolved,
}

// ── Layer Trait ────────────────────────────────────────────────────────

/// Result of processing a tile through a layer.
#[derive(Debug, Clone)]
pub enum LayerResult {
    /// Layer handled the tile — chain stops here.
    Resolved(Tile),
    /// Layer cannot handle — pass to next layer.
    Escalate(Tile),
}

/// A single processing layer in the signal chain.
pub trait Layer: Send + Sync {
    /// Human-readable name for this layer.
    fn name(&self) -> &'static str;

    /// Process a tile. Return Resolved if this layer handled it,
    /// or Escalate to pass it to the next layer.
    fn process(&self, tile: Tile) -> LayerResult;
}

// ── Signal Chain ───────────────────────────────────────────────────────

/// A composable 5-layer signal processing pipeline.
///
/// Layers are executed in order. The first layer to return `Resolved`
/// stops the chain. If all layers escalate, the tile comes out unresolved.
pub struct SignalChain<L0, L1, L2, L3, L4>
where
    L0: Layer,
    L1: Layer,
    L2: Layer,
    L3: Layer,
    L4: Layer,
{
    pub l0: L0,
    pub l1: L1,
    pub l2: L2,
    pub l3: L3,
    pub l4: L4,
    pub stats: std::sync::Mutex<ChainStats>,
}

impl<L0, L1, L2, L3, L4> SignalChain<L0, L1, L2, L3, L4>
where
    L0: Layer,
    L1: Layer,
    L2: Layer,
    L3: Layer,
    L4: Layer,
{
    /// Create a new chain from 5 layers.
    pub fn new(l0: L0, l1: L1, l2: L2, l3: L3, l4: L4) -> Self {
        Self {
            l0,
            l1,
            l2,
            l3,
            l4,
            stats: std::sync::Mutex::new(ChainStats::default()),
        }
    }

    /// Process a tile through the chain.
    pub fn process(&self, tile: Tile) -> ChainResult {
        let start = std::time::Instant::now();
        let layers: [&dyn Layer; 5] = [&self.l0, &self.l1, &self.l2, &self.l3, &self.l4];
        let layer_names: [&str; 5] = [self.l0.name(), self.l1.name(), self.l2.name(), self.l3.name(), self.l4.name()];

        let mut current = tile;
        let mut resolved_at: Option<(usize, &'static str)> = None;

        for (i, layer) in layers.iter().enumerate() {
            match layer.process(current) {
                LayerResult::Resolved(t) => {
                    resolved_at = Some((i, layer_names[i]));
                    current = t;
                    break;
                }
                LayerResult::Escalate(t) => {
                    current = t;
                }
            }
        }

        let elapsed = start.elapsed();

        // Update stats
        if let Ok(mut stats) = self.stats.lock() {
            stats.total_processed += 1;
            stats.total_latency_ns += elapsed.as_nanos() as u64;
            match resolved_at {
                Some((idx, name)) => {
                    stats.resolved_count += 1;
                    *stats.resolution_by_layer.entry(name.to_string()).or_insert(0) += 1;
                    stats.last_resolution_layer = Some(format!("L{}: {}", idx, name));
                }
                None => {
                    stats.unresolved_count += 1;
                    stats.last_resolution_layer = None;
                }
            }
        }

        ChainResult {
            tile: current,
            resolved_at: resolved_at.map(|(i, n)| (i, n.to_string())),
            latency: elapsed,
        }
    }

    /// Get a snapshot of the chain statistics.
    pub fn get_stats(&self) -> ChainStats {
        self.stats.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

/// Result of running a tile through the chain.
#[derive(Debug)]
pub struct ChainResult {
    pub tile: Tile,
    pub resolved_at: Option<(usize, String)>,
    pub latency: std::time::Duration,
}

// ── Helper ─────────────────────────────────────────────────────────────

/// Create a sensor reading for testing.
pub fn make_reading(sensor_id: &str, value: f64, min: f64, max: f64) -> SensorReading {
    SensorReading {
        sensor_id: sensor_id.to_string(),
        room_id: "test-room".to_string(),
        value,
        unit: "units".to_string(),
        timestamp_ms: 1000,
        normal_min: min,
        normal_max: max,
    }
}

/// Create a tile from a sensor reading (initial, unresolved).
pub fn tile_from_reading(reading: &SensorReading) -> Tile {
    Tile {
        id: Uuid::new_v4(),
        room_id: reading.room_id.clone(),
        tile_type: TileType::Status,
        content: format!("{}: {:.1}{}", reading.sensor_id, reading.value, reading.unit),
        confidence: 0.0,
        resolved_by: ResolutionLayer::Unresolved,
        timestamp_ms: reading.timestamp_ms,
        sensor_reading: Some(reading.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_reading() {
        let r = make_reading("temp", 72.0, 60.0, 80.0);
        assert_eq!(r.sensor_id, "temp");
        assert_eq!(r.value, 72.0);
    }

    #[test]
    fn test_tile_from_reading() {
        let r = make_reading("rpm", 1450.0, 1400.0, 1500.0);
        let t = tile_from_reading(&r);
        assert_eq!(t.resolved_by, ResolutionLayer::Unresolved);
        assert!(t.sensor_reading.is_some());
    }

    #[test]
    fn test_layer_result_variants() {
        let r = make_reading("x", 1.0, 0.0, 2.0);
        let t = tile_from_reading(&r);
        let _resolved = LayerResult::Resolved(t.clone());
        let _escalate = LayerResult::Escalate(t);
    }
}
