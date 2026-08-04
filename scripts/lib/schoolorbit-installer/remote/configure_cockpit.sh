#!/usr/bin/env bash
set -euo pipefail

ROOT_PREFIX=${SCHOOLORBIT_INSTALLER_TEST_ROOT-}
ROOT_PREFIX=${ROOT_PREFIX%/}
if [[ -z $ROOT_PREFIX ]]; then
    ((EUID == 0)) || {
        printf 'Cockpit configuration must run as root\n' >&2
        exit 77
    }
else
    [[ $ROOT_PREFIX == /* && $ROOT_PREFIX != / && -d $ROOT_PREFIX ]] || {
        printf 'Invalid isolated test root\n' >&2
        exit 64
    }
fi

root_path() {
    printf '%s%s\n' "$ROOT_PREFIX" "$1"
}

atomic_write() {
    local destination=$1 mode=$2 content=$3 temporary
    temporary=$(mktemp "${destination}.XXXXXX")
    trap 'rm -f "$temporary"' RETURN
    printf '%s\n' "$content" >"$temporary"
    chmod "$mode" "$temporary"
    if [[ -z $ROOT_PREFIX ]]; then
        chown root:root "$temporary"
    fi
    mv "$temporary" "$destination"
    trap - RETURN
}

payload=$(cat)
jq -e '
    type == "object" and
    (keys | sort) == ["management_hostname","server_password","server_user","tunnel_token"]
' <<<"$payload" >/dev/null || {
    printf 'Cockpit input must be one exact JSON object\n' >&2
    exit 64
}
server_user=$(jq -er '.server_user | strings | select(length > 0)' <<<"$payload")
server_password=$(jq -er '.server_password | strings | select(length >= 10)' <<<"$payload")
management_hostname=$(jq -er '.management_hostname | strings | select(length > 0)' <<<"$payload")
tunnel_token=$(jq -er '.tunnel_token | strings | select(length >= 32)' <<<"$payload")
unset payload

[[ $server_user =~ ^[a-z_][a-z0-9_-]{0,31}$ ]] || exit 64
[[ $server_password != *$'\n'* && $server_password != *$'\r'* ]] || exit 64
[[ $tunnel_token != *$'\n'* && $tunnel_token != *$'\r'* ]] || exit 64
[[ $management_hostname =~ ^server\.[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$ ]] || exit 64
id "$server_user" >/dev/null 2>&1 || {
    printf 'Cockpit server user does not exist\n' >&2
    exit 78
}

printf '%s:%s\n' "$server_user" "$server_password" | chpasswd
unset server_password

cockpit_directory=$(root_path /etc/cockpit)
cloudflared_directory=$(root_path /etc/cloudflared)
systemd_directory=$(root_path /etc/systemd/system)
socket_directory="$systemd_directory/cockpit.socket.d"
install -d -m 0755 "$cockpit_directory" "$cloudflared_directory" "$systemd_directory" "$socket_directory"

disallowed_users="$cockpit_directory/disallowed-users"
disallowed_content=''
[[ ! -f $disallowed_users ]] || disallowed_content=$(<"$disallowed_users")
disallowed_content=$(printf '%s\n%s\n' "$disallowed_content" root | awk 'NF && !seen[$0]++')
atomic_write "$disallowed_users" 0644 "$disallowed_content"

cockpit_config=$(
    cat <<EOF
[WebService]
Origins = https://$management_hostname
ProtocolHeader = X-Forwarded-Proto
ForwardedForHeader = X-Forwarded-For
LoginTo = false
AllowUnencrypted = true
EOF
)
atomic_write "$cockpit_directory/cockpit.conf" 0644 "$cockpit_config"

socket_config=$(
    cat <<'EOF'
[Socket]
ListenStream=
ListenStream=127.0.0.1:9090
EOF
)
atomic_write "$socket_directory/listen.conf" 0644 "$socket_config"

token_file="$cloudflared_directory/schoolorbit-cockpit.token"
atomic_write "$token_file" 0600 "$tunnel_token"
unset tunnel_token

cloudflared_service=$(
    cat <<'EOF'
[Unit]
Description=SchoolOrbit Cockpit Cloudflare Tunnel
After=network-online.target cockpit.socket
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/bin/cloudflared --no-autoupdate tunnel run --token-file /etc/cloudflared/schoolorbit-cockpit.token
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
EOF
)
atomic_write "$systemd_directory/schoolorbit-cloudflared.service" 0644 "$cloudflared_service"

systemctl daemon-reload
systemctl enable cockpit.socket schoolorbit-cloudflared.service >/dev/null
systemctl restart cockpit.socket schoolorbit-cloudflared.service
systemctl is-active --quiet cockpit.socket schoolorbit-cloudflared.service

listeners=$(ss -ltnH '( sport = :9090 )')
[[ -n $listeners ]] || {
    printf 'Cockpit port 9090 is not listening\n' >&2
    exit 78
}
while read -r _ _ _ local_address _; do
    [[ $local_address == 127.0.0.1:9090 ]] || {
        printf 'Cockpit port 9090 has a public listener\n' >&2
        exit 78
    }
done <<<"$listeners"

jq -e '.service == "cockpit"' < <(curl -fsS http://127.0.0.1:9090/ping) >/dev/null || {
    printf 'Cockpit ping failed\n' >&2
    exit 78
}
[[ $(stat -c %a "$token_file") == 600 ]] || exit 78
if command -v ufw >/dev/null 2>&1 && LC_ALL=C ufw status | grep -Eq '^9090/tcp[[:space:]]+ALLOW([[:space:]]|$)'; then
    printf 'Firewall exposes Cockpit port 9090\n' >&2
    exit 78
fi
