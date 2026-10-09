// Both ledgers use the annual student's current room, independent of the group's
// source room. Mixed-room groups must keep every active member on the roster.
pub(crate) const STUDENT_ROSTER_SQL: &str = r#"
    SELECT m.id AS membership_id, m.student_academic_year_id,
           concat_ws(' ', u.first_name, u.last_name) AS display_name, m.row_version,
           current_room.class_number, current_room.name AS homeroom_name
    FROM learning_group_students m
    JOIN users u ON u.id = m.student_id
    JOIN student_academic_years y ON y.id = m.student_academic_year_id
    JOIN grade_levels grade ON grade.id = y.grade_level_id
    LEFT JOIN student_info info ON info.user_id = m.student_id
    LEFT JOIN LATERAL (
        SELECT room.room_number, room.name, p.class_number
        FROM homeroom_placements p
        JOIN homerooms room ON room.id = p.homeroom_id
        WHERE p.student_academic_year_id = y.id
          AND p.status IN ('current', 'planned')
        ORDER BY (p.status = 'current') DESC, p.start_date DESC, p.id
        LIMIT 1
    ) current_room ON TRUE
    WHERE m.learning_group_id = $1 AND m.membership_status = 'active'
    ORDER BY CASE grade.level_type WHEN 'kindergarten' THEN 1 WHEN 'primary' THEN 2
                 WHEN 'secondary' THEN 3 ELSE 4 END, grade.year,
        CASE WHEN current_room.name IS NULL THEN 1 ELSE 0 END,
        (current_room.room_number IS NULL),
        CASE WHEN current_room.room_number ~ '^[0-9]+$'
             THEN length(ltrim(current_room.room_number, '0')) ELSE 2147483647 END,
        CASE WHEN current_room.room_number ~ '^[0-9]+$'
             THEN ltrim(current_room.room_number, '0') END,
        current_room.room_number NULLS LAST,
        current_room.class_number NULLS LAST, info.student_id NULLS LAST,
        u.first_name, u.last_name, y.id
    LIMIT 2001
"#;
