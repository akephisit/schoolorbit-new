# 🚀 Auto-Scheduling Quick Start Guide
## Get Started in 5 Minutes!

---

## 📋 Prerequisites

✅ Database migrations applied (039-043)  
✅ Backend compiled  
✅ Frontend running  
✅ At least 1 academic semester created  
✅ At least 1 classroom with courses  
✅ Periods (academic_periods) configured  

---

## 🎯 Quickest Path to Success

### **Method 1: Simple Test (2 minutes)**

**สำหรับการทดสอบครั้งแรก:**

1. **ไปที่หน้าจัดตาราง**
   ```
   /staff/academic/timetable/scheduling/auto-schedule
   ```

2. **เลือกห้องเรียน 1-2 ห้อง**
   - คลิก checkbox ห้องที่ต้องการ

3. **ใช้ค่า default**
   - Algorithm: Backtracking
   - Quality: 70%
   - Timeout: 120s

4. **กดปุ่ม "เริ่มจัดตาราง"**

5. **ดูผลลัพธ์**
   - ระบบจะพาไปหน้า Job Status
   - รอ 5-30 วินาที
   - ดูคะแนนคุณภาพ
   - ตรวจสอบตารางที่ได้

---

### **Method 2: Full Featured (10 minutes)**

**สำหรับการใช้งานจริง:**

#### **Step 1: Configure Subjects (3 นาที)**

```sql
-- ตัวอย่าง: พละต้อง 2 คาบติด
UPDATE subjects 
SET 
    min_consecutive_periods = 2,
    max_consecutive_periods = 2,
    allow_single_period = true,
    preferred_time_of_day = 'AFTERNOON',
    periods_per_week = 3
WHERE subject_type = 'PE';

-- ตัวอย่าง: LAB ต้อง 2-3 คาบติด
UPDATE subjects 
SET 
    min_consecutive_periods = 2,
    max_consecutive_periods = 3,
    allow_single_period = false,
    required_room_type = 'LAB',
    periods_per_week = 4
WHERE code LIKE 'LAB%';

-- ตัวอย่าง: วิชาทั่วไป
UPDATE subjects 
SET 
    min_consecutive_periods = 1,
    max_consecutive_periods = 2,
    allow_single_period = true,
    preferred_time_of_day = 'MORNING',
    periods_per_week = 4
WHERE subject_type = 'CORE';
```

#### **Step 2: Run Auto-Schedule (2 นาที)**

1. Go to `/staff/academic/timetable/scheduling/auto-schedule`
2. Select 5-10 classrooms
3. Choose settings:
   - Algorithm: **BACKTRACKING**
   - Quality: **80%**
   - Timeout: **120s**
   - ✅ Force overwrite (ถ้าต้องการเขียนทับเดิม)
4. Click **"เริ่มจัดตาราง"**

#### **Step 3: Monitor Progress (1 นาที)**

- หน้า Job Status จะ auto-refresh ทุก 2 วินาที
- ดู Progress: 0% → 20% → 100%
- ดู Quality Score: ควรได้ 75-90%
- ถ้าสำเร็จ → ไปดูตารางสอน

#### **Step 4: Review Results (2 นาที)**

```
📊 ตรวจสอบ:
✅ Quality Score >= 80%?
✅ Scheduled Courses = Total Courses?
✅ Failed Courses = 0?
✅ ตารางดูสมเหตุสมผล?

❌ ถ้าไม่พอใจ:
- ลองเพิ่ม timeout
- ลด quality threshold
- เปิด allow_partial
- ปรับ subject constraints
```

#### **Step 5: Fine-Tune (Optional, 2-5 นาที)**

**ถ้าต้องการผลลัพธ์ดีขึ้น:**

1. **Lock Important Periods**
   ```typescript
   // Example: Lock school assembly every Monday period 1
   await createLockedSlot({
     academic_semester_id: "...",
     scope_type: "ALL_SCHOOL",
     subject_id: assembly_subject_id,
     day_of_week: "MON",
     period_ids: [period_1_id],
     reason: "เข้าแถวยามเช้า"
   });
   ```

2. **Set Teacher Unavailability**
   ```typescript
   await createInstructorPreference({
     instructor_id: teacher_id,
     academic_year_id: "...",
     hard_unavailable_slots: [
       { day: "WED", period_id: period_5_id }, // ประชุมครู
     ],
     max_periods_per_day: 6
   });
   ```

3. **Assign Fixed Rooms**
   ```typescript
   await createInstructorRoomAssignment({
     instructor_id: teacher_id,
     room_id: lab_room_id,
     academic_year_id: "...",
     is_required: true,
     for_subjects: ["ปฏิบัติการวิทย์"],
     reason: "ห้องแล็บประจำ"
   });
   ```

