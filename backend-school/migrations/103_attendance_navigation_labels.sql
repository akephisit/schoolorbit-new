-- Repair the generated default only; school-owned display names remain authoritative.
UPDATE menu_groups
SET name = 'งานจัดการเรียนการสอน'
WHERE code = 'academic_delivery' AND name = code;
