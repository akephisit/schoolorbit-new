"""Sanitized behavioral tests for the pure owned-load cleanup proof boundary."""

import copy
import hashlib
import importlib.util
from pathlib import Path
import unittest
import uuid


_SPEC = importlib.util.spec_from_file_location(
    "owned_load_cleanup_proof",
    Path(__file__).resolve().parents[1] / "lib" / "owned-load-cleanup-proof.py",
)
proof = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(proof)


def identity(number):
    return str(uuid.UUID(int=number))


def digest(value):
    return hashlib.sha256(value.encode()).hexdigest()


def summary(count, label):
    return {"count": count, "sha256": digest(label)}


def fixtures():
    counters = [
        {"id": identity(number), "row_revision": number, "lease_generation": number + 1}
        for number in range(1, 110)
    ]
    floor = {"schemaVersion": 1, "sourceSHA256": digest("original source"), "watchCounters": counters}
    catalogue = {
        "schemaVersion": 1, "inputSha256": digest("trusted bundle"),
        "originalWatch": {"rows": 109, "md5": "1" * 32, "sourceSHA256": floor["sourceSHA256"]},
    }
    snapshot = {
        "outside": [
            {"table": "users", **summary(27, "real users including actor password")},
            {"table": "attendance_sessions", **summary(8, "real attendance")},
            {"table": "notifications", **summary(3, "real notices")},
        ],
        "owned": [
            {"table": "attendance_sessions", **summary(105, "synthetic attendance")},
            {"table": "notifications", **summary(300, "synthetic notices")},
        ],
        "auth": summary(418, "revoked sessions"),
        "audit": summary(10, "preserved audit"),
        "actorStatus": "active",
        "realActivity": summary(1, "published real activity"),
        "context": {"years": [identity(200)], "terms": [identity(201)], "settings": digest("settings")},
        "schemaFingerprint": digest("constraints"),
        "originalWatchBusiness": {"rows": 109, "md5": "1" * 32},
        "watchCounters": copy.deepcopy(counters), "behaviorRevision": 50,
        "notices": [{"id": identity(300), "notification_id": identity(301),
                     "recipient_id": identity(302), "created_at": "2026-10-25T12:00:00.123456+00:00"}],
        "jobs": [{"id": identity(400), "notification_id": identity(301),
                  "recipient_id": identity(302), "source_kind": "attendance",
                  "source_id": identity(401)}],
    }
    return snapshot, floor, catalogue


def cleaned(snapshot):
    result = copy.deepcopy(snapshot)
    result["actorStatus"] = "inactive"
    for row in result["owned"]:
        row.update(count=0, sha256=digest(""))
    return result


def replace(value, path, replacement):
    for key in path[:-1]:
        value = value[key]
    value[path[-1]] = replacement


