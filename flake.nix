{
  description = "Docker container with Rust-based file watcher that triggers webhooks on new XML files";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default;

        # Build the Rust application
        xml-watcher = pkgs.rustPlatform.buildRustPackage {
          pname = "xml-watcher";
          version = "0.2.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = [ rustToolchain pkgs.pkg-config ];
          buildInputs = [ pkgs.openssl ];

          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        };

        # Docker image
        dockerImage = pkgs.dockerTools.buildLayeredImage {
          name = "xml-watcher";
          tag = "latest";

          contents = [
            xml-watcher
            pkgs.cacert
          ];

          config = {
            Cmd = [ "${xml-watcher}/bin/xml-watcher" ];
            Env = [
              "WATCH_DIR=/watch"
              "WEBHOOK_URL="
              "WEBHOOK_METHOD=POST"
              "INCLUDE_CONTENT=false"
              "RUST_LOG=info"
            ];
            Volumes = {
              "/watch" = { };
            };
            WorkingDir = "/";
          };
        };

      in
      {
        packages = {
          default = xml-watcher;
          docker = dockerImage;
          xml-watcher = xml-watcher;
        };

        # For local testing without Docker
        apps.default = {
          type = "app";
          program = "${xml-watcher}/bin/xml-watcher";
        };

        devShells.default = pkgs.mkShell {
          packages = [
            rustToolchain
            pkgs.pkg-config
            pkgs.openssl
            pkgs.cargo
            pkgs.rustc
          ];
          
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
          RUST_LOG = "info";
        };
      }
    );
}
