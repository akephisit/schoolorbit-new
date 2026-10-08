#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
shellcheck -x scripts/pipeline scripts/verify_deployment.sh scripts/test_school_database_suite.sh scripts/test_backend_school.sh scripts/test_backend_admin.sh scripts/lib/pipeline-remote/*.sh \
    scripts/schoolorbit-installer scripts/render_nginx_config.sh scripts/reconcile_r2_cors.sh \
    scripts/lib/schoolorbit-installer/*.sh scripts/lib/schoolorbit-installer/remote/*.sh
shfmt -d -i 4 -ci scripts/pipeline scripts/verify_deployment.sh scripts/test_school_database_suite.sh scripts/test_backend_school.sh scripts/test_backend_admin.sh scripts/lib/pipeline-remote/*.sh \
    scripts/schoolorbit-installer scripts/render_nginx_config.sh scripts/reconcile_r2_cors.sh \
    scripts/lib/schoolorbit-installer/*.sh scripts/lib/schoolorbit-installer/remote/*.sh
bats scripts/tests/installer
node --test frontend-school/tests/static/deployment-installer.test.mjs scripts/tests/pipeline*.test.mjs \
    scripts/tests/backend-school-test-database.test.mjs \
    scripts/tests/migration-completion-gate.test.mjs
node --test --test-concurrency=1 scripts/tests/r2-cors.test.mjs scripts/tests/prune-ghcr-versions.test.mjs scripts/tests/worker-release-candidates.test.mjs
# Compose remains the production rootless Podman owner; resolving it is engine-independent.
set -a
# shellcheck disable=SC1091
source scripts/tests/installer/fixtures/runtime.env
set +a
docker compose -f podman-compose.yml config --quiet
tar -cf - .github | docker run --rm -i --entrypoint sh docker.io/rhysd/actionlint:1.7.7 \
    -c 'mkdir -p /tmp/repo/.git; tar -xf - -C /tmp/repo; cd /tmp/repo; actionlint'
