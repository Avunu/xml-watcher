{
  description = "Docker container with inotify watcher that triggers webhooks on new XML files";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        # The watcher script
        watcherScript = pkgs.writeScriptBin "xml-watcher" (builtins.readFile ./xml-watcher.sh);

        # Docker image
        dockerImage = pkgs.dockerTools.buildLayeredImage {
          name = "xml-watcher";
          tag = "latest";

          contents = [
            watcherScript
            pkgs.inotify-tools
            pkgs.curl
            pkgs.jq
            pkgs.coreutils
            pkgs.bash
          ];

          config = {
            Cmd = [ (pkgs.lib.getExe watcherScript) ];
            Env = [
              "WATCH_DIR=/watch"
              "WEBHOOK_URL="
              "WEBHOOK_METHOD=POST"
              "INCLUDE_FILENAME=true"
              "INCLUDE_CONTENT=false"
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
          default = dockerImage;
          docker = dockerImage;
          script = watcherScript;
        };

        # For local testing without Docker
        apps.default = {
          type = "app";
          program = (pkgs.lib.getExe watcherScript);
        };

        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.inotify-tools
            pkgs.curl
            pkgs.jq
          ];
        };
      }
    );
}
