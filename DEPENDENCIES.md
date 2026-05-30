# DEPENDENCIES — plato-signal-chain

## Signal Chain Layer

**L1-L4 (Pipeline)** — Composable 5-layer signal chain pipeline.

The composable pipeline that routes and transforms tiles through the five PLATO signal chain layers: deadband filter → nano model → room LoRA → fleet coordinator → cloud.

## Ecosystem Dependencies

| Repo | Relationship | Description |
|------|-------------|-------------|
| [plato-nervous](https://github.com/SuperInstance/plato-nervous) | **Depends on** | Core nervous system types, distillation pipeline, model interfaces |
| [plato-tiles](https://github.com/SuperInstance/plato-tiles) | **Depends on** | Tile type definitions that flow through the pipeline |
| [plato-state](https://github.com/SuperInstance/plato-state) | **Related** | State vectors may inform pipeline routing decisions |
| [plato-coordination](https://github.com/SuperInstance/plato-coordination) | **Related** | L3 fleet coordination layer uses coordination primitives |

## Data Flow

```
IN:
  - Tiles from sensors (via plato-tiles)
  - Nervous system configuration (from plato-nervous)
  - Model responses at each layer

OUT:
  - Processed tiles after each pipeline stage
  - Resolution decisions (which layer handled it)
  - Pipeline metrics (throughput, latency, compression ratio)
```

## Dependency Graph Position

```
plato-tiles → plato-rooms → plato-state → plato-nervous
                                              ↓
                                    plato-signal-chain ← (this crate)
```
