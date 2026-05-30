# plato-signal-chain

Composable 5-layer signal chain pipeline for PLATO rooms.

## What It Does

Provides the composable pipeline architecture that transforms sensor data through five progressive layers:

```
Sensor → Deadband Filter → Nano Model → Room LoRA → Fleet Coord → Cloud
         L0 (free)          L1 (350M)    L2 (LoRA)   L3 (1.2B)     L4
```

Each layer resolves what it can. Only the remainder escalates to the next layer.

## Ecosystem

- **[plato-nervous](https://github.com/SuperInstance/plato-nervous)** ← Depends on (core signal chain logic)
- **[plato-tiles](https://github.com/SuperInstance/plato-tiles)** ← Depends on (tile types)
- **[plato-state](https://github.com/SuperInstance/plato-state)** — State vectors inform routing
- **[plato-coordination](https://github.com/SuperInstance/plato-coordination)** — L3 fleet coordination layer
- **[plato-diffusion](https://github.com/SuperInstance/plato-diffusion)** — Progressive distillation for model training

See [DEPENDENCIES.md](./DEPENDENCIES.md) for the full dependency map.
