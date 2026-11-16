# Manual supervision cycle status

Approved scope: remove cycle start/end and booking open/close timestamps from database,
API and UI. Staff explicitly choose draft, open, closed or archived. Only open cycles
accept new bookings and returned request resubmission; existing planned evaluation,
certification, approval and acknowledgement continue after booking closes. Academic
year/term write guards, actual observation timestamps, weekday matching and evaluator
conflict checks remain authoritative.

Teachers correct their own returned requests through the existing request update
operation. Saving atomically changes returned to requested; requested edits retain
requested status. Existing return comments and audit actions remain; no history UI
or new resubmission endpoint is introduced. To preserve completed evidence, returning,
resubmitting or reapproving a request with any submitted evaluator is refused under
the existing observation lock. Own requested/returned cancellation remains available.

Migration 159 drops only the four columns and their named checks with RESTRICT,
locking cycles and reconciling ordered hashes/counts of all retained cycle fields.
IDs, statuses, targets, observations, evaluators, responses and actions are preserved.
Term preparation copies fresh draft cycle configuration and mapped staff targets,
without cycle date mappings or observation copying; exam date requirements remain.
