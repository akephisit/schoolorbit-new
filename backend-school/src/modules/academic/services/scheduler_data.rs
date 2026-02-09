use sqlx::PgPool;
use uuid::Uuid;
use crate::modules::academic::services::scheduler::{
    types::*,
    validator::LockedSlotData,
};
use std::collections::{HashMap, HashSet};

/// Load data from database for scheduler
pub struct SchedulerDataLoader<'a> {
    pool: &'a PgPool,
}

impl<'a> SchedulerDataLoader<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
    
    /// Load courses to schedule for given classrooms and semester
    pub async fn load_courses(
        &self,
        classroom_ids: &[Uuid],
        semester_id: Uuid,
    ) -> Result<Vec<CourseToSchedule>, sqlx::Error> {
        let query = r#"
            SELECT 
                cc.id as classroom_course_id,
                cc.classroom_id,
                cls.name as classroom_name,
                cc.subject_id,
                s.code as subject_code,
                s.name_th as subject_name,
                cc.primary_instructor_id as instructor_id,
                u.first_name || ' ' || u.last_name as instructor_name,
                
                -- Period requirements
                COALESCE(s.periods_per_week, 
                    CASE WHEN s.hours_per_semester > 0 THEN CEIL(s.hours_per_semester::float / 20.0)::int
                         WHEN s.credit > 0 THEN CEIL(s.credit * 2.0)::int
                         ELSE 2
                    END) as periods_needed,
                
                -- Consecutive requirements
                COALESCE(s.min_consecutive_periods, 1) as min_consecutive,
                2 as max_consecutive,
                COALESCE(s.allow_single_period, true) as allow_single_period,
                
                -- Room requirements
                s.required_room_type,
                
                -- Time preferences
                s.preferred_time_of_day
                
            FROM classroom_courses cc
            JOIN subjects s ON s.id = cc.subject_id
            JOIN class_rooms cls ON cls.id = cc.classroom_id
            LEFT JOIN users u ON u.id = cc.primary_instructor_id
            WHERE cc.classroom_id = ANY($1)
              AND cc.academic_semester_id = $2
            ORDER BY s.code
        "#;
        
        let rows = sqlx::query_as::<_, CourseRow>(query)
            .bind(classroom_ids)
            .bind(semester_id)
            .fetch_all(self.pool)
            .await?;
        
        // Load instructor room assignments
        let instructor_rooms = self.load_instructor_room_assignments(semester_id).await?;
        
        // Convert to CourseToSchedule
        let mut courses = Vec::new();
        for row in rows {
            let fixed_room_id = if let Some(instructor_id) = row.instructor_id {
                instructor_rooms.get(&instructor_id).copied()
            } else {
                None
            };
            
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
                required_room_type: row.required_room_type,
                fixed_room_id,
                preferred_time_of_day: row.preferred_time_of_day,
            });
        }
        
        Ok(courses)
    }
    
    /// Load available time slots from periods
    pub async fn load_available_slots(&self) -> Result<Vec<TimeSlot>, sqlx::Error> {
        let query = r#"
            SELECT id, order_index as period_order, name, start_time::text, end_time::text
            FROM academic_periods
            WHERE is_active = true
            ORDER BY order_index
        "#;
        
        let periods = sqlx::query_as::<_, PeriodRow>(query)
            .fetch_all(self.pool)
            .await?;
        
        let mut slots = Vec::new();
        let days = ["MON", "TUE", "WED", "THU", "FRI"];
        
        for day in days {
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
    
    /// Load periods info
    pub async fn load_periods(&self) -> Result<Vec<PeriodInfo>, sqlx::Error> {
        let query = r#"
            SELECT id, order_index as period_order, name, start_time::text, end_time::text
            FROM academic_periods
            WHERE is_active = true
            ORDER BY order_index
        "#;
        
        let rows = sqlx::query_as::<_, PeriodRow>(query)
            .fetch_all(self.pool)
            .await?;
        
        Ok(rows.into_iter().map(|r| PeriodInfo {
            id: r.id,
            order: r.period_order,
            name: r.name,
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
        let periods = self.load_periods().await?;
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
        _semester_id: Uuid,
    ) -> Result<HashMap<Uuid, Uuid>, sqlx::Error> {
        let query = r#"
            SELECT instructor_id, room_id
            FROM instructor_room_assignments
            -- WHERE is_required = true (Assuming all assignments in this table are binding for now)
        "#;
        
        let rows = sqlx::query_as::<_, (Uuid, Uuid)>(query)
            .fetch_all(self.pool)
            .await?;
        
        Ok(rows.into_iter().collect())
    }

    /// Load all rooms with details
    pub async fn load_rooms(&self) -> Result<HashMap<Uuid, RoomInfo>, sqlx::Error> {
        let query = r#"
            SELECT id, name, room_type, capacity
            FROM rooms
            WHERE is_active = true
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
    required_room_type: Option<String>,
    preferred_time_of_day: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PeriodRow {
    id: Uuid,
    period_order: i32,
    name: String,
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
