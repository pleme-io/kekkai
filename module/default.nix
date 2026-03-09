# Kekkai home-manager module — GPU-rendered NordVPN client
#
# Namespace: blackmatter.components.kekkai.*
#
# Generates YAML config from typed Nix options, loaded by shikumi at runtime.
#
# Module factory: receives { hmHelpers } from flake.nix, returns HM module.
{ hmHelpers }:
{
  lib,
  config,
  pkgs,
  ...
}:
with lib;
let
  inherit (hmHelpers) mkLaunchdService mkSystemdService;
  cfg = config.blackmatter.components.kekkai;
  isDarwin = pkgs.stdenv.isDarwin;

  logDir =
    if isDarwin then "${config.home.homeDirectory}/Library/Logs"
    else "${config.home.homeDirectory}/.local/share/kekkai/logs";

  # ── YAML config generation ────────────────────────────────────────
  settingsAttr = let
    api = filterAttrs (_: v: v != null) {
      server_list_url = cfg.api.server_list_url;
      cache_ttl_secs = cfg.api.cache_ttl_secs;
    };

    connection = filterAttrs (_: v: v != null) {
      inherit (cfg.connection) protocol auto_connect kill_switch nordvpn_path;
      preferred_country = cfg.connection.preferred_country;
      preferred_city = cfg.connection.preferred_city;
      dns = if cfg.connection.dns == [] then null else cfg.connection.dns;
    };

    appearance = filterAttrs (_: v: v != null) {
      inherit (cfg.appearance) width height background foreground accent;
    };

    favorites = if cfg.favorites == [] then null else cfg.favorites;
  in
    filterAttrs (_: v: v != {} && v != null) {
      inherit api connection appearance favorites;
    }
    // cfg.extraSettings;

  yamlConfig = pkgs.writeText "kekkai.yaml"
    (lib.generators.toYAML { } settingsAttr);
in
{
  options.blackmatter.components.kekkai = {
    enable = mkEnableOption "Kekkai — GPU-rendered NordVPN client";

    package = mkOption {
      type = types.package;
      default = pkgs.kekkai;
      description = "The kekkai package to use.";
    };

    # ── API ──────────────────────────────────────────────────────────
    api = {
      server_list_url = mkOption {
        type = types.str;
        default = "https://api.nordvpn.com/v1";
        description = "NordVPN server list API URL.";
      };

      cache_ttl_secs = mkOption {
        type = types.int;
        default = 3600;
        description = "Server list cache TTL in seconds.";
      };
    };

    # ── Connection ───────────────────────────────────────────────────
    connection = {
      protocol = mkOption {
        type = types.str;
        default = "NordLynx";
        description = "Preferred VPN protocol: NordLynx, openvpn_udp, openvpn_tcp.";
      };

      auto_connect = mkOption {
        type = types.bool;
        default = false;
        description = "Connect automatically on launch.";
      };

      preferred_country = mkOption {
        type = types.nullOr types.str;
        default = null;
        description = "Default country for quick connect.";
        example = "United States";
      };

      preferred_city = mkOption {
        type = types.nullOr types.str;
        default = null;
        description = "Default city for quick connect.";
        example = "New York";
      };

      kill_switch = mkOption {
        type = types.bool;
        default = true;
        description = "Enable kill switch.";
      };

      dns = mkOption {
        type = types.listOf types.str;
        default = [];
        description = "Custom DNS servers.";
        example = [ "1.1.1.1" "8.8.8.8" ];
      };

      nordvpn_path = mkOption {
        type = types.str;
        default = "nordvpn";
        description = "Path to the nordvpn CLI binary.";
      };
    };

    # ── Appearance ───────────────────────────────────────────────────
    appearance = {
      width = mkOption {
        type = types.int;
        default = 1200;
        description = "Window width in pixels.";
      };

      height = mkOption {
        type = types.int;
        default = 800;
        description = "Window height in pixels.";
      };

      background = mkOption {
        type = types.str;
        default = "#2e3440";
        description = "Background color (hex).";
      };

      foreground = mkOption {
        type = types.str;
        default = "#eceff4";
        description = "Foreground color (hex).";
      };

      accent = mkOption {
        type = types.str;
        default = "#88c0d0";
        description = "Accent color (hex).";
      };
    };

    # ── Favorites ────────────────────────────────────────────────────
    favorites = mkOption {
      type = types.listOf types.str;
      default = [];
      description = "Pinned server hostnames.";
      example = [ "us100.nordvpn.com" "de50.nordvpn.com" ];
    };

    # ── Daemon ───────────────────────────────────────────────────────
    daemon = {
      enable = mkOption {
        type = types.bool;
        default = false;
        description = "Run kekkai connection monitor as a background daemon.";
      };
    };

    # ── MCP ──────────────────────────────────────────────────────────
    mcp = {
      enable = mkOption {
        type = types.bool;
        default = false;
        description = "Register kekkai MCP server for Claude Code.";
      };
    };

    # ── Escape hatch ─────────────────────────────────────────────────
    extraSettings = mkOption {
      type = types.attrs;
      default = {};
      description = ''
        Additional raw settings merged on top of typed options.
        Values are serialized directly to YAML.
      '';
    };
  };

  config = mkIf cfg.enable (mkMerge [
    # Install the package
    {
      home.packages = [ cfg.package ];
    }

    # YAML configuration
    {
      xdg.configFile."kekkai/kekkai.yaml".source = yamlConfig;
    }

    # Darwin: launchd agent (daemon mode — connection monitor)
    (mkIf (cfg.daemon.enable && isDarwin)
      (mkLaunchdService {
        name = "kekkai";
        label = "io.pleme.kekkai";
        command = "${cfg.package}/bin/kekkai";
        args = [ "status" ];
        logDir = logDir;
        processType = "Background";
        keepAlive = true;
      })
    )

    # Linux: systemd user service (daemon mode)
    (mkIf (cfg.daemon.enable && !isDarwin)
      (mkSystemdService {
        name = "kekkai";
        description = "Kekkai — NordVPN connection monitor daemon";
        command = "${cfg.package}/bin/kekkai";
        args = [ "status" ];
      })
    )
  ]);
}
