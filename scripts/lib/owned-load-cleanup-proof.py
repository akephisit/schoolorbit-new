"""Pure proof gates for the fixed owned-load cleanup; no transport or persistence."""

import datetime
import hashlib
import re
import uuid


_MAX_INTEGER = (1 << 63) - 1
_MAX_TABLES = 1024
_MAX_IDENTITIES = 100_000
_ORIGINAL_WATCH_ROWS = 109
_EMPTY_SHA256 = hashlib.sha256(b"").hexdigest()
_HEX256 = re.compile(r"[0-9a-f]{64}\Z")
_HEX128 = re.compile(r"[0-9a-f]{32}\Z")
_TABLE = re.compile(r"[a-z_][a-z0-9_]{0,62}\Z")
_TIMESTAMP = re.compile(
    r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,6})?(?:Z|[+-]\d{2}:\d{2})\Z"
)
_SNAPSHOT_KEYS = {
    "outside", "owned", "auth", "audit", "actorStatus", "realActivity",
    "context", "schemaFingerprint", "originalWatchBusiness", "watchCounters",
    "behaviorRevision", "notices", "jobs",
}
_PRESERVED_KEYS = (
    "outside", "auth", "audit", "realActivity", "context", "schemaFingerprint",
    "originalWatchBusiness", "notices", "jobs",
)
_CODES = frozenset({
    "invalid_shape", "invalid_integer", "invalid_digest", "invalid_identity",
    "duplicate_identity", "invalid_table", "duplicate_table", "invalid_timestamp",
    "invalid_actor", "invalid_catalogue", "original_watch_changed",
    "original_watch_source_changed", "watch_identity_changed", "watch_counter_regressed",
    "preservation_changed", "behavior_counter_regressed", "actor_transition_refused",
    "owned_rows_remain", "owned_tables_changed", "invalid_census",
})


class CleanupRefusal(ValueError):
    """A bounded finding code, never the rejected data or an identity."""

    def __init__(self, code):
        self.code = code if type(code) is str and code in _CODES else "invalid_shape"
        super().__init__(self.code)


def _require(condition, code="invalid_shape"):
    if not condition:
        raise CleanupRefusal(code)


def _object(value, keys):
    _require(type(value) is dict and value.keys() == keys)


def _integer(value):
    _require(type(value) is int and 0 <= value <= _MAX_INTEGER, "invalid_integer")


def _digest(value, pattern=_HEX256):
    _require(type(value) is str and pattern.fullmatch(value) is not None, "invalid_digest")


def _identity(value):
    _require(type(value) is str and len(value) == 36, "invalid_identity")
    try:
        canonical = str(uuid.UUID(value))
    except ValueError:
        raise CleanupRefusal("invalid_identity") from None
    _require(value == canonical, "invalid_identity")


def _list(value, limit, minimum=0):
    _require(type(value) is list and minimum <= len(value) <= limit)


def _identities(value):
    _list(value, _MAX_IDENTITIES)
    seen = set()
    for item in value:
        _identity(item)
        _require(item not in seen, "duplicate_identity")
        seen.add(item)


def _summary(value):
    _object(value, {"count", "sha256"})
    _integer(value["count"])
    _digest(value["sha256"])


def _tables(value):
    _list(value, _MAX_TABLES, minimum=1)
    seen = set()
    for item in value:
        _object(item, {"table", "count", "sha256"})
        name = item["table"]
        _require(type(name) is str and _TABLE.fullmatch(name) is not None, "invalid_table")
        _require(name not in seen, "duplicate_table")
        seen.add(name)
        _integer(item["count"])
        _digest(item["sha256"])
        _require(item["count"] != 0 or item["sha256"] == _EMPTY_SHA256, "invalid_digest")
    return seen


def _watches(value):
    _list(value, _ORIGINAL_WATCH_ROWS, minimum=_ORIGINAL_WATCH_ROWS)
    seen = {}
    for item in value:
        _object(item, {"id", "row_revision", "lease_generation"})
        _identity(item["id"])
        _require(item["id"] not in seen, "duplicate_identity")
        _integer(item["row_revision"])
        _integer(item["lease_generation"])
        seen[item["id"]] = item
    return seen


def _captured(value, *, jobs):
    _list(value, _MAX_IDENTITIES)
    keys = {"id", "notification_id", "recipient_id"}
    keys |= {"source_kind", "source_id"} if jobs else {"created_at"}
    seen = set()
    for item in value:
        _object(item, keys)
        for key in ("id", "notification_id", "recipient_id"):
            _identity(item[key])
        _require(item["id"] not in seen, "duplicate_identity")
        seen.add(item["id"])
        if jobs:
            _identity(item["source_id"])
            kind = item["source_kind"]
            _require(type(kind) is str and re.fullmatch(r"[a-z][a-z0-9_]{0,127}", kind) is not None)
        else:
            stamp = item["created_at"]
            _require(type(stamp) is str and len(stamp) <= 32 and _TIMESTAMP.fullmatch(stamp) is not None,
                     "invalid_timestamp")
            try:
                datetime.datetime.fromisoformat(stamp.replace("Z", "+00:00"))
            except ValueError:
                raise CleanupRefusal("invalid_timestamp") from None


