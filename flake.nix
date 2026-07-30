{
  description = "Kirigami kernel — reproducible dev environment (certified-exact geometry for formed flexible-PCB substrates)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # Rust→Lean extraction toolchain — pins resolved by the vv-guide §7 spike
    # (see docs/spike-extraction-report.md §1). Mutually compatible: Aeneas
    # 3a8586fa co-versions Charon 527ea8e and targets Lean v4.31.0; the Aeneas
    # package's bin/ provides BOTH `aeneas` and a co-versioned `charon`, so no
    # separate charon input is needed. hax 5b0ba8be provides `cargo-hax`.
    # These bring their own toolchains/nixpkgs (not forced to follow ours), which
    # is why they are pinned by rev rather than by a shared nixpkgs.
    aeneas.url = "github:AeneasVerif/aeneas/3a8586facab25b31bdb1e1f5f45acd60d1cc5ff0";
    hax.url = "github:cryspen/hax/5b0ba8be6da3c313fdfed1c19dd0f0721a29f4b3";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      fenix,
      aeneas,
      hax,
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

        # The base tier used by every CI step: rustc, test/kani tooling, elan
        # (Lean + lake), and the FFI/oracle deps. Deliberately does NOT include the
        # heavy hax/Aeneas *binaries* — building the Lean proofs (`lake build
        # certify-check`) needs only elan + the Aeneas Lean *library* (a lake `require`,
        # fetched from git), not the extraction binaries. Keeping them out means the
        # fmt/clippy/test/kani/Lean CI steps don't rebuild the OCaml/Rust toolchain.
        basePackages = [
          rustToolchain
          pkgs.cargo-fuzz
          pkgs.cargo-nextest # test runner (nicer output; scales to the heavy suite ahead)

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

        # `lake`/`git`-in-lake break under the darwin C-toolchain's DEVELOPER_DIR/
        # SDKROOT (nix xcbuild xcrun rejects the SDK-only path). Unset them for Lean
        # work; the C++ FFI (difftest/CGAL/CBMC) still sees them per-invocation.
        leanEnvNote = ''
          if [ "$(uname)" = "Darwin" ]; then
            alias lake-clean-env='env -u DEVELOPER_DIR -u SDKROOT lake'
          fi
        '';
      in
      {
        # Default: the CI + everyday shell. Lean proofs build here (elan + lake).
        devShells.default = pkgs.mkShell {
          name = "kirigami";
          packages = basePackages;
          shellHook = ''
            echo "kirigami dev shell:  $(rustc --version 2>/dev/null || echo 'rust not on PATH')"
            echo "  Lean via elan; run \`nix develop .#extraction\` for hax/charon/aeneas"
            ${leanEnvNote}
          '';
        };

        # Extraction: base + the hax/Aeneas *binaries*, for (re)generating the
        # lifted Lean models (`charon cargo --preset=aeneas … && aeneas -backend
        # lean …`, `cargo hax into …`). The `aeneas` package's bin/ ships `aeneas`
        # AND a co-versioned `charon`; `hax` ships `cargo-hax`. Pins in `inputs`;
        # revs resolved by the §7 spike (docs/spike-extraction-report.md §1).
        devShells.extraction = pkgs.mkShell {
          name = "kirigami-extraction";
          packages = basePackages ++ [
            aeneas.packages.${system}.default
            hax.packages.${system}.default
          ];
          shellHook = ''
            echo "kirigami extraction shell:"
            echo "  $(aeneas -version 2>/dev/null | head -1 || echo aeneas) · charon · $(cargo-hax --version 2>/dev/null | head -1 || echo cargo-hax)"
            ${leanEnvNote}
          '';
        };
      }
    );
}
