#!/usr/bin/env bash
set -euo pipefail

SCHOOLORBIT_CLOUDFLARED_VERSION=2026.7.3
declare -ga SCHOOLORBIT_BOOTSTRAP_PACKAGES=(
    podman
    podman-compose
    uidmap
    slirp4netns
    fuse-overlayfs
    curl
    jq
    openssl
    gettext-base
    ca-certificates
    sudo
    ufw
    cockpit
    cockpit-podman
)

cloudflared_artifact_for_arch() {
    local architecture=$1 url sha256
    case "$architecture" in
        amd64)
            url="https://github.com/cloudflare/cloudflared/releases/download/$SCHOOLORBIT_CLOUDFLARED_VERSION/cloudflared-linux-amd64.deb"
            sha256=049777d30f9bf93da6df8bbe31383460eb2aa51a832c6551824d56f9fcc55974
            ;;
        arm64)
            url="https://github.com/cloudflare/cloudflared/releases/download/$SCHOOLORBIT_CLOUDFLARED_VERSION/cloudflared-linux-arm64.deb"
            sha256=d3ea7d22dd337b465da33d6bc1c4b3cfd381407447a2a7d29542c19783430db3
            ;;
        *)
            printf 'Unsupported cloudflared architecture: %s\n' "$architecture" >&2
            return 69
            ;;
    esac
    printf '%s\t%s\n' "$url" "$sha256"
}

install_pinned_cloudflared() {
    local installed_version architecture artifact_url expected_sha256 actual_sha256 package_file
    installed_version=$(dpkg-query -W -f='${Version}' cloudflared 2>/dev/null || true)
    [[ $installed_version != "$SCHOOLORBIT_CLOUDFLARED_VERSION" ]] || return 0
    architecture=$(dpkg --print-architecture)
    IFS=$'\t' read -r artifact_url expected_sha256 < <(cloudflared_artifact_for_arch "$architecture") || return
    package_file=$(mktemp "${TMPDIR:-/tmp}/cloudflared.XXXXXX.deb")
    trap 'rm -f "$package_file"' RETURN
    curl --silent --show-error --fail --location "$artifact_url" --output "$package_file"
    actual_sha256=$(sha256sum "$package_file" | awk '{print $1}')
    [[ $actual_sha256 == "$expected_sha256" ]] || {
        printf 'cloudflared package checksum is invalid\n' >&2
        return 78
    }
    dpkg -i "$package_file"
    rm -f "$package_file"
    trap - RETURN
}

ufw_has_rule() {
    local port=$1 action=$2
    LC_ALL=C ufw status | grep -Eq "^${port}/tcp[[:space:]]+${action}([[:space:]]|$)"
}

ufw_cockpit_allow_rule_numbers() {
    LC_ALL=C ufw status numbered | awk '
        /^\[[[:space:]]*[0-9]+\][[:space:]]+9090\/tcp([[:space:]]|\(v6\))/ &&
        /[[:space:]]ALLOW([[:space:]]|$)/ {
            line = $0
            sub(/^\[[[:space:]]*/, "", line)
            sub(/\].*$/, "", line)
            gsub(/[[:space:]]/, "", line)
            print line
        }
    ' | sort -rn
}

close_public_cockpit_firewall() {
    local rule_number
    if LC_ALL=C ufw status | grep -Fq 'Status: active'; then
        while IFS= read -r rule_number; do
            [[ $rule_number =~ ^[0-9]+$ ]] || continue
            ufw --force delete "$rule_number" >/dev/null
        done < <(ufw_cockpit_allow_rule_numbers)
        if LC_ALL=C ufw status | grep -Eq '^9090/tcp([[:space:]]+\(v6\))?[[:space:]]+ALLOW([[:space:]]|$)'; then
            printf 'Unable to close public Cockpit firewall access\n' >&2
            return 78
        fi
    fi
}

ensure_server_user_administrator() {
    local server_user=${1:?Server user is required}
    getent group sudo >/dev/null || {
        printf 'The sudo administrator group is unavailable\n' >&2
        return 78
    }
    if ! id -nG "$server_user" | tr ' ' '\n' | grep -Fxq sudo; then
        usermod --append --groups sudo "$server_user"
    fi
    id -nG "$server_user" | tr ' ' '\n' | grep -Fxq sudo || {
        printf 'Unable to grant the server user administrative access\n' >&2
        return 78
    }
}

