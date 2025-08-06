# dashdotcache
Simple in-memory cache in Rust.

**Status: Outline**

## Targets
- [x] Basic concurrent cache
- [x] Cache expiry
- [x] Parent relationships & cascading invalidation
- [x] Basic HTTP API
- [x] Prometheus-compatible metrics
- [x] Environment setup
- [ ] Full demo feature set
- [ ] Redux-like web UI
- [ ] RESP API
- [ ] Reasonable subset of redis commands
- [ ] Cache performance optimizations
- [ ] Cache memory optimization
- [ ] Bloom filters

## Quick Start
With Nix installed:
```bash
nix develop
nix build
nix run
# HTTP API: http://localhost:8080
# RESP API: localhost:6379
```

## Development
### CI checks
From within a `nix develop` environment
`check`

This checks linting, formatting, licenses and dependencies via `nix flake check`, with tidied output.

### Tests
```
cargo nextest run
```

### Benchmarks
`cargo bench`

