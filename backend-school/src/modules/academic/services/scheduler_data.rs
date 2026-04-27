use sqlx::PgPool;
use uuid::Uuid;
use crate::modules::academic::services::scheduler::{
    types::*,
    validator::LockedSlotData,
};
use std::collections::{HashMap, HashSet};

/// Phase D: cc preferred rooms — load all in 1 query → group by cc_id
async fn load_cc_preferred_rooms_batch(
    pool: &PgPool,
    cc_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<RoomPref>>, sqlx::Error> {
    if cc_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query_as::<_, (Uuid, Uuid, i32, bool)>(
        r#"SELECT classroom_course_id, room_id, rank, is_required
           FROM classroom_course_preferred_rooms
           WHERE classroom_course_id = ANY($1)
           ORDER BY classroom_course_id, rank ASC"#
    )
    .bind(cc_ids)
    .fetch_all(pool)
    .await?;

    let mut map: HashMap<Uuid, Vec<RoomPref>> = HashMap::new();
    for (cc_id, room_id, _rank, is_required) in rows {
        map.entry(cc_id).or_default().push(RoomPref { room_id, is_required });
    }
    Ok(map)
}

/// Load data from database for scheduler
pub struct SchedulerDataLoader<'a> {
    pool: &'a PgPool,
}

