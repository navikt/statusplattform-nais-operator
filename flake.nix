{
  description = "A Nix-flake based development interface for NAV's Statusplattform's K8s operator";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    # Rust compile stuff
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };

    # Rust 3rd party tooling
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = {self, ...} @ inputs:
    inputs.flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [(import inputs.rust-overlay)];
        };
        inherit (pkgs) lib;

        # Target musl when building on 64-bit linux to create statically linked binaries
        # Set-up build dependencies and configure rust for statically lined binaries
        CARGO_BUILD_TARGET =
          {
            # Insert other "<host archs> = <target archs>" at will
            "x86_64-linux" = "x86_64-unknown-linux-musl";
          }
          .${system}
          or (pkgs.rust.toRustTargetSpec pkgs.stdenv.hostPlatform);
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          targets = [
            CARGO_BUILD_TARGET
            (pkgs.rust.toRustTargetSpec pkgs.stdenv.hostPlatform)
          ];
        };
        craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;

        # Common vars
        cargoDetails = pkgs.lib.importTOML ./Cargo.toml;
        pname = cargoDetails.package.name;
        src = craneLib.cleanCargoSource (craneLib.path ./.);
        commonArgs = {
          inherit pname src CARGO_BUILD_TARGET;
          nativeBuildInputs = with pkgs;
            [pkg-config]
            ++ lib.optionals stdenv.isDarwin [
              darwin.apple_sdk.frameworks.Security
              darwin.apple_sdk.frameworks.SystemConfiguration
            ];
        };

        imageTag = "v${cargoDetails.package.version}-${dockerTag}";
        imageName = "${pname}:${imageTag}";
        teamName = "navdig";
        my-spec = import ./spec.nix {
          inherit
            lib
            teamName
            pname
            imageName
            ;
        };
        # Compile (and cache) cargo dependencies _only_
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        cargo-sbom = craneLib.mkCargoDerivation (
          commonArgs
          // {
            # Require the caller to specify cargoArtifacts we can use
            inherit cargoArtifacts;

            # A suffix name used by the derivation, useful for logging
            pnameSuffix = "-sbom";

            # Set the cargo command we will use and pass through the flags
            installPhase = "mv bom.json $out";
            buildPhaseCargoCommand = "cargo cyclonedx -f json --all --override-filename bom";
            nativeBuildInputs = (commonArgs.nativeBuildInputs or []) ++ [pkgs.cargo-cyclonedx];
          }
        );

        dockerTag =
          if lib.hasAttr "rev" self
          then "${builtins.toString self.revCount}-${self.shortRev}"
          else "gitDirty";

        # Compile workspace code (including 3rd party dependencies)
        cargo-package = craneLib.buildPackage (commonArgs // {inherit cargoArtifacts;});
      in {
        checks = {
          inherit cargo-package cargo-sbom;
          # Run clippy (and deny all warnings) on the crate source,
          # again, resuing the dependency artifacts from above.
          #
          # Note that this is done as a separate derivation so that
          # we can block the CI if there are issues here, but not
          # prevent downstream consumers from building our crate by itself.
          cargo-clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = lib.concatStringsSep " " [
                # "--all-targets"
                # "--"
                # "--deny warnings"
                # "-W"
                # "clippy::pedantic"
                # "-W"
                # "clippy::nursery"
                # "-W"
                # "clippy::unwrap_used"
                # "-W"
                # "clippy::expect_used"
              ];
            }
          );
          cargo-doc = craneLib.cargoDoc (commonArgs // {inherit cargoArtifacts;});
          cargo-fmt = craneLib.cargoFmt {inherit src;};
          cargo-audit = craneLib.cargoAudit {
            inherit (inputs) advisory-db;
            inherit src;
          };
          # Run tests with cargo-nextest
          # Consider setting `doCheck = false` on `cargo-package` if you do not want
          # the tests to run twice
          # cargo-nextest = craneLib.cargoNextest (commonArgs // {
          #   inherit cargoArtifacts;
          #   partitions = 1;
          #   partitionType = "count";
          # });
        };
        devShells.default = craneLib.devShell {
          packages = with pkgs;
            [
              cargo-audit
              cargo-auditable
              cargo-deny
              cargo-outdated
              cargo-cyclonedx
              cargo-watch

              # Editor stuffs
              helix
              lldb
              rust-analyzer
            ]
            ++ lib.optionals stdenv.isDarwin [
              darwin.apple_sdk.frameworks.Security
              darwin.apple_sdk.frameworks.SystemConfiguration
            ];

          shellHook = ''
            ${rustToolchain}/bin/cargo --version
            ${pkgs.helix}/bin/hx --health rust
          '';
        };

        packages = rec {
          default = rust;
          rust = cargo-package;
          sbom = cargo-sbom;
          image = docker;
          spec = let
            toJson = attrSet: builtins.toJSON attrSet;
            yamlContent = builtins.concatStringsSep ''

              ---
            '' (map toJson my-spec);
          in
            pkgs.writeText "spec.yaml" yamlContent;

          docker = pkgs.dockerTools.buildImage {
            name = pname;
            tag = imageTag;
            config.Entrypoint = ["${cargo-package}/bin/${pname}"];
          };
        };

        # Now `nix fmt` works!
        formatter = pkgs.alejandra;
      }
    );
}
