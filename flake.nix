{
  description = "Kirigami kernel — reproducible dev environment (certified-exact geometry for formed flexible-PCB substrates)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # Rust→Lean extraction toolchain. Pinned at the vv-guide §7 spike, at
    # mutually-compatible revs (Charon and Aeneas are co-versioned; the Lean pin
    # is downstream of Aeneas). See docs/environment-and-crate-layout.md §2/§3/§6.
    # Added then as inputs and composed into the devShell, e.g.:
    #   hax    = { url = "github:cryspen/hax/<rev>";           inputs.nixpkgs.follows = "nixpkgs"; };
    #   charon = { url = "github:AeneasVerif/charon/<rev>";    inputs.nixpkgs.follows = "nixpkgs"; };
    #   aeneas = { url = "github:AeneasVerif/aeneas/<rev>";    inputs.nixpkgs.follows = "nixpkgs"; };
    # Fallback if any is flaky to build from source: vendor its release binary.
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      fenix,
      ...
    }:
    flake-utils.lib.eachSystem [ "aarch64-darwin" "x86_64-linux" ] (
      system:
      let
        pkgs = import nixpkgs { inherit system; };

        # Pinned Rust toolchain. `stable` is fixed by the `fenix` input rev in
        # flake.lock, so this is reproducible; rust-toolchain.toml separately
        # pins the exact version for non-Nix users. To pin the exact version in
        # Nix too, switch to:
        #   fenix.packages.${system}.fromToolchainFile {
        #     file = ./rust-toolchain.toml; sha256 = "<paste from first eval>"; }
        rustToolchain = fenix.packages.${system}.stable.withComponents [
          "cargo"
          "clippy"
          "rustc"
          "rustfmt"
          "rust-src"
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          name = "kirigami";
          packages = [
            rustToolchain
            pkgs.cargo-fuzz

            # Kani (bounded model checking). If absent from the pinned nixpkgs,
            # install in-shell: `cargo install --locked kani-verifier && cargo-kani setup`.
            # pkgs.kani

            pkgs.elan # manages Lean + lake from certify-check/lean-toolchain

            # Differential oracles (difftest/) + FFI build tooling.
            pkgs.cgal
            pkgs.opencascade-occt
            pkgs.gmp
            pkgs.mpfr
            pkgs.boost
            pkgs.cmake
            pkgs.pkg-config
            pkgs.clang

            pkgs.jq
          ];

          shellHook = ''
            echo "kirigami dev shell:  $(rustc --version 2>/dev/null || echo 'rust not on PATH')"
            echo "  extraction toolchain (hax/charon/aeneas) + Lean/Mathlib pins land at the §7 spike"
          '';
        };
      }
    );
}
