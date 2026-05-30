use std::collections::HashMap;

use serde::{Deserialize, Serialize};


/// Statistics for a signal chain run.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChainStats {
    pub total_processed: u64,
    pub resolved_count: u64,
    pub unresolved_count: u64,
    pub total_latency_ns: u64,
    pub resolution_by_layer: HashMap<String, u64>,
    pub last_resolution_layer: Option<String>,
}

impl ChainStats {
    /// Average latency per tile in microseconds.
    pub fn avg_latency_us(&self) -> f64 {
        if self.total_processed == 0 {
            return 0.0;
        }
        (self.total_latency_ns as f64 / 1000.0) / self.total_processed as f64
    }

    /// Fraction of tiles that were resolved (not escalated past all layers).
    pub fn resolution_rate(&self) -> f64 {
        if self.total_processed == 0 {
            return 0.0;
        }
        self.resolved_count as f64 / self.total_processed as f64
    }

    /// How many tiles resolved at each layer, as percentages.
    pub fn layer_percentages(&self) -> HashMap<String, f64> {
        if self.resolved_count == 0 {
            return HashMap::new();
        }
        self.resolution_by_layer
            .iter()
            .map(|(k, &v)| (k.clone(), v as f64 / self.resolved_count as f64 * 100.0))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_stats() {
        let s = ChainStats::default();
        assert_eq!(s.total_processed, 0);
        assert_eq!(s.avg_latency_us(), 0.0);
        assert_eq!(s.resolution_rate(), 0.0);
    }

    #[test]
    fn test_avg_latency() {
        let s = ChainStats {
            total_processed: 10,
            total_latency_ns: 10_000_000, // 10ms total
            ..Default::default()
        };
        assert!((s.avg_latency_us() - 1000.0).abs() < 0.1); // 1ms = 1000µs
    }

    #[test]
    fn test_resolution_rate() {
        let s = ChainStats {
            total_processed: 100,
            resolved_count: 80,
            unresolved_count: 20,
            ..Default::default()
        };
        assert!((s.resolution_rate() - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_layer_percentages_empty() {
        let s = ChainStats::default();
        assert!(s.layer_percentages().is_empty());
    }

    #[test]
    fn test_layer_percentages() {
        let mut map = HashMap::new();
        map.insert("deadband".to_string(), 60);
        map.insert("rules".to_string(), 30);
        map.insert("ollama".to_string(), 10);
        let s = ChainStats {
            total_processed: 100,
            resolved_count: 100,
            resolution_by_layer: map,
            ..Default::default()
        };
        let pct = s.layer_percentages();
        assert!((pct["deadband"] - 60.0).abs() < 0.1);
        assert!((pct["rules"] - 30.0).abs() < 0.1);
        assert!((pct["ollama"] - 10.0).abs() < 0.1);
    }
}
