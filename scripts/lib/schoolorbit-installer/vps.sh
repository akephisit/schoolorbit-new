#!/usr/bin/env bash

VPS_MODULE_DIRECTORY=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
SCHOOLORBIT_REPOSITORY_ROOT=${REPOSITORY_ROOT:-$(cd -- "$VPS_MODULE_DIRECTORY/../../.." && pwd)}
CF_ORIGIN_RSA_ROOT_URL=https://developers.cloudflare.com/ssl/static/origin_ca_rsa_root.pem
CF_ORIGIN_RSA_ROOT_SHA256=91a8a5567efa6bf941162aa806b3ba476aaddf7867640e53053b35fb225a5dae
SO_VPS_DEPLOYMENT_KEY_DIR=
SO_VPS_TLS_TEMP_DIR=

_vps_ssh() {
    ssh \
        -p "${SO_CONFIG[ssh_port]}" \
        -o BatchMode=yes \
        -o StrictHostKeyChecking=yes \
        "${SO_CONFIG[bootstrap_user]}@${SO_CONFIG[target]}" \
        "$@"
}

_vps_privileged_prefix() {
    if [[ ${SO_CONFIG[bootstrap_user]} == root ]]; then
        printf '%s\n' ''
    else
        printf '%s\n' 'sudo -n '
    fi
}

remote_os_supported() {
    local os_release_file=$1 distribution
    [[ -r $os_release_file ]] || die 69 'Target OS metadata is not readable' || return
    distribution=$(sed -n 's/^ID=//p' "$os_release_file" | head -n 1)
    distribution=${distribution#\"}
    distribution=${distribution%\"}
    case "$distribution" in
        debian | ubuntu) return 0 ;;
        *) die 69 'Target must run Debian or Ubuntu' ;;
    esac
}

vps_preflight() {
    require_command ssh || return
    local os_release remote_port_check
    os_release=$(mktemp "${TMPDIR:-/tmp}/schoolorbit-os-release.XXXXXX")
    chmod 0600 "$os_release"
    if ! _vps_ssh 'cat /etc/os-release' >"$os_release"; then
        command rm -f "$os_release"
        die 69 'Unable to connect to the target VPS with strict host-key checking'
        return
    fi
    if ! remote_os_supported "$os_release"; then
        command rm -f "$os_release"
        return 69
    fi
    command rm -f "$os_release"

    if [[ ${SO_CONFIG[bootstrap_user]} != root ]]; then
        _vps_ssh 'sudo -n true' >/dev/null || die 69 'Bootstrap user requires passwordless sudo' || return
    fi

    # shellcheck disable=SC2016 # The awk field expression expands only on the target.
    printf -v remote_port_check \
        'ss -ltnH | awk -v port=":%s" '\''$4 ~ port "$" { found=1 } END { exit !found }'\''' \
        "${SO_CONFIG[ssh_port]}"
    _vps_ssh "$remote_port_check" >/dev/null || die 69 'Configured SSH port is not listening on the target'
}

vps_bootstrap() {
    local prefix remote_command bootstrap_script
    prefix=$(_vps_privileged_prefix)
    remote_command="${prefix}bash -s -- ${SO_CONFIG[ssh_port]} ${SO_CONFIG[server_user]}"
    bootstrap_script="$SCHOOLORBIT_REPOSITORY_ROOT/scripts/lib/schoolorbit-installer/remote/bootstrap.sh"
    [[ -r $bootstrap_script ]] || die 69 'Remote bootstrap script is missing' || return
    _vps_ssh "$remote_command" <"$bootstrap_script" || die 69 'VPS bootstrap failed' || return
    _vps_ssh true >/dev/null || die 69 'Fresh SSH session failed after VPS bootstrap'
}

_vps_make_private_temp_dir() {
    local parent candidate
    for candidate in "${XDG_RUNTIME_DIR-}" /dev/shm "${TMPDIR:-/tmp}"; do
        [[ -n $candidate && -d $candidate && -w $candidate ]] || continue
        parent=$candidate
        break
    done
    [[ -n ${parent-} ]] || die 69 'No writable private temporary directory is available' || return
    candidate=$(mktemp -d "$parent/schoolorbit-installer.XXXXXX") || return
    chmod 0700 "$candidate"
    printf '%s\n' "$candidate"
}

