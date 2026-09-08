-- Repair only the generated placeholder; preserve school-owned names and placement.
UPDATE menu_groups
SET name = 'งานวัดผลและประเมินผล',
    name_en = CASE WHEN name_en = code THEN 'Measurement and Evaluation' ELSE name_en END,
    icon = CASE WHEN icon = 'folder' THEN 'badge-check' ELSE icon END,
    updated_at = now()
WHERE code = 'academic_assessment'
  AND name = code;
