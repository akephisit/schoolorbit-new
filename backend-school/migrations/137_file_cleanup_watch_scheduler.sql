-- A per-target starting opportunity survives new sweeps/restarts without a new queue.
ALTER TABLE file_storage_cleanup_watches
    ADD COLUMN prefer_watch_next boolean NOT NULL DEFAULT false;
