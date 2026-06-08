-- Link timetable ACTIVITY entries to activity_slots
ALTER TABLE academic_timetable_entries
  ADD COLUMN activity_slot_id UUID REFERENCES activity_slots(id) ON DELETE SET NULL;

CREATE INDEX idx_timetable_entry_activity_slot
  ON academic_timetable_entries(activity_slot_id)
  WHERE activity_slot_id IS NOT NULL;
