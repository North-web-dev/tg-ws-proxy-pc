{
  description = "Desktop MTProto proxy for Telegram over Cloudflare WebSocket (CLI + GUI)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});

      guiRuntimeLibs = pkgs:
        pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux (with pkgs; [
          libGL
          libxkbcommon
          wayland
          libx11
          libxcursor
          libxi
          libxrandr
          libxcb
        ]);
    in
    {
      packages = forAll (pkgs:
        let
          inherit (pkgs) lib;
          runtimeLibs = guiRuntimeLibs pkgs;

          common = {
            version = "1.0.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            doCheck = false;
            meta = {
              description = "MTProto proxy for Telegram over Cloudflare WebSocket";
              homepage = "https://github.com/North-web-dev/tg-ws-proxy-pc";
              license = lib.licenses.gpl3Only;
              platforms = systems;
            };
          };
        in
        rec {
          cli = pkgs.rustPlatform.buildRustPackage (common // {
            pname = "tgwsproxy-cli";
            cargoBuildFlags = [ "--bin" "tgwsproxy-cli" ];
            meta = common.meta // { mainProgram = "tgwsproxy-cli"; };
          });

          gui = pkgs.rustPlatform.buildRustPackage (common // {
            pname = "tgwsproxy-gui";
            buildFeatures = [ "gui" ];
            cargoBuildFlags = [ "--bin" "tgwsproxy-gui" ];

            nativeBuildInputs = [ pkgs.pkg-config pkgs.makeWrapper ];
            buildInputs = runtimeLibs;

            postFixup = lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
              wrapProgram $out/bin/tgwsproxy-gui \
                --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath runtimeLibs}
            '';

            meta = common.meta // { mainProgram = "tgwsproxy-gui"; };
          });

          default = cli;
        });

      apps = forAll (pkgs:
        let p = self.packages.${pkgs.stdenv.hostPlatform.system}; in {
          cli = {
            type = "app";
            program = "${p.cli}/bin/tgwsproxy-cli";
            meta.description = "Headless MTProto proxy for Telegram over Cloudflare WebSocket";
          };
          gui = {
            type = "app";
            program = "${p.gui}/bin/tgwsproxy-gui";
            meta.description = "Desktop GUI for the Telegram MTProto proxy";
          };
          default = self.apps.${pkgs.stdenv.hostPlatform.system}.cli;
        });

      devShells = forAll (pkgs:
        let runtimeLibs = guiRuntimeLibs pkgs; in {
          default = pkgs.mkShell ({
            packages = with pkgs; [ cargo rustc rust-analyzer clippy rustfmt pkg-config ];
          } // pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
          });
        });

      nixosModules = rec {
        tg-ws-proxy-rs = import ./modules/nixos { inherit self; };
        default = tg-ws-proxy-rs;
      };
    };
}
