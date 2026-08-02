#!/usr/bin/env bash
set -euo pipefail

SSH_PORT=${1:?SSH port is required}
SERVER_USER=${2:?server user is required}

if [[ ! $SSH_PORT =~ ^[0-9]{1,5}$ ]] || ((10#$SSH_PORT < 1 || 10#$SSH_PORT > 65535)); then
    printf 'Invalid SSH port\n' >&2
    exit 64
fi
[[ $SERVER_USER =~ ^[a-z_][a-z0-9_-]{0,31}$ ]] || {
    printf 'Invalid server user\n' >&2
    exit 64
}
((EUID == 0)) || {
    printf 'Bootstrap must run as root\n' >&2
    exit 77
}

packages=(
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
    ufw
)
missing_packages=()
for package in "${packages[@]}"; do
    if ! dpkg-query -W -f='${db:Status-Abbrev}' "$package" 2>/dev/null | grep -q '^ii '; then
        missing_packages+=("$package")
    fi
done
if ((${#missing_packages[@]} > 0)); then
    apt-get update
    DEBIAN_FRONTEND=noninteractive apt-get install -y "${missing_packages[@]}"
fi

if ! id "$SERVER_USER" >/dev/null 2>&1; then
    useradd --create-home --shell /bin/bash "$SERVER_USER"
fi
if [[ ! -e /var/lib/systemd/linger/$SERVER_USER ]]; then
    loginctl enable-linger "$SERVER_USER"
fi

install -d -m 0750 -o "$SERVER_USER" -g "$SERVER_USER" \
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

ufw_has_rule() {
    local port=$1 action=$2
    LC_ALL=C ufw status | grep -Eq "^${port}/tcp[[:space:]]+${action}([[:space:]]|$)"
}

ufw_has_rule "$SSH_PORT" ALLOW || ufw allow "${SSH_PORT}/tcp"
ufw_has_rule 80 ALLOW || ufw allow 80/tcp
ufw_has_rule 443 ALLOW || ufw allow 443/tcp
ufw_has_rule 8080 DENY || ufw deny 8080/tcp
ufw_has_rule 8081 DENY || ufw deny 8081/tcp
if LC_ALL=C ufw status | grep -Fq 'Status: inactive'; then
    ufw --force enable
fi