_vps_remove_private_temp_dir() {
    local directory=${1-}
    [[ -n $directory && -d $directory ]] || return 0
    case ${directory##*/} in
        schoolorbit-installer.*) command rm -rf -- "$directory" ;;
        *) die 78 'Refusing to remove an unexpected temporary directory' ;;
    esac
}

vps_cleanup_transients() {
    _vps_remove_private_temp_dir "$SO_VPS_DEPLOYMENT_KEY_DIR"
    _vps_remove_private_temp_dir "$SO_VPS_TLS_TEMP_DIR"
    SO_VPS_DEPLOYMENT_KEY_DIR=
    SO_VPS_TLS_TEMP_DIR=
}

vps_create_deployment_key() {
    require_command ssh-keygen || return
    local private_key public_key prefix remote_command
    SO_VPS_DEPLOYMENT_KEY_DIR=$(_vps_make_private_temp_dir) || return
    private_key="$SO_VPS_DEPLOYMENT_KEY_DIR/github-actions"
    ssh-keygen -q -t ed25519 -N '' -C schoolorbit-github-actions -f "$private_key" || die 69 'Unable to generate the deployment SSH key' || return
    public_key=$(<"$private_key.pub")
    [[ $public_key =~ ^ssh-ed25519[[:space:]][A-Za-z0-9+/=]+[[:space:]]schoolorbit-github-actions$ ]] || die 78 'Generated deployment public key has an unexpected format' || return
    SO_SECRETS[SSH_PRIVATE_KEY]=$(<"$private_key")

    prefix=$(_vps_privileged_prefix)
    remote_command="${prefix}bash -s -- ${SO_CONFIG[server_user]}"
    {
        printf 'set -euo pipefail\n'
        printf "PUBLIC_KEY='%s'\n" "$public_key"
        cat <<'REMOTE_SCRIPT'
SERVER_USER=$1
SERVER_HOME=$(getent passwd "$SERVER_USER" | cut -d: -f6)
[[ -n $SERVER_HOME ]] || exit 69
install -d -m 0700 -o "$SERVER_USER" -g "$SERVER_USER" "$SERVER_HOME/.ssh"
AUTHORIZED_KEYS="$SERVER_HOME/.ssh/authorized_keys"
touch "$AUTHORIZED_KEYS"
chown "$SERVER_USER:$SERVER_USER" "$AUTHORIZED_KEYS"
chmod 0600 "$AUTHORIZED_KEYS"
grep -Fqx -- "$PUBLIC_KEY" "$AUTHORIZED_KEYS" || printf '%s\n' "$PUBLIC_KEY" >>"$AUTHORIZED_KEYS"
REMOTE_SCRIPT
    } | _vps_ssh "$remote_command" || die 69 'Unable to install the deployment public key'
}