class ProofTests(unittest.TestCase):
    def setUp(self):
        self.snapshot, self.floor, self.catalogue = fixtures()

    def refusal(self, operation, *args, code=None):
        with self.assertRaises(proof.CleanupRefusal) as caught:
            operation(*args)
        finding = caught.exception
        self.assertIs(type(finding.code), str)
        self.assertRegex(finding.code, r"\A[a-z_]{1,40}\Z")
        self.assertEqual(str(finding), finding.code)
        if code:
            self.assertEqual(finding.code, code)

    def validate(self, snapshot):
        proof.validate_snapshot(snapshot, self.floor, self.catalogue)

    def test_equal_and_monotonic_fresh_captures_pass_without_mutation(self):
        original = copy.deepcopy((self.snapshot, self.floor, self.catalogue))
        self.assertIsNone(self.validate(self.snapshot))
        self.assertIsNone(proof.validate_preservation(self.snapshot, copy.deepcopy(self.snapshot)))
        advanced = copy.deepcopy(self.snapshot)
        advanced["watchCounters"][0]["row_revision"] += 1
        advanced["watchCounters"][1]["lease_generation"] += 3
        advanced["behaviorRevision"] += 7
        self.assertIsNone(self.validate(advanced))
        self.assertIsNone(proof.validate_preservation(self.snapshot, advanced))
        self.assertEqual((self.snapshot, self.floor, self.catalogue), original)

    def test_cleanup_and_worker_advancement_pass(self):
        result = cleaned(self.snapshot)
        result["watchCounters"][0]["row_revision"] += 20
        result["watchCounters"][1]["lease_generation"] += 10
        result["behaviorRevision"] += 1
        self.assertIsNone(proof.validate_postcleanup(self.snapshot, result))
        self.assertIsNone(proof.validate_postcleanup(self.snapshot, copy.deepcopy(result)))

    def test_original_watch_business_md5_and_rows_are_bound_to_catalogue(self):
        for path, value in ((["originalWatchBusiness", "md5"], "2" * 32),
                            (["originalWatchBusiness", "rows"], 108)):
            with self.subTest(path=path):
                candidate = copy.deepcopy(self.snapshot)
                replace(candidate, path, value)
                self.refusal(self.validate, candidate, code="original_watch_changed")

    def test_watch_identity_loss_replacement_duplicate_and_regression_refuse(self):
        candidates = []
        missing = copy.deepcopy(self.snapshot)
        missing["watchCounters"].pop()
        candidates.append(missing)
        replaced = copy.deepcopy(self.snapshot)
        replaced["watchCounters"][0]["id"] = identity(999)
        candidates.append(replaced)
        duplicate = copy.deepcopy(self.snapshot)
        duplicate["watchCounters"][1]["id"] = duplicate["watchCounters"][0]["id"]
        candidates.append(duplicate)
        for counter in ("row_revision", "lease_generation"):
            regressed = copy.deepcopy(self.snapshot)
            regressed["watchCounters"][0][counter] -= 1
            candidates.append(regressed)
        for number, candidate in enumerate(candidates):
            with self.subTest(case=number):
                self.refusal(self.validate, candidate)
                self.refusal(proof.validate_preservation, self.snapshot, candidate)

    def test_source_floor_and_catalogue_shape_refuse_drift(self):
        floor = copy.deepcopy(self.floor)
        floor["sourceSHA256"] = digest("replacement source")
        self.refusal(proof.validate_snapshot, self.snapshot, floor, self.catalogue,
                     code="original_watch_source_changed")
        for path, value in ((["schemaVersion"], True), (["originalWatch", "rows"], 108),
                            (["originalWatch", "sourceSHA256"], None), (["inputSha256"], "x")):
            with self.subTest(path=path):
                catalogue = copy.deepcopy(self.catalogue)
                replace(catalogue, path, value)
                self.refusal(proof.validate_snapshot, self.snapshot, self.floor, catalogue)
        floor = copy.deepcopy(self.floor)
        floor["watchCounters"][0]["row_revision"] += 1
        self.refusal(proof.validate_snapshot, self.snapshot, floor, self.catalogue,
                     code="watch_counter_regressed")

    def test_malformed_snapshot_shapes_refuse(self):
        cases = [
            (["behaviorRevision"], True), (["behaviorRevision"], -1),
            (["behaviorRevision"], 1 << 63), (["behaviorRevision"], float("nan")),
            (["auth", "count"], True), (["auth", "count"], 417),
            (["audit", "count"], 9), (["realActivity", "count"], 0),
            (["schemaFingerprint"], "a" * 65), (["schemaFingerprint"], "A" * 64),
            (["originalWatchBusiness", "md5"], None),
            (["watchCounters", 0, "id"], "ABCDEF00-0000-0000-0000-000000000001"),
            (["watchCounters", 0, "id"], "1" * 36),
            (["watchCounters", 0, "row_revision"], True),
            (["watchCounters", 0, "lease_generation"], -1),
            (["outside", 0, "table"], "a" * 64), (["outside", 0, "count"], float("inf")),
            (["outside", 0, "sha256"], "not a digest"), (["owned"], []),
            (["context", "years"], [identity(200), identity(200)]),
            (["context", "terms"], [None]), (["context", "settings"], []),
            (["actorStatus"], "deleted"), (["actorStatus"], None),
            (["notices", 0, "created_at"], "2026-02-30T12:00:00Z"),
            (["notices", 0, "created_at"], "2026-10-25"),
            (["notices", 0, "created_at"], "x" * 1000),
            (["jobs", 0, "source_kind"], "x" * 129),
            (["jobs", 0, "source_id"], None),
        ]
        for number, (path, value) in enumerate(cases):
            with self.subTest(case=number, path=path):
                candidate = copy.deepcopy(self.snapshot)
                replace(candidate, path, value)
                self.refusal(self.validate, candidate)
        for key in self.snapshot:
            candidate = copy.deepcopy(self.snapshot)
            del candidate[key]
            with self.subTest(missing=key):
                self.refusal(self.validate, candidate)
        candidate = copy.deepcopy(self.snapshot)
        candidate["untrustedExtra"] = "refuse extra fields"
        self.refusal(self.validate, candidate)

    def test_oversized_duplicate_and_missing_nested_data_refuse(self):
        for key in ("outside", "owned", "notices", "jobs"):
            candidate = copy.deepcopy(self.snapshot)
            candidate[key].append(copy.deepcopy(candidate[key][0]))
            with self.subTest(duplicate=key):
                self.refusal(self.validate, candidate)
        candidate = copy.deepcopy(self.snapshot)
        candidate["outside"] *= 1025
        self.refusal(self.validate, candidate)
        candidate = copy.deepcopy(self.snapshot)
        candidate["jobs"] *= 100001
        self.refusal(self.validate, candidate)
        for path in (("context", "settings"), ("notices", 0, "recipient_id"),
                     ("watchCounters", 0, "lease_generation"), ("auth", "sha256")):
            candidate = copy.deepcopy(self.snapshot)
            container = candidate
            for key in path[:-1]:
                container = container[key]
            del container[path[-1]]
            with self.subTest(missing=path):
                self.refusal(self.validate, candidate)

    def test_preserved_business_and_captured_identity_changes_refuse(self):
        cases = [
            (["outside", 0, "sha256"], digest("changed actor password")),
            (["outside", 1, "count"], 9), (["auth", "sha256"], digest("changed auth")),
            (["audit", "sha256"], digest("changed audit")),
            (["realActivity", "sha256"], digest("changed published activity")),
            (["context", "years"], [identity(202)]),
            (["context", "terms"], [identity(203)]),
            (["context", "settings"], digest("changed school settings")),
            (["schemaFingerprint"], digest("changed constraint")),
            (["originalWatchBusiness", "md5"], "3" * 32),
            (["notices", 0, "id"], identity(500)),
            (["notices", 0, "recipient_id"], identity(501)),
            (["jobs", 0, "source_id"], identity(502)),
            (["jobs", 0, "notification_id"], identity(503)),
        ]
        for number, (path, value) in enumerate(cases):
            candidate = copy.deepcopy(self.snapshot)
            replace(candidate, path, value)
            with self.subTest(case=number):
                self.refusal(proof.validate_preservation, self.snapshot, candidate,
                             code="preservation_changed")
                self.refusal(proof.validate_postcleanup, self.snapshot, cleaned(candidate))

    def test_counter_regression_and_actor_transition_refuse(self):
        candidate = copy.deepcopy(self.snapshot)
        candidate["behaviorRevision"] -= 1
        self.refusal(proof.validate_preservation, self.snapshot, candidate,
                     code="behavior_counter_regressed")
        self.refusal(proof.validate_postcleanup, self.snapshot, cleaned(candidate))
        candidate = copy.deepcopy(self.snapshot)
        candidate["actorStatus"] = "inactive"
        self.refusal(proof.validate_preservation, self.snapshot, candidate,
                     code="actor_transition_refused")
        self.refusal(self.validate, cleaned(self.snapshot), code="actor_transition_refused")

    def test_postcleanup_requires_all_owned_tables_empty_and_active_approval(self):
        result = cleaned(self.snapshot)
        self.refusal(proof.validate_postcleanup, result, result, code="actor_transition_refused")
        self.refusal(proof.validate_postcleanup, self.snapshot, self.snapshot,
                     code="actor_transition_refused")
        resurrected = copy.deepcopy(result)
        resurrected["owned"][1].update(count=1, sha256=digest("resurrected row"))
        self.refusal(proof.validate_postcleanup, self.snapshot, resurrected, code="owned_rows_remain")
        missing = copy.deepcopy(result)
        missing["owned"].pop()
        self.refusal(proof.validate_postcleanup, self.snapshot, missing, code="owned_tables_changed")
        extra = copy.deepcopy(result)
        extra["owned"].append({"table": "users", **summary(0, "")})
        self.refusal(proof.validate_postcleanup, self.snapshot, extra, code="owned_tables_changed")

    def test_preflight_owned_drift_and_empty_digest_mismatch_refuse(self):
        for field, value in (("count", 106), ("sha256", digest("changed owned row"))):
            candidate = copy.deepcopy(self.snapshot)
            candidate["owned"][0][field] = value
            self.refusal(proof.validate_preservation, self.snapshot, candidate,
                         code="preservation_changed")
        result = cleaned(self.snapshot)
        result["owned"][0]["sha256"] = digest("forged empty digest")
        self.refusal(proof.validate_postcleanup, self.snapshot, result, code="invalid_digest")

    def test_exception_never_echoes_rejected_data(self):
        candidate = copy.deepcopy(self.snapshot)
        candidate["watchCounters"][0]["id"] = "untrusted-private-value"
        self.refusal(self.validate, candidate, code="invalid_identity")
        finding = proof.CleanupRefusal("untrusted-private-value")
        self.assertEqual(str(finding), "invalid_shape")


if __name__ == "__main__":
    unittest.main()
