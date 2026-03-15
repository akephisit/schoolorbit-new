ALTER TABLE class_rooms
    ADD COLUMN IF NOT EXISTS capacity INT NOT NULL DEFAULT 40;
COMMENT ON COLUMN class_rooms.capacity IS 'จำนวนนักเรียนที่รองรับได้ในห้อง (default 40)';
