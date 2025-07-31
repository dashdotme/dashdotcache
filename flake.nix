{
  description = "Fast, concurrent cache built in Rust.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";

    # Nix -> Rust integration, for cached & modular builds
    crane.url = "github:ipetkov/crane";

    # generates configurations for all system targets
    flake-utils.url = "github:numtide/flake-utils";

    # security scanner for cargo dependencies
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false; # rustsec does not use nix
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      advisory-db,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        inherit (pkgs) lib;

        craneLib = crane.mkLib pkgs;
        src = craneLib.cleanCargoSource ./.;

        # Common arguments can be set here to avoid repeating them later
        commonArgs = {
          inherit src;
          strictDeps = true;

          buildInputs =
            [
              # Build time dependencies
            ]
            ++ lib.optionals pkgs.stdenv.isDarwin [
              # Additional darwin specific inputs can be set here
              pkgs.libiconv
            ];
        };

        # Build cargo dependencies separately, so they can be cached
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Build the crate itself
        dashdotcache = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            # TODO: add test instructions to README: `nix flake check`
            doCheck = false; # disable rust tests during builds
          }
        );
      in
      {
        checks = {
          # Build the crate as part of `nix flake check` for convenience
          inherit dashdotcache;

          # Linting check with clippy
          dashdotcache-clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          # Docstring checks
          dashdotcache-doc = craneLib.cargoDoc (
            commonArgs
            // {
              inherit cargoArtifacts;
              # turn docstring warnings into linting errors
              env.RUSTDOCFLAGS = "--deny warnings";
            }
          );

          # Formatting check
          dashdotcache-fmt = craneLib.cargoFmt {
            inherit src;
          };

          dashdotcache-toml-fmt = craneLib.taploFmt {
            src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
            taploExtraArgs = "--config ./taplo.toml";
          };

          # Audit dependencies (vulnerability checks)
          dashdotcache-audit = craneLib.cargoAudit {
            inherit src advisory-db;
          };

          # Audit licenses (see deny.toml)
          dashdotcache-deny = craneLib.cargoDeny {
            inherit src;
          };

          # Run tests with cargo-nextest (faster, better UX)
          dashdotcache-nextest = craneLib.cargoNextest (
            commonArgs
            // {
              inherit cargoArtifacts;
              partitions = 1;
              partitionType = "count";
              cargoNextestPartitionsExtraArgs = "--no-tests=pass";
            }
          );
        };

        packages = {
          default = dashdotcache;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = dashdotcache;
        };

        devShells.default = craneLib.devShell {
          # Inherit inputs from checks.
          checks = self.checks.${system};

          packages = with pkgs; [
            # cargo and rustc provided by crane
            taplo # toml language server
            pkg-config # nix -> C dependency glue
            rust-analyzer
          ];
        };
      }
    );
}