enable_server_user_podman_socket() {
    local server_user=${1:?Server user is required}
    local server_uid server_home runtime_directory

    server_uid=$(id -u "$server_user")
    server_home=$(getent passwd "$server_user" | awk -F: 'NR == 1 { print $6 }')
    [[ $server_home == /* ]] || {
        printf 'The server user home directory is unavailable\n' >&2
        return 78
    }
    runtime_directory="/run/user/$server_uid"

    systemctl start "user@${server_uid}.service"
    runuser -u "$server_user" -- env \
        HOME="$server_home" \
        XDG_RUNTIME_DIR="$runtime_directory" \
        DBUS_SESSION_BUS_ADDRESS="unix:path=$runtime_directory/bus" \
        systemctl --user enable --now podman.socket
    runuser -u "$server_user" -- env \
        HOME="$server_home" \
        XDG_RUNTIME_DIR="$runtime_directory" \
        DBUS_SESSION_BUS_ADDRESS="unix:path=$runtime_directory/bus" \
        systemctl --user is-active --quiet podman.socket
}

schoolorbit_bootstrap_main() {
    local ssh_port=${1:?SSH port is required} server_user=${2:?server user is required}
    local package sysctl_file desired_sysctl temporary
    local -a missing_packages=()

    if [[ ! $ssh_port =~ ^[0-9]{1,5}$ ]] || ((10#$ssh_port < 1 || 10#$ssh_port > 65535)); then
        printf 'Invalid SSH port\n' >&2
        return 64
    fi
    [[ $server_user =~ ^[a-z_][a-z0-9_-]{0,31}$ ]] || {
        printf 'Invalid server user\n' >&2
        return 64
    }
    ((EUID == 0)) || {
        printf 'Bootstrap must run as root\n' >&2
        return 77
    }

    for package in "${SCHOOLORBIT_BOOTSTRAP_PACKAGES[@]}"; do
        if ! dpkg-query -W -f='${db:Status-Abbrev}' "$package" 2>/dev/null | grep -q '^ii '; then
            missing_packages+=("$package")
        fi
    done
    if ((${#missing_packages[@]} > 0)); then
        apt-get update
        DEBIAN_FRONTEND=noninteractive apt-get install -y "${missing_packages[@]}"
    fi
    install_pinned_cloudflared

    if ! id "$server_user" >/dev/null 2>&1; then
        useradd --create-home --shell /bin/bash "$server_user"
    fi
    ensure_server_user_administrator "$server_user"
    if [[ ! -e /var/lib/systemd/linger/$server_user ]]; then
        loginctl enable-linger "$server_user"
    fi
    enable_server_user_podman_socket "$server_user"

    install -d -m 0750 -o "$server_user" -g "$server_user" \
        /opt/stack \
        /opt/stack/nginx \
        /opt/stack/nginx/conf.d \
        /opt/stack/nginx/ssl \
        /opt/stack/deployment

    sysctl_file=/etc/sysctl.d/90-schoolorbit-rootless-ports.conf
    desired_sysctl='net.ipv4.ip_unprivileged_port_start=80'
    if [[ ! -f $sysctl_file ]] || [[ $(<"$sysctl_file") != "$desired_sysctl" ]]; then
        temporary=$(mktemp "${sysctl_file}.XXXXXX")
        printf '%s\n' "$desired_sysctl" >"$temporary"
        chmod 0644 "$temporary"
        mv "$temporary" "$sysctl_file"
        sysctl --system >/dev/null
    fi

    ufw_has_rule "$ssh_port" ALLOW || ufw allow "${ssh_port}/tcp"
    ufw_has_rule 80 ALLOW || ufw allow 80/tcp
    ufw_has_rule 443 ALLOW || ufw allow 443/tcp
    ufw_has_rule 8080 DENY || ufw deny 8080/tcp
    ufw_has_rule 8081 DENY || ufw deny 8081/tcp
    close_public_cockpit_firewall
    if LC_ALL=C ufw status | grep -Fq 'Status: inactive'; then
        ufw --force enable
    fi
    close_public_cockpit_firewall
}

if [[ -z ${BASH_SOURCE[0]-} || ${BASH_SOURCE[0]} == "$0" ]]; then
    schoolorbit_bootstrap_main "$@"
fi
