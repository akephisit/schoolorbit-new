#!/usr/bin/env bash
set -euo pipefail
: "${RELEASE_SHA:?}"
: "${BASE_DOMAIN:?}"
: "${EXPECTED_DATABASES:?Verified source database count is required}"
[[ $RELEASE_SHA =~ ^[0-9a-f]{40}$ ]] || exit 64
[[ $EXPECTED_DATABASES =~ ^[1-9][0-9]{0,3}$ ]] || exit 64
cd /opt/stack
bash deployment/scripts/lib/pipeline-remote/maintenance.sh enter
podman stop schoolorbit-backend-school schoolorbit-backend-admin >/dev/null
checkpoint="/opt/stack/deployment/releases/$RELEASE_SHA/database-cutover-ready.json"
echo 'Maintenance active; source application writers stopped. Waiting for private data/recovery gates.'
for attempt in {1..270}; do
    if [[ -f $checkpoint && ! -L $checkpoint && $(stat -c %a "$checkpoint") == 600 ]]; then
        if python3 -B - "$checkpoint" "$RELEASE_SHA" "$EXPECTED_DATABASES" <<'PY'; then
import json
import sys
from pathlib import Path
evidence = json.loads(Path(sys.argv[1]).read_text())
passed = (evidence.get('schemaVersion') == 1 and evidence.get('sha') == sys.argv[2]
          and evidence.get('databases') == int(sys.argv[3]) and evidence.get('dataReconciled') is True
          and evidence.get('runtimeTlsVerified') is True and evidence.get('backupRestorePassed') is True)
sys.exit(0 if passed else 1)
PY
            echo 'Database preservation, runtime TLS and actual-data restore gates complete; maintenance remains active for release.'
            exit 0
        fi
        echo 'Cutover evidence failed validation; maintenance remains active.' >&2
        exit 78
    fi
    sleep 20
done
echo "Cutover evidence was not completed after $attempt checks; maintenance remains active and writers remain stopped." >&2
exit 78
