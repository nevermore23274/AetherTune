{
  description = "A TUI radio player for internet radio stations";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    treefmt-nix.url = "github:numtide/treefmt-nix";
  };

  outputs =
    inputs@{
      self,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      forAllSystems = inputs.nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = inputs.nixpkgs.legacyPackages.${system};
          lib = inputs.nixpkgs.lib;
        in
        {
          default = self.packages.${system}.aethertune;

          aethertune = pkgs.rustPlatform.buildRustPackage {
            pname = "aethertune";
            version = "0.4.1";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            buildInputs = with pkgs; [ openssl ];
            nativeBuildInputs = with pkgs; [ pkg-config ];
            propagatedBuildInputs = with pkgs; [
              mpv
              libpulseaudio
            ];

            meta = {
              description = "A TUI radio player for internet radio stations";
              homepage = "https://github.com/nevermore23274/AetherTune";
              mainProgram = "AetherTune";
              license = lib.licenses.mit;
              maintainers = with lib.maintainers; [ lonerOrz ];
              platforms = systems;
            };
          };
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = inputs.nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            inputsFrom = [ self.packages.${system}.default ];
            packages = with pkgs; [
              cargo
              rustc
              rust-analyzer
              rustfmt
              clippy
              cargo-watch
              cargo-criterion
              openssl
              pkg-config
            ];
          };
        }
      );

      formatter = forAllSystems (
        system:
        let
          pkgs = inputs.nixpkgs.legacyPackages.${system};
          treefmtEval = inputs.treefmt-nix.lib.evalModule pkgs {
            projectRootFile = "flake.nix";
            programs = {
              rustfmt.enable = true;
              nixfmt.enable = true;
            };
          };
        in
        treefmtEval.config.build.wrapper
      );
    };
}
