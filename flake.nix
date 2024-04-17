{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = { self, nixpkgs, flake-utils, crane, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        craneLib = crane.lib.${system};
        movebeam = craneLib.buildPackage {
          src = craneLib.cleanCargoSource (craneLib.path ./.);
          strictDeps = true;
        };
      in
      {
        checks = {
          inherit movebeam;
        };
        packages.default = movebeam;
        apps.default = flake-utils.lib.mkApp {
          drv = movebeam;
        };
        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages =  [
            pkgs.clippy
            pkgs.cargo-outdated
          ];
        };
      }
    );
}

