#!/usr/bin/env bats

load test_helper

setup() {
    setup_installer_test
    export REMOTE_ROOT="$TEST_ROOT/remote-root"
    export REMOTE_SCRIPT="$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/remote/configure_cockpit.sh"
    export BOOTSTRAP_SCRIPT="$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/remote/bootstrap.sh"
    mkdir -p "$REMOTE_ROOT/etc/cockpit" "$REMOTE_ROOT/etc/systemd/system" "$REMOTE_ROOT/etc/cloudflared"
    printf '%s\n' operator >"$REMOTE_ROOT/etc/cockpit/disallowed-users"

    make_fake_command id '[[ ${1-} == schoolorbit ]]'
    make_fake_command chpasswd 'cat >"$CAPTURED_STDIN"'
    make_fake_command systemctl 'printf "systemctl %s\n" "$*" >>"$FAKE_COMMAND_LOG"'
    make_fake_command ss 'printf "%s\n" "LISTEN 0 4096 127.0.0.1:9090 0.0.0.0:*"'
    make_fake_command curl 'printf "%s\n" "{\"service\":\"cockpit\"}"'
    make_fake_command ufw 'printf "%s\n" "Status: active"'
}

teardown() {
    teardown_installer_test
}

cockpit_payload() {
    jq -cn '{
      server_user:"schoolorbit",
      server_password:"Strong-Cockpit-Password-2026",
      management_hostname:"server.schoolorbit.app",
      tunnel_token:"eyJhIjoiYWNjb3VudC00NTYiLCJ0IjoiY29ja3BpdC10dW5uZWwtdG9rZW4ifQ"
    }'
}

run_remote_configure() {
    local payload
    payload=${REMOTE_PAYLOAD-}
    [[ -n $payload ]] || payload=$(cockpit_payload)
    run env SCHOOLORBIT_INSTALLER_TEST_ROOT="$REMOTE_ROOT" \
        PATH="$FAKE_BIN:$ORIGINAL_PATH" \
        FAKE_COMMAND_LOG="$FAKE_COMMAND_LOG" CAPTURED_STDIN="$CAPTURED_STDIN" \
        bash "$REMOTE_SCRIPT" <<<"$payload"
}

@test "remote Cockpit configuration rejects nine characters and accepts ten" {
    REMOTE_PAYLOAD=$(cockpit_payload | jq '.server_password = "123456789"')
    run_remote_configure
    [ "$status" -ne 0 ]

    REMOTE_PAYLOAD=$(cockpit_payload | jq '.server_password = "1234567890"')
    run_remote_configure
    [ "$status" -eq 0 ]
    [ "$(<"$CAPTURED_STDIN")" = 'schoolorbit:1234567890' ]
}

@test "remote Cockpit configuration is loopback-only root-safe and token-file based" {
    run_remote_configure
    [ "$status" -eq 0 ]

    grep -Fxq 'ListenStream=127.0.0.1:9090' \
        "$REMOTE_ROOT/etc/systemd/system/cockpit.socket.d/listen.conf"
    grep -Fxq root "$REMOTE_ROOT/etc/cockpit/disallowed-users"
    grep -Fxq operator "$REMOTE_ROOT/etc/cockpit/disallowed-users"
    grep -Fq 'Origins = https://server.schoolorbit.app' "$REMOTE_ROOT/etc/cockpit/cockpit.conf"
    grep -Fq 'ProtocolHeader = X-Forwarded-Proto' "$REMOTE_ROOT/etc/cockpit/cockpit.conf"
    grep -Fq 'ForwardedForHeader = X-Forwarded-For' "$REMOTE_ROOT/etc/cockpit/cockpit.conf"
    grep -Fq 'LoginTo = false' "$REMOTE_ROOT/etc/cockpit/cockpit.conf"
    grep -Fq -- '--token-file /etc/cloudflared/schoolorbit-cockpit.token' \
        "$REMOTE_ROOT/etc/systemd/system/schoolorbit-cloudflared.service"
    [ "$(stat -c %a "$REMOTE_ROOT/etc/cloudflared/schoolorbit-cockpit.token")" = 600 ]
    [ "$(<"$CAPTURED_STDIN")" = 'schoolorbit:Strong-Cockpit-Password-2026' ]
}

@test "remote Cockpit configuration is idempotent" {
    run_remote_configure
    [ "$status" -eq 0 ]
    local first
    first=$(find "$REMOTE_ROOT/etc" -type f -print0 | sort -z | xargs -0 sha256sum)

    run_remote_configure
    [ "$status" -eq 0 ]
    [ "$(find "$REMOTE_ROOT/etc" -type f -print0 | sort -z | xargs -0 sha256sum)" = "$first" ]
}

@test "remote Cockpit configuration rejects public port 9090 listeners" {
    make_fake_command ss 'printf "%s\n" "LISTEN 0 4096 0.0.0.0:9090 0.0.0.0:*"'

    run_remote_configure
    [ "$status" -eq 78 ]
}

