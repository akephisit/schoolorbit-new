-- Migration: Remove unused subject constraint fields
-- These fields are replaced by more precise allowed_period_ids and allowed_days

-- Remove preferred_time_of_day (replaced by allowed_period_ids)
ALTER TABLE subjects DROP COLUMN IF EXISTS preferred_time_of_day;

-- Remove required_room_type (room assignment is instructor-based, not subject-based)
ALTER TABLE subjects DROP COLUMN IF EXISTS required_room_type;