_dotenv_line() {
    local name=$1 value=$2
    [[ $name =~ ^[A-Z][A-Z0-9_]*$ ]] || die 64 'Invalid runtime environment name' || return
    [[ $value != *$'\n'* && $value != *$'\r'* ]] || die 64 "Runtime value for $name contains a newline" || return
    value=${value//\\/\\\\}
    value=${value//\'/\\\'}
    printf "%s='%s'\n" "$name" "$value"
}

render_runtime_env() {
    [[ -n $SO_CF_ZONE_ID ]] || die 78 'Cloudflare zone ID is not loaded' || return
    [[ -n ${SO_CF_ACCOUNT_ID-} ]] || die 78 'Cloudflare account ID is not loaded' || return
    _dotenv_line DATABASE_URL "${SO_SECRETS[DATABASE_URL]-}" || return
    _dotenv_line JWT_SECRET "${SO_SECRETS[JWT_SECRET]-}" || return
    _dotenv_line INTERNAL_API_SECRET "${SO_SECRETS[INTERNAL_API_SECRET]-}" || return
    _dotenv_line ENCRYPTION_KEY "${SO_SECRETS[ENCRYPTION_KEY]-}" || return
    _dotenv_line BLIND_INDEX_KEY "${SO_SECRETS[BLIND_INDEX_KEY]-}" || return
    _dotenv_line DEPLOY_KEY "${SO_SECRETS[DEPLOY_KEY]-}" || return
    _dotenv_line BACKEND_ADMIN_URL http://schoolorbit-backend-admin:8080 || return
    _dotenv_line BACKEND_SCHOOL_URL http://schoolorbit-backend-school:8081 || return
    _dotenv_line API_URL "https://school-api.${SO_CONFIG[base_domain]}" || return
    _dotenv_line NEON_API_KEY "${SO_SECRETS[NEON_API_KEY]-}" || return
    _dotenv_line NEON_PROJECT_ID "${SO_CONFIG["runtime:NEON_PROJECT_ID"]-}" || return
    _dotenv_line NEON_BRANCH_ID main || return
    _dotenv_line NEON_HOST "${SO_CONFIG["runtime:NEON_HOST"]-}" || return
    _dotenv_line NEON_DB_PASSWORD "${SO_SECRETS[NEON_DB_PASSWORD]-}" || return
    _dotenv_line CLOUDFLARE_API_TOKEN "${SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_RUNTIME_TOKEN]-}" || return
    _dotenv_line CLOUDFLARE_ZONE_ID "$SO_CF_ZONE_ID" || return
    _dotenv_line CLOUDFLARE_ACCOUNT_ID "$SO_CF_ACCOUNT_ID" || return
    _dotenv_line BASE_DOMAIN "${SO_CONFIG[base_domain]}" || return
    _dotenv_line GITHUB_TOKEN "${SO_SECRETS[SCHOOLORBIT_RUNTIME_GITHUB_TOKEN]-}" || return
    _dotenv_line GITHUB_REPO "${SO_CONFIG[repository]}" || return
    _dotenv_line GITHUB_REPOSITORY "${SO_CONFIG[repository]}" || return
    _dotenv_line R2_ACCOUNT_ID "${SO_CONFIG["runtime:R2_ACCOUNT_ID"]-}" || return
    _dotenv_line R2_ACCESS_KEY_ID "${SO_SECRETS[R2_ACCESS_KEY_ID]-}" || return
    _dotenv_line R2_SECRET_ACCESS_KEY "${SO_SECRETS[R2_SECRET_ACCESS_KEY]-}" || return
    _dotenv_line R2_PUBLIC_BUCKET_NAME "${SO_CONFIG["runtime:R2_PUBLIC_BUCKET_NAME"]-}" || return
    _dotenv_line R2_PRIVATE_BUCKET_NAME "${SO_CONFIG["runtime:R2_PRIVATE_BUCKET_NAME"]-}" || return
    _dotenv_line R2_PUBLIC_URL "${SO_CONFIG["runtime:R2_PUBLIC_URL"]-}" || return
    _dotenv_line R2_REGION auto || return
    _dotenv_line VAPID_PUBLIC_KEY "${SO_CONFIG["runtime:VAPID_PUBLIC_KEY"]-}" || return
    _dotenv_line VAPID_PRIVATE_KEY "${SO_SECRETS[VAPID_PRIVATE_KEY]-}" || return
    _dotenv_line VAPID_SUBJECT "mailto:admin@${SO_CONFIG[base_domain]}"
}

_vps_remote_bash_command() {
    local script=$1
    shift
    local quoted_script prefix argument command
    printf -v quoted_script '%q' "$script"
    prefix=$(_vps_privileged_prefix)
    command="${prefix}bash -c $quoted_script _"
    for argument in "$@"; do
        printf -v argument '%q' "$argument"
        command+=" $argument"
    done
    printf '%s\n' "$command"
}

vps_install_runtime_env() {
    local remote_script remote_command
    # shellcheck disable=SC2016 # This script is intentionally expanded only by remote Bash.
    remote_script='set -euo pipefail
target=/opt/stack/.env
temporary=$(mktemp /opt/stack/.env.XXXXXX)
trap '\''rm -f "$temporary"'\'' EXIT
cat >"$temporary"
for name in DATABASE_URL JWT_SECRET INTERNAL_API_SECRET ENCRYPTION_KEY BLIND_INDEX_KEY DEPLOY_KEY NEON_API_KEY NEON_PROJECT_ID NEON_HOST NEON_DB_PASSWORD CLOUDFLARE_API_TOKEN CLOUDFLARE_ZONE_ID CLOUDFLARE_ACCOUNT_ID BASE_DOMAIN GITHUB_TOKEN GITHUB_REPO R2_ACCOUNT_ID R2_ACCESS_KEY_ID R2_SECRET_ACCESS_KEY R2_PUBLIC_BUCKET_NAME R2_PRIVATE_BUCKET_NAME R2_PUBLIC_URL VAPID_PUBLIC_KEY VAPID_PRIVATE_KEY; do
    grep -Eq "^${name}=" "$temporary" || exit 78
done
chmod 0600 "$temporary"
chown "$1:$1" "$temporary"
mv "$temporary" "$target"
trap - EXIT'
    remote_command=$(_vps_remote_bash_command "$remote_script" "${SO_CONFIG[server_user]}") || return
    render_runtime_env | _vps_ssh "$remote_command" || die 69 'Unable to install the VPS runtime environment'
}

_vps_stream_file() {
    local source_file=$1 destination=$2 mode=$3 remote_script remote_command
    [[ -r $source_file ]] || die 69 'Local TLS source file is unreadable' || return
    # shellcheck disable=SC2016 # This script is intentionally expanded only by remote Bash.
    remote_script='set -euo pipefail
destination=$1
mode=$2
owner=$3
temporary=$(mktemp "${destination}.XXXXXX")
trap '\''rm -f "$temporary"'\'' EXIT
cat >"$temporary"
chmod "$mode" "$temporary"
chown "$owner:$owner" "$temporary"
mv "$temporary" "$destination"
trap - EXIT'
    remote_command=$(_vps_remote_bash_command "$remote_script" "$destination" "$mode" "${SO_CONFIG[server_user]}") || return
    _vps_ssh "$remote_command" <"$source_file" || die 69 "Unable to install TLS file ${destination##*/}"
}

vps_issue_and_install_tls() {
    require_command curl || return
    require_command openssl || return
    local private_key csr certificate root root_hash certificate_key_hash private_key_hash admin_host school_host
    SO_VPS_TLS_TEMP_DIR=$(_vps_make_private_temp_dir) || return
    private_key="$SO_VPS_TLS_TEMP_DIR/schoolorbit-origin.key"
    csr="$SO_VPS_TLS_TEMP_DIR/schoolorbit-origin.csr"
    certificate="$SO_VPS_TLS_TEMP_DIR/schoolorbit-origin.pem"
    root="$SO_VPS_TLS_TEMP_DIR/cloudflare-origin-rsa-root.pem"
    admin_host="admin-api.${SO_CONFIG[base_domain]}"
    school_host="school-api.${SO_CONFIG[base_domain]}"

    openssl req -new -newkey rsa:2048 -nodes \
        -keyout "$private_key" -out "$csr" \
        -subj "/CN=$admin_host" \
        -addext "subjectAltName=DNS:$admin_host,DNS:$school_host" >/dev/null 2>&1 || die 69 'Unable to generate the Origin CA key and CSR' || return
    chmod 0600 "$private_key"
    cf_issue_origin_certificate "$csr" || return
    printf '%s\n' "$CF_CERTIFICATE" >"$certificate"
    chmod 0644 "$certificate"

    curl --silent --show-error --fail --location "$CF_ORIGIN_RSA_ROOT_URL" --output "$root" || die 69 'Unable to download the Cloudflare Origin CA root' || return
    root_hash=$(sha256sum "$root" | awk '{print $1}')
    [[ $root_hash == "$CF_ORIGIN_RSA_ROOT_SHA256" ]] || die 78 'Cloudflare Origin CA root checksum is invalid' || return
    openssl verify -CAfile "$root" "$certificate" >/dev/null || die 78 'Origin certificate verification failed' || return
    openssl x509 -in "$certificate" -noout -checkhost "$admin_host" >/dev/null || die 78 'Origin certificate is missing the admin API host' || return
    openssl x509 -in "$certificate" -noout -checkhost "$school_host" >/dev/null || die 78 'Origin certificate is missing the school API host' || return
    certificate_key_hash=$(openssl x509 -in "$certificate" -pubkey -noout | openssl pkey -pubin -outform DER | sha256sum | awk '{print $1}') || return 78
    private_key_hash=$(openssl pkey -in "$private_key" -pubout -outform DER | sha256sum | awk '{print $1}') || return 78
    [[ $certificate_key_hash == "$private_key_hash" ]] || die 78 'Origin certificate and private key do not match' || return

    _vps_stream_file "$certificate" /opt/stack/nginx/ssl/schoolorbit-origin.pem 0644 || return
    _vps_stream_file "$private_key" /opt/stack/nginx/ssl/schoolorbit-origin.key 0600 || return
    _vps_stream_file "$root" /opt/stack/nginx/ssl/cloudflare-origin-rsa-root.pem 0644 || return
    _vps_remove_private_temp_dir "$SO_VPS_TLS_TEMP_DIR"
    SO_VPS_TLS_TEMP_DIR=
}
