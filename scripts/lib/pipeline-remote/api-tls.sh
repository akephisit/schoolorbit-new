#!/usr/bin/env bash
set -euo pipefail

# This owner is invoked only by the installer or a workflow holding the shared
# production lock. Certbot's live/ directory is deliberately not Nginx's mount.
API_TLS_CERTBOT_IMAGE=docker.io/certbot/dns-cloudflare:v5.8.0@sha256:c45edb002b883da1a1235abb205dff474a7a1a459d878e8d5fdc7f9d83073aea
stack=${SCHOOLORBIT_STACK_ROOT:-/opt/stack}
state="$stack/acme"
ssl="$stack/nginx/ssl"
base=${BASE_DOMAIN:?BASE_DOMAIN is required}
[[ $base =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$ ]] || exit 64
admin_host="admin-api.$base"
school_host="school-api.$base"
operation=${1:?TLS operation is required}
umask 077

validate_pair() {
    local directory=$1 names certificate_key private_key
    test -s "$directory/fullchain.pem"
    test -s "$directory/privkey.pem"
    openssl x509 -in "$directory/fullchain.pem" -noout -checkend 604800 >/dev/null
    names=$(openssl x509 -in "$directory/fullchain.pem" -noout -ext subjectAltName | tail -n +2 | tr -d '[:space:]')
    [[ $names == "DNS:$admin_host,DNS:$school_host" || $names == "DNS:$school_host,DNS:$admin_host" ]] || {
        echo 'Certificate must contain exactly both API DNS names' >&2
        return 78
    }
    openssl verify -purpose sslserver -verify_hostname "$admin_host" \
        -untrusted "$directory/fullchain.pem" "$directory/fullchain.pem" >/dev/null
    openssl x509 -in "$directory/fullchain.pem" -noout -checkhost "$school_host" >/dev/null
    certificate_key=$(openssl x509 -in "$directory/fullchain.pem" -pubkey -noout | openssl pkey -pubin -outform DER | sha256sum)
    private_key=$(openssl pkey -in "$directory/privkey.pem" -pubout -outform DER | sha256sum)
    [[ $certificate_key == "$private_key" ]] || {
        echo 'Certificate and private key do not match' >&2
        return 78
    }
}

metadata() {
    local directory=$1 fingerprint expiry
    fingerprint=$(openssl x509 -in "$directory/fullchain.pem" -outform DER | sha256sum | cut -d ' ' -f 1)
    expiry=$(openssl x509 -in "$directory/fullchain.pem" -noout -enddate | cut -d = -f 2-)
    jq -cn --arg fingerprint "$fingerprint" --arg expiry "$expiry" \
        --arg admin "$admin_host" --arg school "$school_host" \
        '{fingerprint:$fingerprint,certificate_expiry:$expiry,hostnames:[$admin,$school]}'
}

switch_generation() {
    local generation=$1 temporary
    [[ $generation =~ ^api-tls-generations/([0-9a-f]{64}|legacy-[0-9]+)$ ]] || return 64
    test -s "$ssl/$generation/fullchain.pem"
    test -s "$ssl/$generation/privkey.pem"
    temporary="$ssl/.api-tls-next-$$"
    ln -s "$generation" "$temporary"
    mv -Tf "$temporary" "$ssl/.api-tls-current"
}

