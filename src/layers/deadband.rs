use crate::{Layer, LayerResult, Tile, TileType, ResolutionLayer};

/// Layer 0: Deadband filter.
///
/// Resolves tiles whose sensor readings are within normal range
/// and within a configurable deadband of the previous value.
/// Escalates readings that are out of range or have drifted too much.
pub struct DeadbandLayer {
    /// Maximum drift from previous value to still be considered "normal".
    pub deadband: f64,
    /// Last value seen (None = first reading).
    pub last_value: std::sync::Mutex<Option<f64>>,
}

impl DeadbandLayer {
    pub fn new(deadband: f64) -> Self {
        Self {
            deadband,
            last_value: std::sync::Mutex::new(None),
        }
    }
}

impl Layer for DeadbandLayer {
    fn name(&self) -> &'static str {
        "deadband"
    }

    fn process(&self, mut tile: Tile) -> LayerResult {
        let reading = match &tile.sensor_reading {
            Some(r) => r,
            None => return LayerResult::Escalate(tile),
        };

        let in_range = reading.value >= reading.normal_min && reading.value <= reading.normal_max;

        let mut last = self.last_value.lock().unwrap();
        match *last {
            Some(prev) => {
                let drift = (reading.value - prev).abs();
                let in_deadband = drift <= self.deadband;

                if in_range && in_deadband {
                    *last = Some(reading.value);
                    tile.tile_type = TileType::Status;
                    tile.content = format!(
                        "{}: {:.1}{} (normal, drift {:.2})",
                        reading.sensor_id, reading.value, reading.unit, drift
                    );
                    tile.confidence = 1.0;
                    tile.resolved_by = ResolutionLayer::Algorithmic;
                    LayerResult::Resolved(tile)
                } else {
                    *last = Some(reading.value);
                    LayerResult::Escalate(tile)
                }
            }
            None => {
                *last = Some(reading.value);
                if in_range {
                    tile.tile_type = TileType::Status;
                    tile.content = format!(
                        "{}: {:.1}{} (initial reading, normal)",
                        reading.sensor_id, reading.value, reading.unit
                    );
                    tile.confidence = 1.0;
                    tile.resolved_by = ResolutionLayer::Algorithmic;
                    LayerResult::Resolved(tile)
                } else {
                    LayerResult::Escalate(tile)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{make_reading, tile_from_reading};

    #[test]
    fn deadband_resolves_normal_first_reading() {
        let layer = DeadbandLayer::new(5.0);
        let r = make_reading("rpm", 1450.0, 1400.0, 1500.0);
        let tile = tile_from_reading(&r);
        match layer.process(tile) {
            LayerResult::Resolved(t) => {
                assert_eq!(t.resolved_by, ResolutionLayer::Algorithmic);
            }
            LayerResult::Escalate(_) => panic!("Should resolve normal reading"),
        }
    }

    #[test]
    fn deadband_resolves_small_drift() {
        let layer = DeadbandLayer::new(5.0);
        let r1 = make_reading("rpm", 1450.0, 1400.0, 1500.0);
        let t1 = tile_from_reading(&r1);
        layer.process(t1);

        let r2 = make_reading("rpm", 1453.0, 1400.0, 1500.0);
        let t2 = tile_from_reading(&r2);
        match layer.process(t2) {
            LayerResult::Resolved(_) => {}
            LayerResult::Escalate(_) => panic!("3.0 drift < 5.0 deadband, should resolve"),
        }
    }

    #[test]
    fn deadband_escalates_large_drift() {
        let layer = DeadbandLayer::new(5.0);
        let r1 = make_reading("rpm", 1450.0, 1400.0, 1500.0);
        layer.process(tile_from_reading(&r1));

        let r2 = make_reading("rpm", 1460.0, 1400.0, 1500.0);
        match layer.process(tile_from_reading(&r2)) {
            LayerResult::Escalate(_) => {}
            LayerResult::Resolved(_) => panic!("10.0 drift > 5.0 deadband, should escalate"),
        }
    }

    #[test]
    fn deadband_escalates_out_of_range() {
        let layer = DeadbandLayer::new(50.0);
        let r = make_reading("coolant", 228.0, 140.0, 210.0);
        match layer.process(tile_from_reading(&r)) {
            LayerResult::Escalate(_) => {}
            LayerResult::Resolved(_) => panic!("228 > 210 max, should escalate"),
        }
    }

    #[test]
    fn deadband_no_sensor_reading() {
        let layer = DeadbandLayer::new(5.0);
        let tile = Tile {
            id: uuid::Uuid::new_v4(),
            room_id: "r".into(),
            tile_type: TileType::Status,
            content: "no sensor".into(),
            confidence: 0.0,
            resolved_by: ResolutionLayer::Unresolved,
            timestamp_ms: 0,
            sensor_reading: None,
        };
        match layer.process(tile) {
            LayerResult::Escalate(_) => {}
            LayerResult::Resolved(_) => panic!("No sensor reading → escalate"),
        }
    }
}