def _snapshot(snapshot):
    _object(snapshot, _SNAPSHOT_KEYS)
    outside = _tables(snapshot["outside"])
    owned = _tables(snapshot["owned"])
    _require(owned <= outside, "owned_tables_changed")
    for name, expected in (("auth", 418), ("audit", 10), ("realActivity", 1)):
        _summary(snapshot[name])
        _require(snapshot[name]["count"] == expected, "invalid_census")
    _require(type(snapshot["actorStatus"]) is str and snapshot["actorStatus"] in ("active", "inactive"),
             "invalid_actor")
    _digest(snapshot["schemaFingerprint"])
    context = snapshot["context"]
    _object(context, {"years", "terms", "settings"})
    _identities(context["years"])
    _identities(context["terms"])
    _digest(context["settings"])
    business = snapshot["originalWatchBusiness"]
    _object(business, {"rows", "md5"})
    _integer(business["rows"])
    _require(business["rows"] == _ORIGINAL_WATCH_ROWS, "original_watch_changed")
    _digest(business["md5"], _HEX128)
    _watches(snapshot["watchCounters"])
    _integer(snapshot["behaviorRevision"])
    _captured(snapshot["notices"], jobs=False)
    _captured(snapshot["jobs"], jobs=True)


def _monotonic(before, after):
    old = _watches(before)
    new = _watches(after)
    _require(old.keys() == new.keys(), "watch_identity_changed")
    for identity, previous in old.items():
        current = new[identity]
        _require(all(current[key] >= previous[key] for key in ("row_revision", "lease_generation")),
                 "watch_counter_regressed")


def validate_snapshot(snapshot, original_watch_floor, catalogue):
    """Validate one fresh preflight against the trusted catalogue and original floor."""
    _snapshot(snapshot)
    _require(type(catalogue) is dict and type(catalogue.get("schemaVersion")) is int
             and catalogue["schemaVersion"] == 1 and "originalWatch" in catalogue
             and "inputSha256" in catalogue, "invalid_catalogue")
    _digest(catalogue["inputSha256"])
    expected = catalogue["originalWatch"]
    _object(expected, {"rows", "md5", "sourceSHA256"})
    _integer(expected["rows"])
    _require(expected["rows"] == _ORIGINAL_WATCH_ROWS, "invalid_catalogue")
    _digest(expected["md5"], _HEX128)
    _digest(expected["sourceSHA256"])
    _object(original_watch_floor, {"schemaVersion", "sourceSHA256", "watchCounters"})
    _require(type(original_watch_floor["schemaVersion"]) is int
             and original_watch_floor["schemaVersion"] == 1)
    _digest(original_watch_floor["sourceSHA256"])
    _require(original_watch_floor["sourceSHA256"] == expected["sourceSHA256"],
             "original_watch_source_changed")
    _require(snapshot["originalWatchBusiness"] == {"rows": expected["rows"], "md5": expected["md5"]},
             "original_watch_changed")
    _monotonic(original_watch_floor["watchCounters"], snapshot["watchCounters"])
    _require(snapshot["actorStatus"] == "active", "actor_transition_refused")


def _preservation(before, after):
    _snapshot(before)
    _snapshot(after)
    _require(all(before[key] == after[key] for key in _PRESERVED_KEYS), "preservation_changed")
    _require({row["table"] for row in before["owned"]} == {row["table"] for row in after["owned"]},
             "owned_tables_changed")
    _monotonic(before["watchCounters"], after["watchCounters"])
    _require(after["behaviorRevision"] >= before["behaviorRevision"], "behavior_counter_regressed")


def validate_preservation(before, after):
    """Require preserved business data and monotonic worker counters between captures."""
    _preservation(before, after)
    _require(before["owned"] == after["owned"], "preservation_changed")
    _require(before["actorStatus"] == after["actorStatus"], "actor_transition_refused")


def validate_postcleanup(approved, observed):
    """Require an active approved baseline, complete cleanup, and preserved real data."""
    _preservation(approved, observed)
    _require(approved["actorStatus"] == "active" and observed["actorStatus"] == "inactive",
             "actor_transition_refused")
    _require(all(row["count"] == 0 for row in observed["owned"]), "owned_rows_remain")