case "$operation" in
    stage)
        install -d -m 0700 "$state" "$state/config" "$state/work" "$state/logs"
        install -d -m 0750 "$ssl" "$ssl/api-tls-generations"
        # The same path is recorded by Certbot for renewal. Its contents exist
        # only during this invocation, never in persistent renewal configuration.
        runtime=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
        test -d "$runtime" && test -w "$runtime"
        credential_directory=$(mktemp -d "$runtime/schoolorbit-api-tls.XXXXXX")
        credential="$credential_directory/credentials.ini"
        trap 'rm -rf "$credential_directory"' EXIT
        trap 'exit 130' INT
        trap 'exit 143' TERM HUP
        token=$(jq -er '.cloudflare_token | strings | select(length > 0 and (test("[\\r\\n]") | not))')
        printf 'dns_cloudflare_api_token = %s\n' "$token" >"$credential"
        unset token
        chmod 0600 "$credential"
        # Output from the ACME client may contain account details; private logs
        # stay on the host and only bounded status is returned to workflow logs.
        if ! podman run --rm --pull=missing --cap-drop=all --security-opt=no-new-privileges \
            -v "$state/config:/etc/letsencrypt:rw" -v "$state/work:/var/lib/letsencrypt:rw" \
            -v "$state/logs:/var/log/letsencrypt:rw" -v "$credential:/run/api-tls-credentials.ini:ro" "$API_TLS_CERTBOT_IMAGE" \
            certonly --non-interactive --agree-tos --register-unsafely-without-email \
            --dns-cloudflare --dns-cloudflare-credentials /run/api-tls-credentials.ini \
            --dns-cloudflare-propagation-seconds 60 --keep-until-expiring \
            --cert-name schoolorbit-api -d "$admin_host" -d "$school_host" \
            >"$state/logs/last-operation.log" 2>&1; then
            echo 'ACME DNS validation or issuance failed; serving certificate is unchanged' >&2
            exit 69
        fi
        rm -rf "$credential_directory"
        trap - EXIT INT TERM HUP
        source_directory="$state/config/live/schoolorbit-api"
        validate_pair "$source_directory"
        details=$(metadata "$source_directory")
        fingerprint=$(jq -er '.fingerprint' <<<"$details")
        generation="$ssl/api-tls-generations/$fingerprint"
        if [[ ! -d $generation ]]; then
            temporary=$(mktemp -d "$ssl/api-tls-generations/.staged.XXXXXX")
            trap 'rm -rf "$temporary"' EXIT
            install -m 0644 "$source_directory/fullchain.pem" "$temporary/fullchain.pem"
            install -m 0600 "$source_directory/privkey.pem" "$temporary/privkey.pem"
            chmod 0750 "$temporary"
            mv "$temporary" "$generation"
            trap - EXIT
        fi
        printf '%s\n' "$details"
        ;;
    activate)
        fingerprint=${2:?Staged fingerprint is required}
        [[ $fingerprint =~ ^[0-9a-f]{64}$ ]] || exit 64
        generation="api-tls-generations/$fingerprint"
        validate_pair "$ssl/$generation"
        if podman container exists schoolorbit-nginx; then
            test -s "$stack/deployment/pending-release" || {
                echo 'Serving certificate activation requires an active maintenance journal' >&2
                exit 78
            }
        fi
        if [[ ! -L $ssl/.api-tls-current ]]; then
            if [[ -e $ssl/schoolorbit-origin.pem || -e $ssl/schoolorbit-origin.key ]]; then
                test -s "$ssl/schoolorbit-origin.pem"
                test -s "$ssl/schoolorbit-origin.key"
                backup="api-tls-generations/legacy-$(date +%s)"
                install -d -m 0700 "$ssl/$backup"
                install -m 0644 "$ssl/schoolorbit-origin.pem" "$ssl/$backup/fullchain.pem"
                install -m 0600 "$ssl/schoolorbit-origin.key" "$ssl/$backup/privkey.pem"
                switch_generation "$backup"
            else
                switch_generation "$generation"
            fi
        fi
        # Reconcile both links after an interrupted initial conversion. An operator's
        # unrelated replacement must never be overwritten by a retry.
        for pair in schoolorbit-origin.pem:fullchain.pem schoolorbit-origin.key:privkey.pem; do
            filename=${pair%%:*}
            material=${pair#*:}
            if [[ -L $ssl/$filename ]]; then
                test "$(readlink "$ssl/$filename")" = ".api-tls-current/$material" || exit 78
            elif [[ -e $ssl/$filename ]]; then
                cmp -s "$ssl/$filename" "$ssl/.api-tls-current/$material" || exit 78
            fi
            temporary="$ssl/.api-tls-link-$$"
            ln -s ".api-tls-current/$material" "$temporary"
            mv -Tf "$temporary" "$ssl/$filename"
        done
        previous=$(readlink "$ssl/.api-tls-current")
        printf '%s\n' "$previous" >"$state/previous-generation"
        switch_generation "$generation"
        if podman container exists schoolorbit-nginx; then
            if ! podman exec schoolorbit-nginx nginx -t || ! podman exec schoolorbit-nginx nginx -s reload; then
                switch_generation "$previous"
                podman exec schoolorbit-nginx nginx -t && podman exec schoolorbit-nginx nginx -s reload
                echo 'Certificate activation failed; previous pair restored, maintenance retained' >&2
                exit 69
            fi
        fi
        metadata "$ssl/.api-tls-current"
        ;;
    status)
        validate_pair "$ssl/.api-tls-current"
        metadata "$ssl/.api-tls-current"
        ;;
    restore)
        previous=$(cat "$state/previous-generation")
        switch_generation "$previous"
        podman exec schoolorbit-nginx nginx -t
        podman exec schoolorbit-nginx nginx -s reload
        ;;
    *) exit 64 ;;
esac