@test "remote Cockpit configuration requires root outside the isolated test root" {
    run bash "$REMOTE_SCRIPT" <<<"$(cockpit_payload)"
    [ "$status" -eq 77 ]
}

@test "bootstrap selects pinned cloudflared artifacts for amd64 and arm64" {
    run bash -c 'source "$1"; cloudflared_artifact_for_arch amd64' _ "$BOOTSTRAP_SCRIPT"
    [ "$status" -eq 0 ]
    [ "$output" = $'https://github.com/cloudflare/cloudflared/releases/download/2026.7.3/cloudflared-linux-amd64.deb\t049777d30f9bf93da6df8bbe31383460eb2aa51a832c6551824d56f9fcc55974' ]

    run bash -c 'source "$1"; cloudflared_artifact_for_arch arm64' _ "$BOOTSTRAP_SCRIPT"
    [ "$status" -eq 0 ]
    [ "$output" = $'https://github.com/cloudflare/cloudflared/releases/download/2026.7.3/cloudflared-linux-arm64.deb\td3ea7d22dd337b465da33d6bc1c4b3cfd381407447a2a7d29542c19783430db3' ]

    run bash -c 'source "$1"; cloudflared_artifact_for_arch ppc64el' _ "$BOOTSTRAP_SCRIPT"
    [ "$status" -eq 69 ]
}

@test "bootstrap package contract includes sudo and Cockpit Podman without opening 9090" {
    run bash -c 'source "$1"; printf "%s\n" "${SCHOOLORBIT_BOOTSTRAP_PACKAGES[@]}"' _ "$BOOTSTRAP_SCRIPT"
    [ "$status" -eq 0 ]
    [[ "$output" == *$'sudo\n'* ]]
    [[ "$output" == *$'cockpit\ncockpit-podman'* ]]
    ! grep -Eq 'ufw allow (9090|"?\$\{?COCKPIT)' "$BOOTSTRAP_SCRIPT"
}

@test "bootstrap grants the server user password-backed sudo access exactly once" {
    local admin_group_state="$TEST_ROOT/admin-group-state"
    export ADMIN_GROUP_STATE=$admin_group_state

    make_fake_command id '
case "$*" in
    "-nG schoolorbit")
        if [ -f "$ADMIN_GROUP_STATE" ]; then
            printf "%s\n" "schoolorbit sudo"
        else
            printf "%s\n" "schoolorbit"
        fi
        ;;
    *) exit 1 ;;
esac
'
    make_fake_command getent '
[ "$*" = "group sudo" ]
printf "%s\n" "sudo:x:27:"
'
    make_fake_command usermod '
[ "$*" = "--append --groups sudo schoolorbit" ]
printf "usermod %s\n" "$*" >>"$FAKE_COMMAND_LOG"
touch "$ADMIN_GROUP_STATE"
'

    run env PATH="$FAKE_BIN:$ORIGINAL_PATH" \
        FAKE_COMMAND_LOG="$FAKE_COMMAND_LOG" ADMIN_GROUP_STATE="$ADMIN_GROUP_STATE" \
        bash -c 'source "$1"; ensure_server_user_administrator schoolorbit; ensure_server_user_administrator schoolorbit' \
        _ "$BOOTSTRAP_SCRIPT"

    [ "$status" -eq 0 ]
    [ -f "$admin_group_state" ]
    [ "$(grep -c '^usermod --append --groups sudo schoolorbit$' "$FAKE_COMMAND_LOG")" -eq 1 ]
}

@test "bootstrap removes a previous public Cockpit firewall allow" {
    local firewall_state="$TEST_ROOT/ufw-9090-open"
    touch "$firewall_state"
    export FIREWALL_STATE=$firewall_state
    make_fake_command ufw '
printf "ufw %s\n" "$*" >>"$FAKE_COMMAND_LOG"
if [ "$*" = "--force delete allow 9090/tcp" ]; then
    # Debian UFW exits successfully even when this exact rule does not exist.
    exit 0
fi
if [ "$*" = "status numbered" ]; then
    printf "%s\n" "Status: active"
    [ ! -f "$FIREWALL_STATE" ] || printf "%s\n" "[ 6] 9090/tcp ALLOW IN Anywhere"
elif [ "$*" = "status" ]; then
    printf "%s\n" "Status: active"
    [ ! -f "$FIREWALL_STATE" ] || printf "%s\n" "9090/tcp ALLOW Anywhere"
elif [ "$*" = "--force delete 6" ]; then
    rm -f "$FIREWALL_STATE"
fi
'

    run bash -c 'source "$1"; close_public_cockpit_firewall' _ "$BOOTSTRAP_SCRIPT"

    [ "$status" -eq 0 ]
    [ ! -e "$firewall_state" ]
    grep -Fq 'ufw --force delete 6' "$FAKE_COMMAND_LOG"
    ! grep -Fq 'ufw --force delete allow 9090/tcp' "$FAKE_COMMAND_LOG"
}