impl<'a> SchedulerDataLoader<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
    
    /// Load courses to schedule for given classrooms and semester
    /// Sorted by primary instructor's priority — ครูสำคัญถูกจัดก่อน
    pub async fn load_courses(
        &self,
        classroom_ids: &[Uuid],
        semester_id: Uuid,
    ) -> Result<Vec<CourseToSchedule>, sqlx::Error> {
        let query = r#"
            SELECT
                cc.id as classroom_course_id,
                cc.classroom_id,
                COALESCE(cls.name, '') as classroom_name,
                cc.subject_id,
                COALESCE(s.code, '') as subject_code,
                COALESCE(s.name_th, '') as subject_name,
                cc.primary_instructor_id as instructor_id,
                u.first_name || ' ' || u.last_name as instructor_name,

                -- Period requirements
                COALESCE(s.periods_per_week,
                    CASE WHEN s.hours_per_semester > 0 THEN CEIL(s.hours_per_semester::float / 20.0)::int
                         WHEN s.credit > 0 THEN CEIL(s.credit * 2.0)::int
                         ELSE 2
                    END) as periods_needed,

                -- Consecutive requirements (subject-level, kept for back-compat)
                COALESCE(s.min_consecutive_periods, 1) as min_consecutive,
                2 as max_consecutive,
                COALESCE(s.allow_single_period, true) as allow_single_period,
                s.allowed_period_ids,
                s.allowed_days,

                -- Phase B: classroom_course-level constraints
                cc.consecutive_pattern AS consecutive_pattern,
                cc.same_day_unique AS same_day_unique,
                cc.hard_unavailable_slots AS cc_hard_unavailable_slots

            FROM classroom_courses cc
            JOIN subjects s ON s.id = cc.subject_id
            JOIN class_rooms cls ON cls.id = cc.classroom_id
            JOIN academic_semesters sem ON sem.id = cc.academic_semester_id
            LEFT JOIN users u ON u.id = cc.primary_instructor_id
            LEFT JOIN instructor_preferences ip
                ON ip.instructor_id = cc.primary_instructor_id
                AND ip.academic_year_id = sem.academic_year_id
            WHERE cc.classroom_id = ANY($1)
              AND cc.academic_semester_id = $2
            ORDER BY COALESCE(ip.priority, 100) ASC, s.code ASC
        "#;
        
        let rows = sqlx::query_as::<_, CourseRow>(query)
            .bind(classroom_ids)
            .bind(semester_id)
            .fetch_all(self.pool)
            .await?;

        // Load instructor room assignments
        let instructor_rooms = self.load_instructor_room_assignments(semester_id).await?;

        // Phase D: load cc preferred rooms (batch — 1 query for all cc)
        let cc_ids: Vec<Uuid> = rows.iter().map(|r| r.classroom_course_id).collect();
        let cc_room_map = load_cc_preferred_rooms_batch(self.pool, &cc_ids).await?;

        // Convert to CourseToSchedule
        let mut courses = Vec::new();
        for row in rows {
            let fixed_room_id = if let Some(instructor_id) = row.instructor_id {
                instructor_rooms.get(&instructor_id).copied()
            } else {
                None
            };
            
            // Parse JSONB arrays
            let allowed_period_ids = row.allowed_period_ids.as_ref().and_then(|json| {
                json.as_array().and_then(|arr| {
                    let uuids: Result<Vec<Uuid>, _> = arr
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| Uuid::parse_str(s))
                        .collect();
                    uuids.ok()
                })
            });
            
            let allowed_days = row.allowed_days.as_ref().and_then(|json| {
                json.as_array().and_then(|arr| {
                    Some(arr
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect())
                })
            });
            
            // Phase B: parse cc.consecutive_pattern (Option<Vec<i32>>)
            let consecutive_pattern: Option<Vec<i32>> = row.consecutive_pattern.as_ref().and_then(|json| {
                json.as_array().map(|arr| {
                    arr.iter().filter_map(|v| v.as_i64().map(|n| n as i32)).collect()
                })
            });

            // Phase B: parse cc.hard_unavailable_slots → HashSet<key>
            let cc_hard_unavailable: HashSet<String> = {
                let mut set = HashSet::new();
                if let Some(arr) = row.cc_hard_unavailable_slots.as_array() {
                    for slot in arr {
                        let day = slot.get("day").and_then(|v| v.as_str()).map(String::from);
                        let period_id = slot.get("period_id").and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok());
                        if let (Some(d), Some(p)) = (day, period_id) {
                            set.insert(format!("{}__{}", d, p));
                        }
                    }
                }
                set
            };

            // Phase D: pull cc preferred rooms from batch map
            let preferred_rooms = cc_room_map
                .get(&row.classroom_course_id)
                .cloned()
                .unwrap_or_default();

            courses.push(CourseToSchedule {
                id: Uuid::new_v4(), // Unique ID for this scheduling instance
                classroom_course_id: row.classroom_course_id,
                classroom_id: row.classroom_id,
                classroom_name: row.classroom_name,
                subject_id: row.subject_id,
                subject_code: row.subject_code,
                subject_name: row.subject_name,
                instructor_id: row.instructor_id,
                instructor_name: row.instructor_name,
                periods_needed: row.periods_needed,
                periods_remaining: row.periods_needed,
                min_consecutive: row.min_consecutive,
                max_consecutive: row.max_consecutive,
                allow_single_period: row.allow_single_period,
                fixed_room_id,
                allowed_period_ids,
                allowed_days,
                cc_hard_unavailable,
                same_day_unique: row.same_day_unique,
                consecutive_pattern,
                preferred_rooms,
                activity_slot_id: None,
            });
        }

        Ok(courses)
    }

    /// Phase C: Load independent activity slots → CourseToSchedule list
    /// แต่ละ (slot, classroom) → 1 entity (ครู = จาก activity_slot_classroom_assignments)
    /// scheduler มอง activity เหมือน course — schedule ลง slot ว่างได้
    pub async fn load_independent_activities(
        &self,
        classroom_ids: &[Uuid],
        semester_id: Uuid,
    ) -> Result<Vec<CourseToSchedule>, sqlx::Error> {
        // ดึง slot + assignments + catalog
        // 1 query — JOIN ทุกอย่าง
        let rows = sqlx::query_as::<_, IndepActivityRow>(
            r#"
            SELECT
                s.id AS slot_id,
                cls.id AS classroom_id,
                COALESCE(cls.name, '') AS classroom_name,
                COALESCE(ac.name, '') AS activity_name,
                COALESCE(ac.activity_type, '') AS activity_type,
                ac.periods_per_week AS periods_per_week,
                asca.instructor_id,
                u.first_name || ' ' || u.last_name AS instructor_name
            FROM activity_slots s
            JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
            JOIN activity_slot_classroom_assignments asca ON asca.slot_id = s.id
            JOIN class_rooms cls ON cls.id = asca.classroom_id
            LEFT JOIN users u ON u.id = asca.instructor_id
            WHERE s.semester_id = $1
              AND s.is_active = true
              AND ac.scheduling_mode = 'independent'
              AND asca.classroom_id = ANY($2)
            "#
        )
        .bind(semester_id)
        .bind(classroom_ids)
        .fetch_all(self.pool)
        .await?;

        let mut result = Vec::new();
        for row in rows {
            // ใช้ Uuid::nil() สำหรับ classroom_course_id (ไม่ใช้ในกรณี activity)
            // subject_id ก็ใช้ slot_id แทน (เพื่อให้ same_day_unique check ทำงาน — slot เดียวกัน
            // ในวันเดียวกันจะถือว่าเป็นวิชาเดียวกัน)
            result.push(CourseToSchedule {
                id: Uuid::new_v4(),
                classroom_course_id: Uuid::nil(),
                classroom_id: row.classroom_id,
                classroom_name: row.classroom_name,
                subject_id: row.slot_id,  // slot_id เป็น "subject id" สำหรับ same_day_unique
                subject_code: format!("[{}]", row.activity_type),
                subject_name: row.activity_name,
                instructor_id: row.instructor_id,
                instructor_name: row.instructor_name,
                periods_needed: row.periods_per_week,
                periods_remaining: row.periods_per_week,
                min_consecutive: 1,
                max_consecutive: 2,
                allow_single_period: true,
                fixed_room_id: None,
                allowed_period_ids: None,
                allowed_days: None,
                cc_hard_unavailable: HashSet::new(),
                same_day_unique: true,
                consecutive_pattern: None,
                preferred_rooms: Vec::new(),
                activity_slot_id: Some(row.slot_id),
            });
        }

        Ok(result)
    }
    
    /// Load available time slots from periods
    /// ดึง school_days จาก academic_year ของ semester ที่กำลัง schedule
    pub async fn load_available_slots(&self, semester_id: Uuid) -> Result<Vec<TimeSlot>, sqlx::Error> {
        // ดึง school_days จาก academic_year ที่ผูกกับ semester
        let school_days: String = sqlx::query_scalar(
            r#"SELECT ay.school_days
               FROM academic_semesters s
               JOIN academic_years ay ON ay.id = s.academic_year_id
               WHERE s.id = $1"#
        )
        .bind(semester_id)
        .fetch_optional(self.pool)
        .await?
        .unwrap_or_else(|| "MON,TUE,WED,THU,FRI".to_string());

        let days: Vec<&str> = school_days.split(',').map(|d| d.trim()).collect();

        // ดึง academic_year_id จาก semester เพื่อ filter periods
        let year_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT academic_year_id FROM academic_semesters WHERE id = $1"
        )
        .bind(semester_id)
        .fetch_optional(self.pool)
        .await?;

        let periods = sqlx::query_as::<_, PeriodRow>(
            r#"SELECT id, order_index as period_order, name, start_time::text, end_time::text
               FROM academic_periods
               WHERE is_active = true AND academic_year_id = $1
               ORDER BY order_index"#
        )
        .bind(year_id)
        .fetch_all(self.pool)
        .await?;

        let mut slots = Vec::new();

        for day in &days {
            for period in &periods {
                slots.push(TimeSlot {
                    day: day.to_string(),
                    period_id: period.id,
                    period_order: period.period_order,
                });
            }
        }

        Ok(slots)
    }
    
    /// Load periods info filtered by academic year
    pub async fn load_periods(&self, academic_year_id: Uuid) -> Result<Vec<PeriodInfo>, sqlx::Error> {
        let rows = sqlx::query_as::<_, PeriodRow>(
            r#"SELECT id, order_index as period_order, name, start_time::text, end_time::text
               FROM academic_periods
               WHERE is_active = true AND academic_year_id = $1
               ORDER BY order_index"#
        )
        .bind(academic_year_id)
        .fetch_all(self.pool)
        .await?;
        
        Ok(rows.into_iter().map(|r| PeriodInfo {
            id: r.id,
            order: r.period_order,
            name: r.name.unwrap_or_default(),
            start_time: r.start_time,
            end_time: r.end_time,
        }).collect())
    }
    
    /// Load locked slots
    pub async fn load_locked_slots(
        &self,
        semester_id: Uuid,
        classroom_ids: &[Uuid],
    ) -> Result<Vec<LockedSlotData>, sqlx::Error> {
        let query = r#"
            SELECT 
                subject_id,
                day_of_week,
                period_ids,
                scope_type,
                scope_ids
            FROM timetable_locked_slots
            WHERE academic_semester_id = $1
        "#;
        
        let rows = sqlx::query_as::<_, LockedSlotRow>(query)
            .bind(semester_id)
            .fetch_all(self.pool)
            .await?;
        
        let mut locked_slots = Vec::new();
        
        for row in rows {
            // Parse period_ids from JSONB
            let period_ids: Vec<Uuid> = serde_json::from_value(row.period_ids)
                .unwrap_or_default();
            
            // Parse scope_ids
            let scope_ids: Vec<Uuid> = row.scope_ids
                .as_ref()
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            
            // Filter by scope
            let applicable_classrooms = match row.scope_type.as_str() {
                "ALL_SCHOOL" => classroom_ids.to_vec(),
                "CLASSROOM" => {
                    // Only include classrooms in both scope_ids and classroom_ids
                    scope_ids.iter()
                        .filter(|id| classroom_ids.contains(id))
                        .copied()
                        .collect()
                }
                "GRADE_LEVEL" => {
                    // TODO: Load classrooms by grade level
                    // For now, include all requested classrooms
                    classroom_ids.to_vec()
                }
                _ => vec![],
            };
            
            if !applicable_classrooms.is_empty() {
                locked_slots.push(LockedSlotData {
                    subject_id: row.subject_id,
                    day: row.day_of_week,
                    period_ids,
                    classroom_ids: applicable_classrooms,
                    scope_type: row.scope_type,
                });
            }
        }
        
        Ok(locked_slots)
    }
    
    /// Load instructor preferences
    pub async fn load_instructor_preferences(
        &self,
        academic_year_id: Uuid,
    ) -> Result<HashMap<Uuid, InstructorPrefData>, sqlx::Error> {
        // 1. Load periods to map index -> id
        let periods = self.load_periods(academic_year_id).await?;
        let period_map: HashMap<i32, Uuid> = periods.iter()
            .map(|p| (p.order, p.id)) 
            .collect();
            
        let resolve_period_id = |slot: &TimeSlotJson| -> Option<Uuid> {
            if let Some(id) = slot.period_id {
                return Some(id);
            }
            if let Some(idx) = slot.period_index {
                // Front end index 0 = Period 1 (order 1)
                return period_map.get(&(idx + 1)).copied(); 
            }
            None
        };

        let query = r#"
            SELECT 
                instructor_id,
                hard_unavailable_slots,
                preferred_slots,
                COALESCE(max_periods_per_day, 7) as max_periods_per_day
            FROM instructor_preferences
            WHERE academic_year_id = $1
        "#;
        
        let rows = sqlx::query_as::<_, InstructorPrefRow>(query)
            .bind(academic_year_id)
            .fetch_all(self.pool)
            .await?;
        
        let mut prefs = HashMap::new();
        
        for row in rows {
            // Parse hard unavailable slots
            let hard_unavailable: Vec<TimeSlotJson> = serde_json::from_value(
                row.hard_unavailable_slots
            ).unwrap_or_default();
            
            let mut hard_unavailable_set = HashSet::new();
            for slot in hard_unavailable {
                if let Some(pid) = resolve_period_id(&slot) {
                    hard_unavailable_set.insert(format!("{}__{}", slot.day, pid));
                }
            }
            
            // Parse preferred slots
            let preferred: Vec<TimeSlotJson> = serde_json::from_value(
                row.preferred_slots
            ).unwrap_or_default();
            
            let mut preferred_set = HashSet::new();
            for slot in preferred {
                if let Some(pid) = resolve_period_id(&slot) {
                    preferred_set.insert(format!("{}__{}", slot.day, pid));
                }
            }
            
            prefs.insert(row.instructor_id, InstructorPrefData {
                instructor_id: row.instructor_id,
                hard_unavailable: hard_unavailable_set,
                preferred_slots: preferred_set,
                max_periods_per_day: row.max_periods_per_day,
            });
        }
        
        Ok(prefs)
    }
    
    /// Load instructor room assignments
    async fn load_instructor_room_assignments(
        &self,
        semester_id: Uuid,
    ) -> Result<HashMap<Uuid, Uuid>, sqlx::Error> {
        // Filter by academic_year_id ของ semester
        let rows = sqlx::query_as::<_, (Uuid, Uuid)>(
            r#"SELECT ira.instructor_id, ira.room_id
               FROM instructor_room_assignments ira
               JOIN academic_semesters s ON s.academic_year_id = ira.academic_year_id
               WHERE s.id = $1"#
        )
        .bind(semester_id)
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().collect())
    }

    /// Load entries ที่มีอยู่แล้ว (Phase 1 fixed slots: BREAK/HOMEROOM/ACTIVITY/TEXT)
    /// แปลงเป็น LockedSlotData เพื่อให้ scheduler มองเป็น "ช่องที่ถูกจองแล้ว"
    /// Skip COURSE entries — auto-scheduler จะ regenerate (พร้อม force_overwrite)
    pub async fn load_existing_entries_as_locked(
        &self,
        semester_id: Uuid,
        classroom_ids: &[Uuid],
    ) -> Result<Vec<LockedSlotData>, sqlx::Error> {
        // 1 query — group by (classroom, day, period) → unique locks
        // skip COURSE เพราะ scheduler regenerate (ถ้า force_overwrite=true ยังไงก็ลบ)
        let rows = sqlx::query_as::<_, (Uuid, String, Uuid)>(
            r#"SELECT DISTINCT te.classroom_id, te.day_of_week, te.period_id
               FROM academic_timetable_entries te
               WHERE te.academic_semester_id = $1
                 AND te.classroom_id = ANY($2)
                 AND te.is_active = true
                 AND te.entry_type <> 'COURSE'"#
        )
        .bind(semester_id)
        .bind(classroom_ids)
        .fetch_all(self.pool)
        .await?;

        // Pack: 1 row per (classroom, day, period)
        // ใช้ Uuid::nil() เป็น sentinel "ไม่ใช่ subject ใด ๆ" — locked check ใน validator
        // ใช้ subject_id != course.subject_id → nil ≠ real subject → block ทุกวิชา
        let nil = Uuid::nil();
        let locked = rows.into_iter().map(|(classroom_id, day, period_id)| {
            LockedSlotData {
                subject_id: nil,
                day,
                period_ids: vec![period_id],
                classroom_ids: vec![classroom_id],
                scope_type: "EXISTING_ENTRY".to_string(),
            }
        }).collect();

        Ok(locked)
    }

    /// Load global setting: default_max_consecutive
    pub async fn load_default_max_consecutive(&self) -> Result<i32, sqlx::Error> {
        let val: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT value FROM scheduler_settings WHERE key = 'default_max_consecutive'"
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(val.and_then(|v| v.as_i64()).unwrap_or(4) as i32)
    }

    /// Load all rooms with details
    pub async fn load_rooms(&self) -> Result<HashMap<Uuid, RoomInfo>, sqlx::Error> {
        // COALESCE: บางห้องอาจไม่มีชื่อ (name_th NULL) → fallback เป็น code, แล้วเป็น "ห้อง"
        let query = r#"
            SELECT id,
                   COALESCE(NULLIF(name_th, ''), code, 'ห้อง') AS name,
                   room_type,
                   capacity
            FROM rooms
            WHERE status = 'ACTIVE'
        "#;

        let rows = sqlx::query_as::<_, RoomRow>(query)
            .fetch_all(self.pool)
            .await?;
            
        Ok(rows.into_iter().map(|r| (r.id, RoomInfo {
            id: r.id,
            name: r.name,
            room_type: r.room_type,
            capacity: r.capacity.unwrap_or(0),
        })).collect())
    }
}

