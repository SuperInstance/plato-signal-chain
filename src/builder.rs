use crate::{
    DeadbandLayer, NoopLayer, RuleLayer, LoggingLayer, CloudLayer, Rule, SignalChain,
};


/// Fluent builder for constructing a `SignalChain`.
///
/// # Example
/// ```
/// use plato_signal_chain::SignalChainBuilder;
///
/// let chain = SignalChainBuilder::new()
///     .deadband(5.0)
///     .rules(vec![])
///     .logging(1000)
///     .cloud(true)
///     .build();
///
/// let tile = chain.process(plato_signal_chain::tile_from_reading(
///     &plato_signal_chain::make_reading("rpm", 1450.0, 1400.0, 1500.0),
/// ));
/// assert!(tile.resolved_at.is_some());
/// ```
pub struct SignalChainBuilder {
    deadband: Option<f64>,
    rules: Vec<Rule>,
    #[allow(dead_code)]
    use_ollama: bool,
    #[allow(dead_code)]
    ollama_endpoint: String,
    #[allow(dead_code)]
    ollama_model: String,
    #[allow(dead_code)]
    ollama_prompt: String,
    use_cloud: bool,
    log_max: usize,
}

impl Default for SignalChainBuilder {
    fn default() -> Self {
        Self {
            deadband: None,
            rules: Vec::new(),
            use_ollama: false,
            ollama_endpoint: "http://localhost:11434".into(),
            ollama_model: "llama3".into(),
            ollama_prompt: "Analyze sensor {sensor_id}={value}{unit}".into(),
            use_cloud: true,
            log_max: 0,
        }
    }
}

impl SignalChainBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set deadband threshold for L0.
    pub fn deadband(mut self, threshold: f64) -> Self {
        self.deadband = Some(threshold);
        self
    }

    /// Set rules for L1.
    pub fn rules(mut self, rules: Vec<Rule>) -> Self {
        self.rules = rules;
        self
    }

    /// Enable logging layer at L4 with the given max entries.
    pub fn logging(mut self, max_entries: usize) -> Self {
        self.log_max = max_entries;
        self
    }

    /// Enable or disable cloud fallback at L3.
    pub fn cloud(mut self, available: bool) -> Self {
        self.use_cloud = available;
        self
    }

    /// Build the signal chain with concrete layer types.
    ///
    /// Returns a chain with: DeadbandLayer, RuleLayer, NoopLayer, CloudLayer, LoggingLayer/NoopLayer.
    pub fn build(
        self,
    ) -> SignalChain<DeadbandLayer, RuleLayer, NoopLayer, CloudLayer, LoggingLayer> {
        let l0 = match self.deadband {
            Some(d) => DeadbandLayer::new(d),
            None => DeadbandLayer::new(f64::MAX), // Effectively pass-through
        };

        let l1 = RuleLayer::new(self.rules);

        let l2 = NoopLayer::new("noop-l2");

        let l3 = if self.use_cloud {
            CloudLayer::new()
        } else {
            CloudLayer::unavailable()
        };

        let l4 = if self.log_max > 0 {
            LoggingLayer::new(self.log_max)
        } else {
            LoggingLayer::new(100) // Default small buffer
        };

        SignalChain::new(l0, l1, l2, l3, l4)
    }
}

/// Build a chain where all layers are Noop (everything escalates).
/// Useful for testing the escalation path.
pub fn all_noop_chain() -> SignalChain<NoopLayer, NoopLayer, NoopLayer, NoopLayer, NoopLayer> {
    SignalChain::new(
        NoopLayer::new("noop-0"),
        NoopLayer::new("noop-1"),
        NoopLayer::new("noop-2"),
        NoopLayer::new("noop-3"),
        NoopLayer::new("noop-4"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{make_reading, tile_from_reading, RuleCondition, ResolutionLayer};

    #[test]
    fn builder_default_builds_chain() {
        let chain = SignalChainBuilder::new()
            .deadband(5.0)
            .rules(vec![])
            .cloud(true)
            .logging(100)
            .build();

        let r = make_reading("rpm", 1450.0, 1400.0, 1500.0);
        let result = chain.process(tile_from_reading(&r));
        assert!(result.resolved_at.is_some());
        assert_eq!(result.resolved_at.unwrap().0, 0); // Resolved at L0
    }

    #[test]
    fn builder_chain_with_rule() {
        let chain = SignalChainBuilder::new()
            .deadband(5.0)
            .rules(vec![Rule {
                name: "high_coolant".into(),
                condition: RuleCondition::AboveThreshold {
                    sensor_id: "coolant".into(),
                    threshold: 210.0,
                },
                tile_content: "Coolant too hot!".into(),
            }])
            .cloud(true)
            .logging(100)
            .build();

        // Out of deadband → escalate to rules → rule matches
        let r = make_reading("coolant", 215.0, 140.0, 220.0);
        let result = chain.process(tile_from_reading(&r));
        assert!(result.resolved_at.is_some());
    }

    #[test]
    fn builder_all_escalate_reaches_cloud() {
        let chain = SignalChainBuilder::new()
            .deadband(0.0) // Tiny deadband → most readings escalate
            .rules(vec![])  // No rules
            .cloud(true)
            .logging(100)
            .build();

        // First reading passes deadband (initial), but drift will escalate
        let r1 = make_reading("x", 50.0, 0.0, 100.0);
        chain.process(tile_from_reading(&r1));

        let r2 = make_reading("x", 80.0, 0.0, 100.0); // Large drift
        let result = chain.process(tile_from_reading(&r2));
        assert!(result.resolved_at.is_some());
    }

    #[test]
    fn all_noop_chain_escalates_everything() {
        let chain = all_noop_chain();
        let r = make_reading("x", 1.0, 0.0, 2.0);
        let result = chain.process(tile_from_reading(&r));
        assert!(result.resolved_at.is_none()); // Nothing resolves
    }

    #[test]
    fn builder_stats_tracked() {
        let chain = SignalChainBuilder::new()
            .deadband(50.0)
            .rules(vec![])
            .cloud(true)
            .logging(100)
            .build();

        for i in 0..10 {
            let r = make_reading("x", 50.0 + i as f64, 0.0, 100.0);
            chain.process(tile_from_reading(&r));
        }

        let stats = chain.get_stats();
        assert_eq!(stats.total_processed, 10);
        assert!(stats.resolved_count > 0);
    }

    #[test]
    fn builder_cloud_unavailable() {
        let chain = SignalChainBuilder::new()
            .deadband(0.0)
            .rules(vec![])
            .cloud(false)
            .logging(100)
            .build();

        // First reading passes deadband (initial)
        let r1 = make_reading("x", 50.0, 0.0, 100.0);
        chain.process(tile_from_reading(&r1));

        // Second reading with drift → escalates through all layers → cloud unavailable → unresolved
        let r2 = make_reading("x", 80.0, 0.0, 100.0);
        let result = chain.process(tile_from_reading(&r2));
        // Cloud unavailable means it escalates past cloud too, logging escalates → unresolved
        assert!(result.resolved_at.is_none() || result.tile.resolved_by == ResolutionLayer::Algorithmic);
    }
}
