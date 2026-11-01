{ self }:
{ config, lib, pkgs, utils, ... }:

let
  cfg = config.services.tg-ws-proxy-rs;
  inherit (lib) mkEnableOption mkOption mkIf types;

  exe = lib.getExe cfg.package;

  args =
    [
      "--host" cfg.host
      "--port" (toString cfg.port)
      "--pool-size" (toString cfg.poolSize)
      "--stats-interval" (toString cfg.statsInterval)
      "--cache-dir" cfg.cacheDir
    ]
    ++ lib.optionals (cfg.cfDomain != null) [ "--cf-domain" cfg.cfDomain ]
    ++ lib.optionals (cfg.dcIps != null) [ "--dc-ips" cfg.dcIps ]
    ++ lib.optional cfg.noCf "--no-cf"
    ++ lib.optional cfg.verbose "--verbose";

  # %d expands to the unit's credentials directory.
  secretArg = lib.optionalString (cfg.secret != null) " --secret %d/secret";

  isLoopback = cfg.host == "127.0.0.1" || cfg.host == "::1" || cfg.host == "localhost";
in
{
  options.services.tg-ws-proxy-rs = {
    enable = mkEnableOption "tg-ws-proxy-rs, a local MTProto proxy for Telegram over Cloudflare WebSocket";

    package = mkOption {
      type = types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.cli;
      defaultText = lib.literalExpression "tg-ws-proxy-pc.packages.\${system}.cli";
      description = "Package providing the `tgwsproxy-cli` binary.";
    };

    host = mkOption {
      type = types.str;
      default = "127.0.0.1";
      example = "0.0.0.0";
      description = "Value of `--host`: address the proxy binds to.";
    };

    port = mkOption {
      type = types.port;
      default = 1443;
      description = "Value of `--port`: local port the Telegram client connects to.";
    };

    openFirewall = mkOption {
      type = types.bool;
      default = false;
      description = ''
        Whether to open the TCP port in the firewall. Only useful when `host`
        is not a loopback address (for example `0.0.0.0`).
      '';
    };

    secret = mkOption {
      type = types.nullOr types.str;
      default = null;
      example = "/var/lib/secrets/tg-ws-proxy-rs";
      description = ''
        Value of `--secret`, given as an absolute path to a file containing
        the MTProto secret (32 hex characters, optional `dd` prefix). The file
        is loaded as a systemd credential and never enters the Nix store.
        When null, a random secret is generated on every start.
      '';
    };

    cfDomain = mkOption {
      type = types.nullOr types.str;
      default = null;
      description = "Value of `--cf-domain`: Cloudflare-WS domain override.";
    };

    noCf = mkOption {
      type = types.bool;
      default = false;
      description = "Pass `--no-cf`: disable the Cloudflare-WS transport (direct DC connections only).";
    };

    dcIps = mkOption {
      type = types.nullOr types.str;
      default = null;
      description = "Value of `--dc-ips`: DC IP overrides as a CIDR pool string.";
    };

    poolSize = mkOption {
      type = types.ints.between 2 16;
      default = 4;
      description = "Value of `--pool-size`: WS pool size per datacenter.";
    };

    cacheDir = mkOption {
      type = types.str;
      default = "/var/lib/tg-ws-proxy-rs";
      description = "Value of `--cache-dir`: cache directory for the cfproxy domain list.";
    };

    statsInterval = mkOption {
      type = types.ints.unsigned;
      default = 30;
      description = "Value of `--stats-interval`: print stats every N seconds (0 = off).";
    };

    verbose = mkOption {
      type = types.bool;
      default = false;
      description = "Pass `--verbose`: enable debug logging.";
    };
  };

  config = mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.secret == null || lib.hasPrefix "/" cfg.secret;
        message = "services.tg-ws-proxy-rs.secret must be an absolute path to a file, not the secret itself.";
      }
    ];

    warnings =
      lib.optional (cfg.openFirewall && isLoopback)
        "services.tg-ws-proxy-rs: openFirewall is enabled but host is ${cfg.host}; the proxy is not reachable from the network. Set host = \"0.0.0.0\" to listen on all interfaces."
      ++ lib.optional (!isLoopback && cfg.secret == null)
        "services.tg-ws-proxy-rs: the proxy listens on ${cfg.host} without a fixed secret; a new random secret is generated on every start. Set services.tg-ws-proxy-rs.secret.";

    networking.firewall.allowedTCPPorts = lib.optional cfg.openFirewall cfg.port;

    systemd.services.tg-ws-proxy-rs = {
      description = "Telegram MTProto proxy over Cloudflare WebSocket (tg-ws-proxy-rs)";
      wantedBy = [ "multi-user.target" ];
      wants = [ "network-online.target" ];
      after = [ "network-online.target" ];

      serviceConfig = {
        ExecStart = "${exe} ${utils.escapeSystemdExecArgs args}${secretArg}";
        Restart = "on-failure";
        RestartSec = 5;

        DynamicUser = true;
        StateDirectory = "tg-ws-proxy-rs";
        ReadWritePaths = [ cfg.cacheDir ];
        LoadCredential = lib.optional (cfg.secret != null) "secret:${cfg.secret}";

        CapabilityBoundingSet =
          if cfg.port < 1024 then [ "CAP_NET_BIND_SERVICE" ] else "";
        AmbientCapabilities = lib.optional (cfg.port < 1024) "CAP_NET_BIND_SERVICE";
        NoNewPrivileges = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictAddressFamilies = [ "AF_INET" "AF_INET6" "AF_UNIX" ];
        RestrictNamespaces = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        RestrictRealtime = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [ "@system-service" "~@privileged" ];
      };
    };
  };
}
