{
  description = "Vite plugin for processing Mog files";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
    playwright.url = "github:pietdevries94/playwright-web-flake";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane, pre-commit-hooks, treefmt-nix, playwright }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [
          (import rust-overlay)
          (final: prev: { inherit (playwright.packages.${system}) playwright-driver; })
        ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchainToml = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        rustToolchain = rustToolchainToml.override {
          extensions = [ "rust-src" ];
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # chromium only — the e2e tests launch nothing else
        playwright-browsers = pkgs.playwright-driver.browsers.override {
          withFirefox = false;
          withWebkit = false;
        };

        # Common args for crane
        commonArgs = {
          src = ./.;
          buildInputs = with pkgs; [
            # Add any system dependencies here
          ];
        };

        # Build dependencies first for reuse
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;


        pre-commit-check = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          hooks = {
            nixpkgs-fmt.enable = true;
          };
        };

        treefmtEval = treefmt-nix.lib.evalModule pkgs {
          projectRootFile = "flake.nix";
          programs = {
            prettier = {
              enable = true;
              includes = [ "*.js" "*.ts" "*.json" "*.md" "*.yaml" "*.yml" ];
              excludes = [ "pnpm-lock.yaml" ];
            };
            rustfmt = {
              enable = true;
              package = rustToolchain;
            };
            nixpkgs-fmt.enable = true;
          };
        };
      in
      {
        formatter = treefmtEval.config.build.wrapper;

        checks = {
          pre-commit-check = pre-commit-check;

          # Crane-based Rust checks that handle dependencies properly
          clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "-- --deny warnings";
          });

          rust-tests = craneLib.cargoTest (commonArgs // {
            inherit cargoArtifacts;
          });

          formatting = treefmtEval.config.build.check self;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Node
            nodejs_24
            pnpm

            # Rust
            rustToolchain

            # Formatters
            treefmtEval.config.build.wrapper
            nixpkgs-fmt

            git
            direnv
          ] ++ pre-commit-check.enabledPackages;

          shellHook = ''
            export PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1
            export PLAYWRIGHT_BROWSERS_PATH="${playwright-browsers}"

            ${pre-commit-check.shellHook}
          '';
        };
      });
}

