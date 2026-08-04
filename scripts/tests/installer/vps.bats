#!/usr/bin/env bats

load test_helper

setup() {
    local database_scheme=postgresql

    setup_installer_test
    export FIXTURE_DIR="$BATS_TEST_DIRNAME/fixtures"
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/vps.sh"

    SO_CONFIG[repository]=owner/repo
    SO_CONFIG[target]=192.0.2.20
    SO_CONFIG[base_domain]=schoolorbit.app
    SO_CONFIG[bootstrap_user]=root
    SO_CONFIG[server_user]=schoolorbit
    SO_CONFIG[ssh_port]=22
    SO_CONFIG[runtime:NEON_PROJECT_ID]=silent-moon-24680
    SO_CONFIG[runtime:NEON_HOST]=ep-silent-moon-24680.ap-southeast-1.aws.neon.tech
    SO_CONFIG[runtime:R2_ACCOUNT_ID]=9a8b7c6d5e4f32100123456789abcdef
    SO_CONFIG[runtime:R2_PUBLIC_BUCKET_NAME]=schoolorbit-public-assets
    SO_CONFIG[runtime:R2_PRIVATE_BUCKET_NAME]=schoolorbit-private-files
    SO_CONFIG[runtime:R2_PUBLIC_URL]=https://assets.schoolorbit.invalid
    SO_CONFIG[runtime:VAPID_PUBLIC_KEY]=BHT7mN3qP9vK5xT2rL8wC4sF6dG1hJ0kZyUeIoaS
    SO_CF_ZONE_ID=zone-123
    SO_CF_ACCOUNT_ID=account-456

    SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN]=must-not-reach-vps
    SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_DEPLOY_TOKEN]=deploy-token-must-not-reach-vps
    SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_RUNTIME_TOKEN]=runtime-only-value
    SO_SECRETS[SCHOOLORBIT_RUNTIME_GITHUB_TOKEN]=runtime-github-value
    SO_SECRETS[DATABASE_URL]="${database_scheme}://schoolorbit:DbPass@db.invalid/schoolorbit"
    SO_SECRETS[JWT_SECRET]=jwt-runtime-secret-value
    SO_SECRETS[INTERNAL_API_SECRET]=internal-runtime-secret-value
    SO_SECRETS[ENCRYPTION_KEY]=encryption-runtime-secret-value-32chars
    SO_SECRETS[BLIND_INDEX_KEY]=blind-index-runtime-secret-value-32chars
    SO_SECRETS[DEPLOY_KEY]=deploy-runtime-secret-value
    SO_SECRETS[NEON_API_KEY]=neon-api-runtime-value
    SO_SECRETS[NEON_DB_PASSWORD]=neon-db-runtime-value
    SO_SECRETS[R2_ACCESS_KEY_ID]=r2-access-runtime-value
    SO_SECRETS[R2_SECRET_ACCESS_KEY]=r2-secret-runtime-value
    SO_SECRETS[VAPID_PRIVATE_KEY]=vapid-private-runtime-value
    SO_SECRETS[SMOKE_PASSWORD]=smoke-password-must-not-reach-vps
    SO_SECRETS[SCHOOLORBIT_SERVER_PASSWORD]=Strong-Cockpit-Password-2026
    SO_SECRETS[SCHOOLORBIT_COCKPIT_TUNNEL_TOKEN]=eyJhIjoiYWNjb3VudC00NTYiLCJ0IjoiY29ja3BpdC10dW5uZWwtdG9rZW4ifQ
    SO_CF_COCKPIT_HOSTNAME=server.schoolorbit.app

    make_fake_command ssh '
set -eu
printf "ssh" >>"$FAKE_COMMAND_LOG"
printf " %s" "$@" >>"$FAKE_COMMAND_LOG"
printf "\n" >>"$FAKE_COMMAND_LOG"
count_file="$TEST_ROOT/ssh-count"
count=0
[ ! -f "$count_file" ] || count=$(cat "$count_file")
count=$((count + 1))
printf "%s" "$count" >"$count_file"
cat >"$TEST_ROOT/ssh-stdin-$count"
case "$*" in
    *"cat /etc/os-release"*) cat "$FIXTURE_DIR/os-release-debian" ;;
