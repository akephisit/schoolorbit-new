"""Trusted transport phases and secret confinement; no network or production access."""
import hashlib
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('owned_cleanup_transport', ROOT / 'scripts/lib/owned-load-cleanup-transport.py')
transport = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(transport)


class TrustedBootstrapTests(unittest.TestCase):
    def run_phase(self, action, phase, classification='0', *, missing_secrets=False, tamper=None):
        proof = b'value=1\n'
        controller = b"""import os,sys
if any(key in os.environ for key in ['API_TLS_CLOUDFLARE_TOKEN','SMOKE_USERNAME','SMOKE_PASSWORD']): raise SystemExit(77)
phase=os.environ['CLEANUP_PHASE']
with open(os.environ['EVENT'], 'a') as output: output.write('controller:'+phase+'\\n')
if phase=='classify-recovery': raise SystemExit(int(os.environ['CLASSIFY']))
"""
        common = b'# trusted common\n'; cloudflare = b'# trusted provider\n'
        verifier = b"""api_dns_verify_only() {
 [[ -n ${API_TLS_CLOUDFLARE_TOKEN:-} && -n ${SMOKE_PASSWORD:-} ]] || return 78
 printf '%s\\n' 'verify:secret-present' >> "$EVENT"
}
"""
        with tempfile.TemporaryDirectory() as directory:
            script = Path(directory) / 'trusted.sh'; event = Path(directory) / 'events'
            script.write_text(transport.render_transport(proof, controller, common, cloudflare, verifier))
            digest = lambda value: hashlib.sha256(value).hexdigest()
            env = {**os.environ, 'CLEANUP_ACTION': action, 'CLEANUP_PHASE': phase, 'EVENT': str(event), 'CLASSIFY': classification,
                'CLEANUP_PROOF_SHA256': digest(proof), 'CLEANUP_CONTROLLER_SHA256': digest(controller), 'CLEANUP_DNS_OWNER_SHA256': digest(common + cloudflare + verifier),
                'API_TLS_CLOUDFLARE_TOKEN': 'fixture-only', 'SMOKE_USERNAME': 'fixture', 'SMOKE_PASSWORD': 'fixture-only', 'PYTHONOPTIMIZE': '1'}
            if missing_secrets:
                for key in ['API_TLS_CLOUDFLARE_TOKEN','SMOKE_USERNAME','SMOKE_PASSWORD']: env.pop(key)
            if tamper: env[tamper] = '0' * 64
            result = subprocess.run(['bash', str(script)], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
            events = event.read_text().splitlines() if event.exists() else []
            self.assertNotIn(b'fixture-only', result.stdout + result.stderr)
            return result.returncode, events

    def test_prepare_and_commit_verify_before_secret_stripped_controller(self):
        for phase in ['prepare', 'commit']:
            code, events = self.run_phase('execute', phase)
            self.assertEqual(code, 0)
            self.assertEqual(events, ['verify:secret-present', 'controller:' + phase])

    def test_no_intent_abort_works_without_cloudflare_or_dns(self):
        code, events = self.run_phase('reconcile', 'reconcile', missing_secrets=True)
        self.assertEqual(code, 0)
        self.assertEqual(events, ['controller:classify-recovery'])

    def test_postintent_recovery_cannot_skip_verifier(self):
        code, events = self.run_phase('reconcile', 'reconcile', '10')
        self.assertEqual(code, 0)
        self.assertEqual(events, ['controller:classify-recovery', 'verify:secret-present', 'controller:reconcile'])
        code, events = self.run_phase('reconcile', 'reconcile', '10', missing_secrets=True)
        self.assertEqual(code, 78)
        self.assertEqual(events, ['controller:classify-recovery'])

    def test_unknown_recovery_does_not_reach_provider_or_database(self):
        code, events = self.run_phase('reconcile', 'reconcile', '78')
        self.assertEqual(code, 78)
        self.assertEqual(events, ['controller:classify-recovery'])

    def test_source_hashes_remain_mandatory_with_python_optimization(self):
        for field in ['CLEANUP_PROOF_SHA256', 'CLEANUP_CONTROLLER_SHA256', 'CLEANUP_DNS_OWNER_SHA256']:
            code, events = self.run_phase('execute', 'prepare', tamper=field)
            self.assertNotEqual(code, 0)
            self.assertFalse(any(event.startswith('controller:') for event in events))

    def test_unknown_phase_has_no_owner_or_controller_effect(self):
        code, events = self.run_phase('execute', 'arbitrary')
        self.assertEqual(code, 64)
        self.assertEqual(events, [])


if __name__ == '__main__':
    unittest.main()