4. **Re-run with Constraints**
   - กลับไปหน้า auto-schedule
   - Run อีกครั้ง
   - ผลลัพธ์จะดีขึ้น!

---

## 🔧 Troubleshooting

### **ปัญหา: Quality Score ต่ำ (< 70%)**

**สาเหตุ:**
- Constraints เยอะเกิน
- Periods ไม่พอ
- วิชาที่ต้องติดกันเยอะ

**แก้:**
- ✅ ลด min_quality_score → 60%
- ✅ เพิ่ม timeout → 300s
- ✅ เปิด allow_partial → true
- ✅ ลด consecutive requirements

---

### **ปัญหา: Failed Courses > 0**

**สาเหตุ:**
- Periods ไม่พอสำหรับวิชานั้น
- Instructor ไม่ว่างเลย
- Room ไม่มีเลย

**แก้:**
- ✅ เช็ค reason ของ failed course
- ✅ เพิ่ม periods ให้เพียงพอ
- ✅ ปรับ instructor unavailability
- ✅ เพิ่ม rooms

---

### **ปัญหา: Timeout ก่อนเสร็จ**

**สาเหตุ:**
- Classrooms เยอะเกิน
- Algorithm ช้า
- Constraints ซับซ้อน

**แก้:**
- ✅ เพิ่ม timeout → 600s
- ✅ เปลี่ยน algorithm → GREEDY
- ✅ แบ่ง classrooms ทำทีละกลุ่ม
- ✅ เปิด allow_partial

---

## 📊 Expected Results

### **Small Batch (1-5 classrooms)**
```
⏱️  Time: 3-15 seconds
📊 Quality: 85-95%
✅ Success: 100%
```

### **Medium Batch (6-15 classrooms)**
```
⏱️  Time: 15-60 seconds
📊 Quality: 75-90%
✅ Success: 95%
```

### **Large Batch (16-30 classrooms)**
```
⏱️  Time: 60-180 seconds
📊 Quality: 70-85%
✅ Success: 85-95%
```

---

## 💡 Pro Tips

### **Tip 1: Start Small**
ทดสอบกับ 1-2 ห้องก่อน → ปรับ settings → แล้วค่อยทำทั้งหมด

### **Tip 2: Use Locks Wisely**
Lock เฉพาะช่วงที่สำคัญจริงๆ (เข้าแถว, ประชุม) ไม่ใช่ทั้งหมด

### **Tip 3: Batch by Grade**
จัดทีละชั้น (ม.1 ก่อน, แล้วค่อย ม.2) จะง่ายกว่าจัดทั้งโรงเรียน

### **Tip 4: Review Subjects First**
ตรวจสอบ `periods_per_week` และ consecutive requirements ก่อน run

### **Tip 5: Be Flexible**
ถ้า quality 75% = ดีแล้ว! ไม่ต้องฝืนให้ 95% เสมอไป

---

## 🎯 Success Checklist

Before running auto-schedule:

- [x] Subjects have `periods_per_week` set
- [x] Subjects have consecutive requirements set (if needed)
- [x] Courses assigned to classrooms
- [x] academic_periods table has data
- [x] Selected appropriate algorithm
- [x] Set realistic quality threshold

After running:

- [x] Job completed successfully
- [x] Quality score acceptable (>= 70%)
- [x] No failed courses (or acceptable)
- [x] Timetable looks reasonable
- [x] No obvious conflicts

---

## 🆘 Need Help?

### **Check Logs**
```bash
# Backend logs
tail -f backend.log | grep scheduling

# Database
psql $DATABASE_URL -c "SELECT * FROM timetable_scheduling_jobs ORDER BY created_at DESC LIMIT 5;"
```

### **Verify Data**
```sql
-- Check subjects config
SELECT code, name_th, periods_per_week, 
       min_consecutive_periods, max_consecutive_periods, allow_single_period
FROM subjects
WHERE periods_per_week > 0;

-- Check periods
SELECT COUNT(*) FROM academic_periods WHERE is_active = true;

-- Check courses
SELECT COUNT(*) FROM classroom_courses WHERE academic_semester_id = '...';
```

---

## 🎉 You're Ready!

**ระบบพร้อมใช้งานแล้ว!**

1. ✅ Database schema ready
2. ✅ Scheduling engine working
3. ✅ API endpoints available
4. ✅ UI pages ready
5. ✅ Documentation complete

**ไปทดสอบได้เลย!** 🚀

→ `/staff/academic/timetable/scheduling/auto-schedule`

---

**Good luck!** 🍀  
**Last Updated**: 2026-02-08
