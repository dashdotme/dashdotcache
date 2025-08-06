{
  description = "Fast, concurrent cache built in Rust.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";

    # Nix -> Rust integration, for cached & modular builds
    crane.url = "github:ipetkov/crane";

    # Generates configurations for all system targets
    flake-utils.url = "github:numtide/flake-utils";

    # Security scanner for cargo dependencies
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    { self
    , nixpkgs
    , crane
    , flake-utils
    , advisory-db
    , ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        inherit (pkgs) lib;

        craneLib = crane.mkLib pkgs;
        src = craneLib.cleanCargoSource ./.;

        # Repeated args
        commonArgs = {
          inherit src;
          strictDeps = true;

          buildInputs =
            [
            ]
            ++ lib.optionals pkgs.stdenv.isDarwin [
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
            doCheck = false; # disable rust tests during builds
          }
        );
      in
      {
        checks = {
          # Build the crate as part of `nix flake check` for convenience
          inherit dashdotcache;

          # Linting
          dashdotcache-clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              # TODO: disallow unused variables at demo completion
              cargoClippyExtraArgs = "--all-targets -- --deny warnings --allow unused-variables";
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
            (writeScriptBin "check" ''
              #!/usr/bin/env bash
              echo -e "\033[35mRunning all checks...\033[0m"

              if nix flake check "$@"; then
                  echo ""
                  echo -e "\033[32mALL CHECKS PASSED SUCCESSFULLY!\033[0m"
                  echo -e "\033[32mLinting, formatting, tests, and security checks complete\033[0m"
              else
                  echo ""
                  echo -e "\033[31mChecks failed - see output above for details\033[0m"
              fi
            '')
          ];

          shellHook = ''
            echo "Run 'check' to do CI checks"
          '';
        };
      }
    );
}
