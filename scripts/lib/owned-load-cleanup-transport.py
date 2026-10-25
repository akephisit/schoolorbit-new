"""Assemble trusted repository source, never code from the private data bundle."""
import base64
import hashlib
from pathlib import Path


def render_transport(proof, controller, common, cloudflare, verifier):
    encode = lambda raw: base64.b64encode(raw).decode()
    bootstrap = 'import base64,hashlib,os,sys,types\n'
    bootstrap += f"proof=base64.b64decode('{encode(proof)}');controller=base64.b64decode('{encode(controller)}')\n"
    bootstrap += "if hashlib.sha256(proof).hexdigest()!=os.environ['CLEANUP_PROOF_SHA256']: raise SystemExit('Trusted proof source mismatch')\n"
    bootstrap += "if hashlib.sha256(controller).hexdigest()!=os.environ['CLEANUP_CONTROLLER_SHA256']: raise SystemExit('Trusted controller source mismatch')\n"
    bootstrap += "module=types.ModuleType('owned_cleanup_proof');sys.modules[module.__name__]=module\n"
    bootstrap += "exec(compile(proof,'owned-load-cleanup-proof.py','exec'),module.__dict__)\n"
    bootstrap += "exec(compile(controller,'owned-load-cleanup.py','exec'),{'__name__':'__main__'})\n"
    body = '#!/usr/bin/env bash\nset -euo pipefail\n'
    body += "cleanup_controller() {\n env -u API_TLS_CLOUDFLARE_TOKEN -u SMOKE_USERNAME -u SMOKE_PASSWORD python3 - <<'OWNED_CONTROLLER'\n"
    body += bootstrap + 'OWNED_CONTROLLER\n}\n'
    body += 'dns_owner_verify() (\n set -euo pipefail\n'
    owner_hash = hashlib.sha256(common + cloudflare + verifier).hexdigest()
    body += f' [[ $CLEANUP_DNS_OWNER_SHA256 == {owner_hash} ]] || exit 78\n'
    for label, raw in [('COMMON', common), ('CLOUDFLARE', cloudflare), ('VERIFIER', verifier)]:
        marker = 'OWNED_DNS_' + label
        source = raw.decode()
        if marker in source.splitlines():
            raise ValueError('Trusted source delimiter collision')
        body += f" source /dev/stdin <<'{marker}'\n" + source + '\n' + marker + '\n'
    body += ' api_dns_verify_only\n)\n'
    body += '''case "$CLEANUP_ACTION:$CLEANUP_PHASE" in
 execute:prepare|execute:commit)
  dns_owner_verify
  cleanup_controller
  ;;
 reconcile:reconcile)
  status=0
  CLEANUP_PHASE=classify-recovery cleanup_controller || status=$?
  case "$status" in
   0) exit 0 ;;
   10) dns_owner_verify; cleanup_controller ;;
   *) exit "$status" ;;
  esac
  ;;
 *) exit 64 ;;
esac
'''
    return body


if __name__ == '__main__':
    import os
    root = Path.cwd()
    read = lambda file: (root / file).read_bytes()
    output = Path(os.environ['RUNNER_TEMP']) / 'owned-load-cleanup.sh'
    output.write_text(render_transport(
        read('scripts/lib/owned-load-cleanup-proof.py'),
        read('scripts/lib/pipeline-remote/owned-load-cleanup.py'),
        read('scripts/lib/schoolorbit-installer/common.sh'),
        read('scripts/lib/schoolorbit-installer/cloudflare.sh'),
        read('scripts/lib/pipeline-remote/api-dns-verification.sh')))
    output.chmod(0o600)
