use crate::{Layer, LayerResult, Tile};

/// A pass-through layer that always escalates. Useful as a placeholder
/// for layers not yet configured.
pub struct NoopLayer {
    pub label: &'static str,
}

impl NoopLayer {
    pub fn new(label: &'static str) -> Self {
        Self { label }
    }
}

impl Default for NoopLayer {
    fn default() -> Self {
        Self::new("noop")
    }
}

impl Layer for NoopLayer {
    fn name(&self) -> &'static str {
        self.label
    }

    fn process(&self, tile: Tile) -> LayerResult {
        LayerResult::Escalate(tile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{make_reading, tile_from_reading};

    #[test]
    fn noop_always_escalates() {
        let layer = NoopLayer::new("test");
        let r = make_reading("x", 1.0, 0.0, 2.0);
        match layer.process(tile_from_reading(&r)) {
            LayerResult::Escalate(_) => {}
            LayerResult::Resolved(_) => panic!("Noop should always escalate"),
        }
    }

    #[test]
    fn noop_name() {
        let layer = NoopLayer::new("my-noop");
        assert_eq!(layer.name(), "my-noop");
    }
}
