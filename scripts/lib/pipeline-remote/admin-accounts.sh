#!/usr/bin/env bash
# Pipeline-only account checks/bootstrap. Never pass credentials as arguments.
set -euo pipefail
mode=${1:?Account release mode is required}
case "$mode" in
    --check-deploy-inputs | --prepare-release | --check-release-ready) ;;
    *)
        echo 'Unknown account release mode' >&2
        exit 64
        ;;
esac
: "${BASE_DOMAIN:?}"
: "${BACKEND_IMAGE_DIGEST:?}"
[[ $BACKEND_IMAGE_DIGEST =~ ^sha256:[0-9a-f]{64}$ ]] || exit 64
stack=${SCHOOLORBIT_STACK_ROOT:-/opt/stack}
image="ghcr.io/akephisit/schoolorbit-backend-admin@${BACKEND_IMAGE_DIGEST}"
test -r "$stack/postgresql/ca.crt"
# shellcheck source=scripts/lib/pipeline-remote/runtime-env.sh
. "$(dirname "$0")/runtime-env.sh"
ADMIN_MIGRATION_DATABASE_URL=$(schoolorbit_runtime_env_value ADMIN_MIGRATION_DATABASE_URL "$stack/.env")
RESEND_API_KEY=$(schoolorbit_runtime_env_value RESEND_API_KEY "$stack/.env")
RESEND_FROM=$(schoolorbit_runtime_env_value RESEND_FROM "$stack/.env")
export ADMIN_MIGRATION_DATABASE_URL RESEND_API_KEY RESEND_FROM
ADMIN_BOOTSTRAP_EMAIL=${ADMIN_BOOTSTRAP_EMAIL:-}
ADMIN_BOOTSTRAP_PASSWORD=${ADMIN_BOOTSTRAP_PASSWORD:-}
ADMIN_BOOTSTRAP_NAME=SchoolOrbit
export ADMIN_BOOTSTRAP_EMAIL ADMIN_BOOTSTRAP_PASSWORD ADMIN_BOOTSTRAP_NAME
if [[ $mode == --check-deploy-inputs ]]; then
    : "${REGISTRY_TOKEN:?}"
    : "${REGISTRY_USER:?}"
    printf '%s' "$REGISTRY_TOKEN" | podman login ghcr.io -u "$REGISTRY_USER" --password-stdin
    podman pull "$image"
fi
podman run --rm --network schoolorbit-web \
    --env ADMIN_MIGRATION_DATABASE_URL --env BASE_DOMAIN \
    --env RESEND_API_KEY --env RESEND_FROM \
    --env ADMIN_BOOTSTRAP_EMAIL --env ADMIN_BOOTSTRAP_PASSWORD --env ADMIN_BOOTSTRAP_NAME \
    --volume "$stack/postgresql/ca.crt:/run/secrets/postgresql-ca.crt:ro" \
    "$image" /app/create_admin "$mode"
