use crate::error::AppError;
use crate::modules::academic::models::timetable::{TimetableEntry, TimetableQuery};
use sqlx::PgPool;
use uuid::Uuid;

/// Filter สำหรับ list_entries — รวม use case ทุกมุมมอง:
/// - Staff/Admin: ส่ง classroom_id หรือ semester_id (ไม่ filter user)
/// - Student: ส่ง student_id → filter ตาม student_class_enrollments
/// - Teacher: ส่ง instructor_id → filter ตาม timetable_entry_instructors
/// - Parent → ดูลูก: caller verify parent-child link แล้วส่ง student_id
#[derive(Debug, Default, Clone)]
pub struct TimetableFilter {
    pub classroom_id: Option<Uuid>,
    pub student_id: Option<Uuid>,
    pub instructor_id: Option<Uuid>,
    pub room_id: Option<Uuid>,
    pub academic_semester_id: Option<Uuid>,
    pub day_of_week: Option<String>,
    pub entry_type: Option<String>,
    /// ใช้กับ instructor_id: รวม cell ที่ instructor อยู่ใน team แต่ไม่ใช่ผู้สอนหลักของ cell
    pub include_team_ghosts: bool,
}

impl From<TimetableQuery> for TimetableFilter {
    fn from(q: TimetableQuery) -> Self {
        Self {
            classroom_id: q.classroom_id,
            student_id: q.student_id,
            instructor_id: q.instructor_id,
            room_id: q.room_id,
            academic_semester_id: q.academic_semester_id,
            day_of_week: q.day_of_week,
            entry_type: q.entry_type,
            include_team_ghosts: q.include_team_ghosts.unwrap_or(false),
        }
    }
}

/// SELECT clause พร้อม joins ที่ใช้ร่วมระหว่าง list_entries และ fetch_entry_by_id
/// แก้ตรงนี้ที่เดียวเมื่อต้องเพิ่มฟิลด์ joined
const ENTRY_SELECT_WITH_JOINS: &str = r#"
SELECT
    te.*,
    s.code   AS subject_code,
    s.name_th AS subject_name_th,
    (SELECT ARRAY_AGG(concat(u2.first_name, ' ', u2.last_name) ORDER BY tei2.role, tei2.created_at)
     FROM timetable_entry_instructors tei2
     JOIN users u2 ON u2.id = tei2.instructor_id
     WHERE tei2.entry_id = te.id) AS instructor_names,
    (SELECT ARRAY_AGG(tei_id.instructor_id ORDER BY tei_id.role, tei_id.created_at)
     FROM timetable_entry_instructors tei_id
     WHERE tei_id.entry_id = te.id) AS instructor_ids,
    (SELECT concat(u3.first_name, ' ', u3.last_name)
     FROM timetable_entry_instructors tei3
     JOIN users u3 ON u3.id = tei3.instructor_id
     WHERE tei3.entry_id = te.id
     ORDER BY tei3.role, tei3.created_at
     LIMIT 1) AS instructor_name,
    cr.name  AS classroom_name,
    r.code   AS room_code,
    ap.name  AS period_name,
    ap.start_time,
    ap.end_time,
    asl_ac.name AS activity_slot_name,
    asl_ac.activity_type AS activity_type,
    asl_ac.scheduling_mode AS activity_scheduling_mode
FROM academic_timetable_entries te
LEFT JOIN classroom_courses cc ON te.classroom_course_id = cc.id
LEFT JOIN subjects s ON cc.subject_id = s.id
LEFT JOIN class_rooms cr ON te.classroom_id = cr.id
JOIN academic_periods ap ON te.period_id = ap.id
LEFT JOIN rooms r ON te.room_id = r.id
LEFT JOIN activity_slots asl ON te.activity_slot_id = asl.id
LEFT JOIN activity_catalog asl_ac ON asl.activity_catalog_id = asl_ac.id
"#;

/// List timetable entries ตาม filter — single query path สำหรับทุกมุมมอง
pub async fn list_entries(
    pool: &PgPool,
    filter: TimetableFilter,
) -> Result<Vec<TimetableEntry>, AppError> {
    let mut sql = String::from(ENTRY_SELECT_WITH_JOINS);
    sql.push_str(" WHERE te.is_active = true");

    let mut idx = 0u32;

    if filter.classroom_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND te.classroom_id = ${idx}"));
    }

    if filter.student_id.is_some() {
        idx += 1;
        sql.push_str(&format!(
            " AND te.classroom_id IN (SELECT class_room_id FROM student_class_enrollments WHERE student_id = ${idx} AND status = 'active')"
        ));
    }

    if filter.instructor_id.is_some() {
        idx += 1;
        if filter.include_team_ghosts {
            // Ghost mode: รวม cell ที่ instructor อยู่ในทีมของ course (cci) หรือถูก assign ใน tei
            sql.push_str(&format!(
                " AND (EXISTS (SELECT 1 FROM classroom_course_instructors cci \
                       WHERE cci.classroom_course_id = te.classroom_course_id AND cci.instructor_id = ${idx}) \
                    OR EXISTS (SELECT 1 FROM timetable_entry_instructors tei \
                       WHERE tei.entry_id = te.id AND tei.instructor_id = ${idx}))"
            ));
        } else {
            sql.push_str(&format!(
                " AND EXISTS (SELECT 1 FROM timetable_entry_instructors tei WHERE tei.entry_id = te.id AND tei.instructor_id = ${idx})"
            ));
        }
    }

    if filter.room_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND te.room_id = ${idx}"));
    }

    if filter.academic_semester_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND te.academic_semester_id = ${idx}"));
    }

    if filter.day_of_week.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND te.day_of_week = ${idx}"));
    }

    if filter.entry_type.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND te.entry_type = ${idx}"));
    }

    sql.push_str(" ORDER BY te.day_of_week, ap.order_index");

    let mut q = sqlx::query_as::<_, TimetableEntry>(&sql);
    if let Some(v) = filter.classroom_id { q = q.bind(v); }
    if let Some(v) = filter.student_id { q = q.bind(v); }
    if let Some(v) = filter.instructor_id { q = q.bind(v); }
    if let Some(v) = filter.room_id { q = q.bind(v); }
    if let Some(v) = filter.academic_semester_id { q = q.bind(v); }
    if let Some(v) = filter.day_of_week { q = q.bind(v); }
    if let Some(v) = filter.entry_type { q = q.bind(v); }

    q.fetch_all(pool).await.map_err(|e| {
        eprintln!("Failed to fetch timetable entries: {}", e);
        AppError::InternalServerError("Failed to fetch timetable".to_string())
    })
}

/// Fetch 1 entry พร้อม joined fields (subject, classroom, room, period, instructors)
/// ใช้กับ patch events เพื่อให้ frontend ได้ entry ครบ, patch ได้ทันทีไม่ต้อง re-fetch
pub async fn fetch_entry_by_id(pool: &PgPool, entry_id: Uuid) -> Option<TimetableEntry> {
    let mut sql = String::from(ENTRY_SELECT_WITH_JOINS);
    sql.push_str(" WHERE te.id = $1");

    sqlx::query_as::<_, TimetableEntry>(&sql)
        .bind(entry_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}
