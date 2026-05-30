use std::sync::Mutex;

use crate::{Layer, LayerResult, Tile, TileType, ResolutionLayer, SensorReading};

/// A condition that a rule checks against a sensor reading.
#[derive(Debug, Clone)]
pub enum RuleCondition {
    AboveThreshold { sensor_id: String, threshold: f64 },
    BelowThreshold { sensor_id: String, threshold: f64 },
    InRange { sensor_id: String, min: f64, max: f64 },
}

/// A single rule that produces an alert tile when its condition matches.
#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub condition: RuleCondition,
    pub tile_content: String,
}

impl Rule {
    pub fn evaluate(&self, reading: &SensorReading) -> bool {
        match &self.condition {
            RuleCondition::AboveThreshold { sensor_id, threshold } => {
                reading.sensor_id == *sensor_id && reading.value > *threshold
            }
            RuleCondition::BelowThreshold { sensor_id, threshold } => {
                reading.sensor_id == *sensor_id && reading.value < *threshold
            }
            RuleCondition::InRange { sensor_id, min, max } => {
                reading.sensor_id == *sensor_id
                    && reading.value >= *min
                    && reading.value <= *max
            }
        }
    }
}

/// Layer 1: Rule-based classification.
///
/// Checks sensor readings against a list of rules. If any rule matches,
/// resolves the tile as an Alert. Otherwise escalates.
pub struct RuleLayer {
    pub rules: Vec<Rule>,
    /// Track how many tiles each rule has matched.
    pub match_counts: Mutex<Vec<u64>>,
}

impl RuleLayer {
    pub fn new(rules: Vec<Rule>) -> Self {
        let count = rules.len();
        Self {
            rules,
            match_counts: Mutex::new(vec![0; count]),
        }
    }
}

impl Layer for RuleLayer {
    fn name(&self) -> &'static str {
        "rules"
    }

    fn process(&self, mut tile: Tile) -> LayerResult {
        let reading = match &tile.sensor_reading {
            Some(r) => r,
            None => return LayerResult::Escalate(tile),
        };

        for (i, rule) in self.rules.iter().enumerate() {
            if rule.evaluate(reading) {
                if let Ok(mut counts) = self.match_counts.lock() {
                    if i < counts.len() {
                        counts[i] += 1;
                    }
                }
                tile.tile_type = TileType::Alert;
                tile.content = rule.tile_content.clone();
                tile.confidence = 1.0;
                tile.resolved_by = ResolutionLayer::Rules;
                return LayerResult::Resolved(tile);
            }
        }

        LayerResult::Escalate(tile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{make_reading, tile_from_reading};

    fn high_coolant_rule() -> Rule {
        Rule {
            name: "high_coolant".into(),
            condition: RuleCondition::AboveThreshold {
                sensor_id: "coolant".into(),
                threshold: 210.0,
            },
            tile_content: "Coolant above 210F!".into(),
        }
    }

    fn low_oil_rule() -> Rule {
        Rule {
            name: "low_oil".into(),
            condition: RuleCondition::BelowThreshold {
                sensor_id: "oil".into(),
                threshold: 35.0,
            },
            tile_content: "Oil below 35 PSI!".into(),
        }
    }

    #[test]
    fn rule_matches_above_threshold() {
        let layer = RuleLayer::new(vec![high_coolant_rule()]);
        let r = make_reading("coolant", 215.0, 140.0, 220.0);
        match layer.process(tile_from_reading(&r)) {
            LayerResult::Resolved(t) => {
                assert_eq!(t.tile_type, TileType::Alert);
                assert_eq!(t.resolved_by, ResolutionLayer::Rules);
            }
            LayerResult::Escalate(_) => panic!("Should match rule"),
        }
    }

    #[test]
    fn rule_no_match_below_threshold() {
        let layer = RuleLayer::new(vec![high_coolant_rule()]);
        let r = make_reading("coolant", 195.0, 140.0, 210.0);
        match layer.process(tile_from_reading(&r)) {
            LayerResult::Escalate(_) => {}
            LayerResult::Resolved(_) => panic!("195 < 210, should not match"),
        }
    }

    #[test]
    fn rule_matches_below_threshold() {
        let layer = RuleLayer::new(vec![low_oil_rule()]);
        let r = make_reading("oil", 28.0, 35.0, 80.0);
        match layer.process(tile_from_reading(&r)) {
            LayerResult::Resolved(t) => {
                assert!(t.content.contains("Oil"));
            }
            LayerResult::Escalate(_) => panic!("28 < 35, should match"),
        }
    }

    #[test]
    fn rule_wrong_sensor_id() {
        let layer = RuleLayer::new(vec![high_coolant_rule()]);
        let r = make_reading("rpm", 215.0, 140.0, 220.0); // value exceeds but wrong sensor
        match layer.process(tile_from_reading(&r)) {
            LayerResult::Escalate(_) => {}
            LayerResult::Resolved(_) => panic!("Wrong sensor, should not match"),
        }
    }

    #[test]
    fn rule_match_counts_tracked() {
        let layer = RuleLayer::new(vec![high_coolant_rule(), low_oil_rule()]);
        let r1 = make_reading("coolant", 215.0, 140.0, 220.0);
        let r2 = make_reading("coolant", 220.0, 140.0, 220.0);
        layer.process(tile_from_reading(&r1));
        layer.process(tile_from_reading(&r2));

        let counts = layer.match_counts.lock().unwrap();
        assert_eq!(counts[0], 2);
        assert_eq!(counts[1], 0);
    }

    #[test]
    fn rule_in_range_condition() {
        let rule = Rule {
            name: "normal_coolant".into(),
            condition: RuleCondition::InRange {
                sensor_id: "coolant".into(),
                min: 180.0,
                max: 210.0,
            },
            tile_content: "Coolant in normal range".into(),
        };
        let layer = RuleLayer::new(vec![rule]);

        let r = make_reading("coolant", 195.0, 140.0, 220.0);
        match layer.process(tile_from_reading(&r)) {
            LayerResult::Resolved(_) => {}
            LayerResult::Escalate(_) => panic!("195 is in [180, 210]"),
        }

        let r2 = make_reading("coolant", 170.0, 140.0, 220.0);
        match layer.process(tile_from_reading(&r2)) {
            LayerResult::Escalate(_) => {}
            LayerResult::Resolved(_) => panic!("170 is not in [180, 210]"),
        }
    }
}