// Database row types

#[derive(sqlx::FromRow)]
struct RoomRow {
    id: Uuid,
    name: String,
    room_type: Option<String>,
    capacity: Option<i32>,
}

#[derive(sqlx::FromRow)]
struct CourseRow {
    classroom_course_id: Uuid,
    classroom_id: Uuid,
    classroom_name: String,
    subject_id: Uuid,
    subject_code: String,
    subject_name: String,
    instructor_id: Option<Uuid>,
    instructor_name: Option<String>,
    periods_needed: i32,
    min_consecutive: i32,
    max_consecutive: i32,
    allow_single_period: bool,
    allowed_period_ids: Option<serde_json::Value>,         // JSONB
    allowed_days: Option<serde_json::Value>,               // JSONB
    consecutive_pattern: Option<serde_json::Value>,        // jsonb [1,1,1] etc.
    same_day_unique: bool,
    cc_hard_unavailable_slots: serde_json::Value,          // [{"day","period_id"}]
}

#[derive(sqlx::FromRow)]
struct IndepActivityRow {
    slot_id: Uuid,
    classroom_id: Uuid,
    classroom_name: String,
    activity_name: String,
    activity_type: String,
    periods_per_week: i32,
    instructor_id: Option<Uuid>,
    instructor_name: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PeriodRow {
    id: Uuid,
    period_order: i32,
    name: Option<String>,
    start_time: String,
    end_time: String,
}

#[derive(sqlx::FromRow)]
struct LockedSlotRow {
    subject_id: Uuid,
    day_of_week: String,
    period_ids: serde_json::Value,
    scope_type: String,
    scope_ids: Option<serde_json::Value>,
}

#[derive(sqlx::FromRow)]
struct InstructorPrefRow {
    instructor_id: Uuid,
    hard_unavailable_slots: serde_json::Value,
    preferred_slots: serde_json::Value,
    max_periods_per_day: i32,
}

#[derive(serde::Deserialize)]
struct TimeSlotJson {
    day: String,
    period_id: Option<Uuid>,
    period_index: Option<i32>,
}
