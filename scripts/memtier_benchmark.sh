#!/usr/bin/env -S nix shell nixpkgs#memtier-benchmark --command bash

memtier_benchmark --server=localhost --port=6379 --clients=50 --threads=4 --requests=10000