esac
'

    make_fake_command ssh-keygen '
set -eu
key_file=
while [ "$#" -gt 0 ]; do
    case "$1" in
        -f) key_file=$2; shift 2 ;;
        *) shift ;;
    esac
done
printf "%s\n" "fixture-private-deployment-key" >"$key_file"
printf "%s\n" "ssh-ed25519 AAAAC3NzaFixture schoolorbit-github-actions" >"$key_file.pub"
'
}

teardown() {
    vps_cleanup_transients
    teardown_installer_test
}

@test "supports Debian and Ubuntu and rejects another distribution" {
    run remote_os_supported "$FIXTURE_DIR/os-release-debian"
    [ "$status" -eq 0 ]
    run remote_os_supported "$FIXTURE_DIR/os-release-ubuntu"
    [ "$status" -eq 0 ]
    run remote_os_supported "$FIXTURE_DIR/os-release-unsupported"
    [ "$status" -eq 69 ]
}

@test "VPS preflight reads the target OS through strict host-key SSH" {
    vps_preflight

    grep -F 'StrictHostKeyChecking=yes' "$FAKE_COMMAND_LOG"
    grep -F 'root@192.0.2.20 cat /etc/os-release' "$FAKE_COMMAND_LOG"
}

@test "remote bootstrap guards package user linger sysctl and firewall mutations" {
    local bootstrap="$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/remote/bootstrap.sh"
    grep -Fq 'dpkg-query' "$bootstrap"
    grep -Fq 'id "$server_user"' "$bootstrap"
    grep -Fq 'useradd --create-home' "$bootstrap"
    grep -Fq '/var/lib/systemd/linger/$server_user' "$bootstrap"
    grep -Fq 'ufw status' "$bootstrap"
    grep -Fq 'ufw --force enable' "$bootstrap"
}

@test "bootstrap can run twice and verifies a fresh SSH session each time" {
    vps_bootstrap
    vps_bootstrap

    [ "$(grep -c 'bash -s -- 22 schoolorbit' "$FAKE_COMMAND_LOG")" -eq 2 ]
    [ "$(grep -c 'root@192.0.2.20 true' "$FAKE_COMMAND_LOG")" -eq 2 ]
}

@test "Cockpit bootstrap revalidation checks user packages and pinned cloudflared" {
    vps_reverify_cockpit_bootstrap

    grep -Fq 'cockpit cockpit-podman' "$FAKE_COMMAND_LOG"
    grep -Fq 'cloudflared --version' "$FAKE_COMMAND_LOG"
    grep -Fq '2026.7.3' "$FAKE_COMMAND_LOG"
    grep -Fq 'id -nG' "$FAKE_COMMAND_LOG"
    grep -Fq 'sudo -l -U' "$FAKE_COMMAND_LOG"
}

@test "Cockpit script and secrets use separate SSH stdin streams and a fresh verification session" {
    vps_configure_cockpit
    vps_reverify_cockpit

    [ "$(<"$TEST_ROOT/ssh-count")" -eq 3 ]
    grep -Fq 'SCHOOLORBIT_INSTALLER_TEST_ROOT' "$TEST_ROOT/ssh-stdin-1"
    jq -e '
      .server_user == "schoolorbit" and
      .server_password == "Strong-Cockpit-Password-2026" and
      .management_hostname == "server.schoolorbit.app" and
      .tunnel_token == "eyJhIjoiYWNjb3VudC00NTYiLCJ0IjoiY29ja3BpdC10dW5uZWwtdG9rZW4ifQ"
    ' "$TEST_ROOT/ssh-stdin-2"
    [ ! -s "$TEST_ROOT/ssh-stdin-3" ]
    grep -Fq 'systemctl is-active cockpit.socket schoolorbit-cloudflared.service' "$FAKE_COMMAND_LOG"
    run grep -E 'Strong-Cockpit-Password|eyJhIjoiYWNjb3VudC00NTY' "$FAKE_COMMAND_LOG"
    [ "$status" -eq 1 ]
}

