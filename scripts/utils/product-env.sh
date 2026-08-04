# shellcheck shell=bash
#
# Shared product-environment selection (prod | beta | staging) for the dev
# scripts. Source it, parse your own flags with warren_env_flag, then settle
# the choice with warren_env_require.
#
# The environment is compiled INTO a build: API host, update channel, product
# paths, firewall identity. Nothing at runtime moves a release build to
# another one, and each environment installs as a separate product. So a
# launcher that picks silently is how a beta test session ends up talking to
# the prod backend, and the launchers refuse to build without an explicit
# choice.
#
# The tables below mirror warren-product-env/src/lib.rs, which reads this file
# and fails on drift (dev_launcher_tables_match_the_shared_shell_helper).

# Read by the sourcing script, not here.
# shellcheck disable=SC2034
WARREN_ENV_FLAG=""

# Sets WARREN_ENV_FLAG and returns 0 when the argument selects an environment,
# so a caller can keep owning the rest of its command line.
warren_env_flag() {
    case "${1:-}" in
        --prod)    WARREN_ENV_FLAG="prod" ;;
        --beta)    WARREN_ENV_FLAG="beta" ;;
        --staging) WARREN_ENV_FLAG="staging" ;;
        *) return 1 ;;
    esac
}

warren_env_is_valid() {
    case "${1:-}" in
        prod|beta|staging) return 0 ;;
        *) return 1 ;;
    esac
}

# Per-install directory: names the daemon's socket, settings, cache and logs.
# Two environments never share it, so a CLI pointed at the wrong one talks to
# nothing while reporting a healthy "not running".
warren_env_product_dir() {
    case "${1:-}" in
        prod)    echo "warren-vpn" ;;
        beta)    echo "warren-vpn-beta" ;;
        staging) echo "warren-vpn-staging" ;;
        *) return 1 ;;
    esac
}

# Name of the installed CLI. The desktop packaging suffixes every installed
# name of a non-prod environment (desktop/.../tasks/distribution.cjs), so the
# three products coexist on one machine.
warren_env_cli_name() {
    case "${1:-}" in
        prod)    echo "warren" ;;
        beta)    echo "warren-beta" ;;
        staging) echo "warren-staging" ;;
        *) return 1 ;;
    esac
}

warren_env_api_host() {
    case "${1:-}" in
        prod)    echo "api.warrenbrowse.com" ;;
        beta)    echo "api.beta.warrenbrowse.com" ;;
        staging) echo "api.staging.warrenbrowse.com" ;;
        *) return 1 ;;
    esac
}

# Settles the environment and exports it: the flag the caller parsed wins,
# then WARREN_PRODUCT_ENV (what the Windows VM build helpers set), and nothing
# implicit after that. A bare invocation gets the caller's usage and builds
# nothing. Call it directly, never in a subshell: it exits on failure.
warren_env_require() {
    local chosen="${1:-${WARREN_PRODUCT_ENV:-}}"
    if [[ -z "$chosen" ]]; then
        usage >&2
        printf '\nerror: pick an environment: --prod, --beta or --staging\n' >&2
        exit 1
    fi
    if ! warren_env_is_valid "$chosen"; then
        printf 'error: environment must be prod, beta or staging, got: %s\n' "$chosen" >&2
        exit 1
    fi
    WARREN_PRODUCT_ENV="$chosen"
    export WARREN_PRODUCT_ENV
}
