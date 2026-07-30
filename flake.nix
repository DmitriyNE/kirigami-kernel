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

        # Pinned Rust toolchain, driven by rust-toolchain.toml so the SAME file
        # is the single source of truth for Nix and non-Nix (rustup) users —
        # `nix develop`'s rustc == the file's pin (a vv-guide §8 M0 criterion).
        # The file also lists the `thumbv7em-none-eabi` no_std gate target, which
        # fromToolchainFile fetches. If the file changes, `sha256` must be
        # updated: set it to lib.fakeSha256, run `nix develop`, paste the hash
        # the error reports.
        rustToolchain = fenix.packages.${system}.fromToolchainFile {
          file = ./rust-toolchain.toml;
          sha256 = "sha256-mvUGEOHYJpn3ikC5hckneuGixaC+yGrkMM/liDIDgoU=";
        };
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
            pkgs.gcc # CBMC/goto-cc (Kani) preprocesses its C intrinsics with `gcc`
            pkgs.z3 # SMT solver for Kani (`--solver z3`); native bitvector theory
            # is far faster than cadical's bit-blasting on the 128-bit gcd loop.

            pkgs.git # so `git` works inside `nix develop` (CI == local)
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