@test "deployment private key stays out of SSH arguments and public key is streamed" {
    vps_create_deployment_key

    [ "${SO_SECRETS[SSH_PRIVATE_KEY]}" = 'fixture-private-deployment-key' ]
    run grep -F 'fixture-private-deployment-key' "$FAKE_COMMAND_LOG"
    [ "$status" -eq 1 ]
    grep -R -Fq 'ssh-ed25519 AAAAC3NzaFixture schoolorbit-github-actions' "$TEST_ROOT"/ssh-stdin-*
}

@test "runtime environment includes only runtime credentials and canonical URLs" {
    render_runtime_env >"$TEST_ROOT/runtime.env"

    ! grep -Fq must-not-reach-vps "$TEST_ROOT/runtime.env"
    grep -Fxq "CLOUDFLARE_API_TOKEN='runtime-only-value'" "$TEST_ROOT/runtime.env"
    grep -Fxq "CLOUDFLARE_ACCOUNT_ID='account-456'" "$TEST_ROOT/runtime.env"
    grep -Fxq "GITHUB_TOKEN='runtime-github-value'" "$TEST_ROOT/runtime.env"
    grep -Fxq "BACKEND_ADMIN_URL='http://schoolorbit-backend-admin:8080'" "$TEST_ROOT/runtime.env"
    grep -Fxq "BACKEND_SCHOOL_URL='http://schoolorbit-backend-school:8081'" "$TEST_ROOT/runtime.env"
    grep -Fxq "API_URL='https://school-api.schoolorbit.app'" "$TEST_ROOT/runtime.env"
    grep -Fxq "VAPID_SUBJECT='mailto:admin@schoolorbit.app'" "$TEST_ROOT/runtime.env"
}

@test "runtime environment is streamed to an atomic mode-0600 target" {
    local database_scheme=postgresql
    local database_url="${database_scheme}://schoolorbit:DbPass@db.invalid/schoolorbit"

    vps_install_runtime_env

    grep -Fq "mktemp /opt/stack/.env" "$FAKE_COMMAND_LOG"
    grep -Fq 'chmod 0600' "$FAKE_COMMAND_LOG"
    grep -R -Fq "DATABASE_URL='$database_url'" "$TEST_ROOT"/ssh-stdin-*
    ! grep -R -Fq must-not-reach-vps "$TEST_ROOT"/ssh-stdin-*
}

@test "Origin TLS verifies the pinned root and streams certificate key and root modes" {
    make_fake_command curl '
set -eu
output=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --output) output=$2; shift 2 ;;
        *) shift ;;
    esac
done
printf "%s\n" "fixture-cloudflare-origin-root" >"$output"
'
    make_fake_command openssl '
set -eu
case "${1-}" in
    req)
        while [ "$#" -gt 0 ]; do
            case "$1" in
                -keyout) key=$2; shift 2 ;;
                -out) csr=$2; shift 2 ;;
                *) shift ;;
            esac
        done
        printf "%s\n" "fixture-tls-private-key" >"$key"
        printf "%s\n" "fixture-csr" >"$csr"
        ;;
    verify) exit 0 ;;
    x509)
        case "$*" in *-pubkey*) printf "%s\n" "fixture-public-key" ;; *) exit 0 ;; esac
        ;;
    pkey) printf "%s\n" "fixture-key-der" ;;
esac
'
    make_fake_command sha256sum '
if [ "$#" -gt 0 ]; then
    printf "%s  %s\n" "91a8a5567efa6bf941162aa806b3ba476aaddf7867640e53053b35fb225a5dae" "$1"
else
    cat >/dev/null
    printf "%s  -\n" "fixture-key-hash"
fi
'
    cf_issue_origin_certificate() {
        CF_CERTIFICATE=$'-----BEGIN CERTIFICATE-----\nfixture-certificate\n-----END CERTIFICATE-----'
    }

    vps_issue_and_install_tls

    [ -r "$SO_CF_ORIGIN_ROOT_FILE" ]
    grep -Fq 'schoolorbit-origin.pem 0644' "$FAKE_COMMAND_LOG"
    grep -Fq 'schoolorbit-origin.key 0600' "$FAKE_COMMAND_LOG"
    grep -Fq 'cloudflare-origin-rsa-root.pem 0644' "$FAKE_COMMAND_LOG"
    run grep -F 'fixture-tls-private-key' "$FAKE_COMMAND_LOG"
    [ "$status" -eq 1 ]
}
