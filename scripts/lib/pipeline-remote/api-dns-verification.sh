#!/usr/bin/env bash
# Source-only public DNS verification owner. Callers hold schoolorbit-production.

api_dns_public_smoke() {
    local deployment=$1 domain=$2 output status=0 script
    local -a copy_environment=()
    script="$deployment/scripts/smoke_test.sh"
    if (($# == 3)); then
        script=$3
        copy_environment=(SMOKE_ENV_FILE=/dev/null FILE_SMOKE_PNG=)
    fi
    output=$(mktemp "${TMPDIR:-/tmp}/schoolorbit-public-smoke.XXXXXX") || return
    chmod 0600 "$output" || return
    SMOKE_SUBDOMAIN=sandbox SMOKE_API_URL="https://school-api.$domain" \
        SMOKE_ADMIN_API_URL="https://admin-api.$domain" SMOKE_TENANT_URL="https://sandbox.$domain" \
        SMOKE_ORIGIN="https://sandbox.$domain" SMOKE_REQUIRE_AUTH=true SMOKE_ACADEMIC_CONTEXT=true \
        SMOKE_DIRECT_BACKEND=false SMOKE_RESOLVE_IP='' SMOKE_CA_CERT='' \
        SMOKE_RELEASE_PROBE_TOKEN='' env "${copy_environment[@]}" bash "$script" >"$output" 2>&1 || status=69
    rm -f "$output"
    if ((status != 0)); then
        echo 'Public authenticated full-context verification failed.' >&2
    fi
    return "$status"
}

_api_dns_verification_state() {
    python3 -B - "$1" "${2-}" <<'PY'
import base64
from datetime import datetime, timezone
import hashlib
import ipaddress
import json
import os
from pathlib import Path
import re
import stat
import sys


def require(ok):
    if not ok:
        raise ValueError('refused')


def directory(path, private=False, create=False):
    if create and not os.path.lexists(path):
        path.mkdir(mode=0o700)
    s = path.lstat()
    require(stat.S_ISDIR(s.st_mode))
    if private:
        require(s.st_uid == os.getuid() and stat.S_IMODE(s.st_mode) == 0o700)


def record(path, private=True):
    s = path.lstat()
    require(stat.S_ISREG(s.st_mode) and s.st_uid == os.getuid() and s.st_size <= 2 * 1024 * 1024)
    if private:
        require(stat.S_IMODE(s.st_mode) == 0o600)
    raw = path.read_bytes()
    value = json.loads(raw)
    require(type(value) is dict)
    return value, hashlib.sha256(raw).hexdigest()


def text(value, pattern):
    return type(value) is str and re.fullmatch(pattern, value) is not None


def journal_shape(value, legacy=False):
    keys = {'accepted_app_sha', 'base_domain', 'target_ip', 'zone', 'account', 'snapshot_etag', 'snapshot', 'status'}
    if not legacy:
        keys |= {'schema_version', 'owner', 'request_id'}
    require(value.keys() == keys)
    require(text(value['accepted_app_sha'], '[0-9a-f]{40}'))
    require(value['base_domain'] == os.environ['BASE_DOMAIN'])
    require(str(ipaddress.IPv4Address(value['target_ip'])) == value['target_ip'])
    require(value['target_ip'] == os.environ['TARGET_ORIGIN_IP'])
    require(text(value['snapshot_etag'], '[0-9a-f]{64}'))
    for key in ['zone', 'account']:
        require(text(value[key], '[A-Za-z0-9_-]{1,128}'))
    rows = value['snapshot']
    require(type(rows) is list and len(rows) == 2)
    names = set()
    ids = set()
    for row in rows:
        require(type(row) is dict and row.keys() == {'id', 'type', 'name', 'content', 'ttl', 'proxied', 'modified_on'})
        require(text(row['id'], '[A-Za-z0-9_-]{1,128}') and row['id'] not in ids)
        ids.add(row['id'])
        require(row['type'] == 'A' and row['name'] in ['admin-api.' + value['base_domain'], 'school-api.' + value['base_domain']])
        require(row['name'] not in names)
        names.add(row['name'])
        require(type(row['content']) is str and str(ipaddress.IPv4Address(row['content'])) == row['content'])
        require(type(row['ttl']) is int and 1 <= row['ttl'] <= 2147483647)
        require(type(row['proxied']) is bool and type(row['modified_on']) is str and 0 < len(row['modified_on']) <= 64)
    if legacy:
        require(value['status'] == 'applied')
    else:
        require(type(value['schema_version']) is int and value['schema_version'] == 1 and value['owner'] == 'public-api-dns')
        require(text(value['request_id'], '[0-9]+-[0-9]+') and value['status'] in ['accepted', 'rolled_back'])


try:
    raw = base64.b64decode(os.environ['CLEANUP_CATALOGUE'], validate=True)
    require(len(raw) <= 65536 and hashlib.sha256(raw).hexdigest() == os.environ['CLEANUP_CATALOGUE_SHA256'])
    cat = json.loads(raw)
    require(type(cat) is dict and type(cat.get('schemaVersion')) is int and cat['schemaVersion'] == 1)
    operation = cat['operationId']
    require(text(operation, 'cleanup-[0-9a-f]{8}-[0-9a-f-]{36}'))
    require(text(cat['inputSha256'], '[0-9a-f]{64}'))
    legacy = cat['legacyDns']
    require(type(legacy) is dict and text(legacy['filename'], '[0-9]+-[0-9]+[.]json') and text(legacy['sha256'], '[0-9a-f]{64}'))
    require(type(legacy['successfulNoopRun']) is int and legacy['successfulNoopRun'] > 0)
    require(type(legacy['successfulNoopJob']) is int and legacy['successfulNoopJob'] > 0)
    require(text(legacy['successfulNoopSource'], '[0-9a-f]{40}'))
    phase = os.environ['CLEANUP_PHASE']
    require(phase in ['prepare', 'commit', 'reconcile'])
    run = os.environ['CLEANUP_RUN_ID']
    attempt = os.environ['CLEANUP_RUN_ATTEMPT']
    require(text(run, '[0-9]{1,20}') and text(attempt, '[0-9]{1,10}'))
    accepted = os.environ['ACCEPTED_APP_SHA']
    require(text(accepted, '[0-9a-f]{40}'))
    root = Path(os.environ.get('SCHOOLORBIT_STACK_ROOT', '/opt/stack')) / 'deployment'
    require(root.is_absolute())
    for parent in reversed((root, *root.parents)):
        directory(parent)
    if sys.argv[1] == 'stage':
        temporary = Path(os.environ['TMPDIR'])
        directory(temporary, True)
        for name, variable, copied in [('lib/pipeline-remote/api-tls.sh', 'DNS_TLS_OWNER_SHA256', 'api-tls-status.sh'),
                                       ('smoke_test.sh', 'DNS_SMOKE_OWNER_SHA256', 'public-smoke.sh')]:
            expected = os.environ[variable]
            require(text(expected, '[0-9a-f]{64}'))
            script = root / 'scripts' / name
            for parent in script.parents:
                if parent == root:
                    break
                directory(parent)
            fd = os.open(script, os.O_RDONLY | os.O_NOFOLLOW)
            with os.fdopen(fd, 'rb') as stream:
                metadata = os.fstat(stream.fileno())
                require(stat.S_ISREG(metadata.st_mode) and metadata.st_uid == os.getuid() and metadata.st_size <= 2 * 1024 * 1024)
                verified = stream.read(2 * 1024 * 1024 + 1)
            require(len(verified) <= 2 * 1024 * 1024 and hashlib.sha256(verified).hexdigest() == expected)
            fd = os.open(temporary / copied, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            with os.fdopen(fd, 'wb') as stream:
                stream.write(verified)
    accepted_path = root / 'accepted-release'
    s = accepted_path.lstat()
    require(stat.S_ISREG(s.st_mode) and s.st_uid == os.getuid() and s.st_size <= 41)
    require(accepted_path.read_text() in [accepted, accepted + '\n'])
    require(not os.path.lexists(root / 'pending-release'))
    tls = root / 'api-tls-operation.json'
    if os.path.lexists(tls):
        value, _ = record(tls)
        require(value.get('phase') == 'accepted')
    dns = root / 'api-dns'
    directory(dns, True)
    known, known_hash = record(dns / legacy['filename'])
    require(known_hash == legacy['sha256'])
    journal_shape(known, True)
    paths = list(dns.iterdir())
    require(len(paths) <= 1000)
    for path in paths:
        if path.name == legacy['filename']:
            continue
        if re.fullmatch('[0-9]+-[0-9]+[.]pending-release', path.name):
            s = path.lstat()
            require(stat.S_ISREG(s.st_mode) and s.st_uid == os.getuid() and stat.S_IMODE(s.st_mode) == 0o600 and s.st_size <= 41)
            require(text(path.read_text().strip(), '[0-9a-f]{40}'))
            continue
        require(re.fullmatch('[0-9]+-[0-9]+[.]json', path.name) is not None)
        value, _ = record(path)
        journal_shape(value)
        require(path.name == value['request_id'] + '.json')
        require(value['zone'] == known['zone'] and value['account'] == known['account'])
    cleanup = root / 'owned-load-cleanup'
    if os.path.lexists(cleanup):
        directory(cleanup, True)
        active = cleanup / 'active'
        if os.path.lexists(active):
            require(phase in ['commit', 'reconcile'])
            value, _ = record(active)
            require(type(value.get('schemaVersion')) is int and value['schemaVersion'] == 1)
            require(value.get('operationId') == operation and value.get('inputSha256') == cat['inputSha256'])
            require(value.get('catalogueSha256') == os.environ['CLEANUP_CATALOGUE_SHA256'])
            require(value.get('status') in ['prepared', 'reserved'] and text(value.get('run'), '[0-9]+-[0-9]+'))
            if phase == 'commit':
                require(value['run'] == run + '-' + attempt)
        operations = cleanup / 'operations'
        if os.path.lexists(operations):
            directory(operations, True)
            for path in operations.iterdir():
                directory(path, True)
                require(path.name == operation or not any(path.iterdir()))
    state = {'operationId': operation, 'legacyJournal': {'filename': legacy['filename'], 'sha256': known_hash},
             'zone': known['zone'], 'account': known['account'], 'acceptedRelease': accepted}
    if sys.argv[1] in ['check', 'stage']:
        print(json.dumps(state, sort_keys=True, separators=(',', ':')))
    elif sys.argv[1] == 'write':
        facts = json.loads(Path(sys.argv[2]).read_text())
        require(facts['zone'] == state['zone'] and facts['account'] == state['account'])
        canonical = json.dumps(facts, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
        value = {'schemaVersion': 1, 'owner': 'public-api-dns-verification', 'operationId': operation,
                 'phase': phase, 'runId': run, 'runAttempt': attempt,
                 'observedAt': datetime.now(timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z'),
                 'legacyJournal': state['legacyJournal'], 'acceptedRelease': accepted, 'stableFacts': facts,
                 'stableDnsSha256': hashlib.sha256(canonical).hexdigest(),
                 'checks': {k: True for k in ['provider', 'dns', 'tls', 'authenticatedFullContext', 'pendingAbsent', 'orphansAbsent']}}
        directory(cleanup, True, True)
        directory(cleanup / 'dns-verification', True, True)
        destination = cleanup / 'dns-verification' / operation
        directory(destination, True, True)
        target = destination / (run + '-' + attempt + '-' + phase + '.json')
        fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, 'wb') as stream:
            stream.write((json.dumps(value, sort_keys=True, separators=(',', ':')) + '\n').encode())
            stream.flush()
            os.fsync(stream.fileno())
        for path in [destination, destination.parent, cleanup, root]:
            fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
            try:
                os.fsync(fd)
            finally:
                os.close(fd)
    else:
        raise ValueError('refused')
except (OSError, ValueError, KeyError, TypeError, AttributeError):
    print('Public DNS verification refused: ownership or operation state is invalid.', file=sys.stderr)
    sys.exit(78)
PY
}

_api_dns_verification_orphans_absent() {
    local name status
    for name in schoolorbit-school-rollout schoolorbit-admin-migration schoolorbit-owned-load-cleanup-3962938a; do
        if podman container exists "$name" >/dev/null 2>&1; then
            return 78
        else
            status=$?
            [[ $status == 1 ]] || return 78
        fi
    done
}

_api_dns_verification_facts() {
    jq -ceS --arg zone "$SO_CF_ZONE_ID" --arg account "$SO_CF_ACCOUNT_ID" --arg target "$TARGET_ORIGIN_IP" '
        def identity: type == "string" and test("^[A-Za-z0-9_-]{1,128}$");
        if ($zone | identity) and ($account | identity) and length == 2 and
           ([.[].id] | unique | length == 2) and
           all(.[]; (.id | identity) and .type == "A" and .content == $target and
               .proxied == false and (.ttl | type == "number" and . == floor and . >= 1 and . <= 2147483647))
        then {zone:$zone,account:$account,targetIp:$target,records:(map({id,type,name,content,ttl,proxied}) | sort_by(.name))}
        else error("invalid DNS verification facts") end
    ' <<<"$SO_CF_DNS_RECORDS"
}

api_dns_verify_only() (
    set -euo pipefail
    umask 077
    local root="${SCHOOLORBIT_STACK_ROOT:-/opt/stack}/deployment" runtime temporary before after facts current
    : "${SMOKE_USERNAME:?Authenticated smoke username is required}" "${SMOKE_PASSWORD:?Authenticated smoke password is required}"
    : "${BASE_DOMAIN:?BASE_DOMAIN is required}" "${TARGET_ORIGIN_IP:?Target origin IPv4 is required}"
    [[ $BASE_DOMAIN =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$ ]] || exit 64
    runtime=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
    [[ -d $runtime && ! -L $runtime && -w $runtime && $(stat -c '%a:%u' "$runtime") == "700:$(id -u)" ]] || exit 78
    [[ $(findmnt -n -o FSTYPE --target "$runtime") == tmpfs ]] || exit 78
    temporary=$(mktemp -d "$runtime/schoolorbit-dns-verify.XXXXXX") || exit 78
    trap 'rm -rf "$temporary"' EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM HUP
    export TMPDIR=$temporary
    before=$(_api_dns_verification_state stage) || exit 78
    _api_dns_verification_orphans_absent || exit 78
    declare -A SO_CONFIG=() SO_SECRETS=()
    SO_CONFIG[base_domain]=$BASE_DOMAIN
    # shellcheck disable=SC2034 # Canonical Cloudflare helpers consume these dynamically scoped inputs.
    SO_CONFIG[target]=$TARGET_ORIGIN_IP
    # shellcheck disable=SC2034 # Canonical Cloudflare request owner consumes the private token.
    SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN]=${API_TLS_CLOUDFLARE_TOKEN:?DNS token is required}
    unset API_TLS_CLOUDFLARE_TOKEN
    # shellcheck disable=SC2034 # Pin the canonical Cloudflare request owner to its production endpoint.
    CF_API_BASE_URL=https://api.cloudflare.com/client/v4
    cf_dns_preflight >"$temporary/provider.log" 2>&1 || exit 78
    facts=$(_api_dns_verification_facts 2>"$temporary/facts.log") || exit 78
    [[ $(jq -r .zone <<<"$before") == "$SO_CF_ZONE_ID" && $(jq -r .account <<<"$before") == "$SO_CF_ACCOUNT_ID" ]] || exit 78
    cf_snapshot_dns >"$temporary/provider.log" 2>&1 || exit 78
    cf_wait_for_target_resolution >"$temporary/dns.log" 2>&1 || exit 78
    bash "$temporary/api-tls-status.sh" status >"$temporary/tls.log" 2>&1 || exit 78
    api_dns_public_smoke "$root" "$BASE_DOMAIN" "$temporary/public-smoke.sh" || exit 69
    cf_assert_cutover_state >"$temporary/provider.log" 2>&1 || exit 78
    cf_dns_preflight >"$temporary/provider.log" 2>&1 || exit 78
    current=$(_api_dns_verification_facts 2>"$temporary/facts.log") || exit 78
    [[ $facts == "$current" ]] || exit 78
    after=$(_api_dns_verification_state check) || exit 78
    [[ $before == "$after" ]] || exit 78
    _api_dns_verification_orphans_absent || exit 78
    printf '%s' "$facts" >"$temporary/facts.json"
    _api_dns_verification_state write "$temporary/facts.json" || exit 78
    echo 'Public DNS-only verification passed; immutable private proof recorded.'
)
