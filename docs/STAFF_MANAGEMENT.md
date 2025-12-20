# 🎓 ระบบจัดการบุคลากร (Staff Management System)

สร้างเมื่อ: 20 ธันวาคม 2567  
สถานะ: **✅ เสร็จสมบูรณ์ (MVP)**

---

## 📋 สรุปภาพรวม

ระบบจัดการบุคลากรแบบยืดหยุ่นที่รองรับ Multi-Role และ Multi-Department พร้อม Permission System ที่ครบถ้วน สำหรับโรงเรียนที่มีโครงสร้างองค์กรที่ซับซ้อน

### ✨ จุดเด่นของระบบ

- **Multi-Role Support**: บุคคลหนึ่งคนสามารถมีหลายบทบาทได้ (เช่น ครู + หัวหน้าฝ่าย + ธุรการ)
- **Multi-Department**: สังกัดหลายฝ่ายพร้อมกัน พร้อมระบุตำแหน่งในแต่ละฝ่าย
- **Permission-based**: ควบคุมสิทธิ์การเข้าถึงตาม Role
- **Teaching Assignments**: จัดการวิชาที่สอนแยกตามปีการศึกษาและเทอม
- **Historical Data**: เก็บประวัติการดำรงตำแหน่ง (started_at/ended_at)

---

## 🗄️ Database Schema

### Core Tables

#### 1. **users** (ข้อมูลผู้ใช้พื้นฐาน)
```sql
- id (UUID)
- national_id (เลขบัตรปชช.)
- email
- password_hash
- first_name, last_name, nickname
- user_type ('student', 'staff', 'parent')
- title (คำนำหน้า)
- phone, emergency_contact, line_id
- date_of_birth, gender, address
- status ('active', 'inactive', 'suspended', 'resigned', 'retired')
- hired_date, resigned_date
- metadata (JSONB)
```

#### 2. **roles** (บทบาท/ตำแหน่งในระบบ)
```sql
- id (UUID)
- code (รหัส เช่น 'TEACHER', 'DIRECTOR')
- name (ชื่อภาษาไทย)
- name_en (ชื่อภาษาอังกฤษ)
- category ('administrative', 'teaching', 'operational', 'support')
- level (ระดับอำนาจ 0-999)
- permissions (JSONB array)
- is_active
```

**Default Roles:**
- `TEACHER` (ครูผู้สอน) - level 10
- `DEPT_HEAD` (หัวหน้าฝ่าย) - level 50
- `VICE_DIRECTOR` (รองผู้อำนวยการ) - level 80
- `DIRECTOR` (ผู้อำนวยการ) - level 100
- `SECRETARY` (ธุรการ) - level 20
- `LIBRARIAN` (บรรณารักษ์) - level 15
- `ADMIN` (ผู้ดูแลระบบ) - level 999

#### 3. **user_roles** (ความสัมพันธ์ User-Role)
```sql
- id (UUID)
- user_id → users
- role_id → roles
- is_primary (บทบาทหลัก)
- started_at, ended_at
- notes
```

#### 4. **departments** (ฝ่าย/แผนก)
```sql
- id (UUID)
- code (รหัส เช่น 'ACADEMIC')
- name (ชื่อภาษาไทย)
- name_en (ชื่อภาษาอังกฤษ)
- parent_department_id (ฝ่ายแม่)
- phone, email, location
- is_active
- display_order
```

**Default Departments:**
- `ACADEMIC` (ฝ่ายวิชาการ)
- `STUDENT_AFFAIRS` (ฝ่ายกิจการนักเรียน)
- `ADMINISTRATION` (ฝ่ายบริหารทั่วไป)
- `FINANCE` (ฝ่ายการเงิน)
- `LIBRARY` (ห้องสมุด)

#### 5. **department_members** (สมาชิกในฝ่าย)
```sql
- id (UUID)
- user_id → users
- department_id → departments
- position ('head', 'deputy_head', 'member', 'coordinator')
- is_primary_department
- responsibilities (หน้าที่รับผิดชอบ)
- started_at, ended_at
```

#### 6. **teaching_assignments** (การมอบหมายการสอน)
```sql
- id (UUID)
- teacher_id → users
- class_id → classes
- subject (วิชา)
- grade_level (ระดับชั้น)
- hours_per_week
- teacher_type ('main_teacher', 'co_teacher', 'substitute')
- is_homeroom_teacher
- academic_year, semester
- started_at, ended_at
```

#### 7. **staff_info** (ข้อมูลเฉพาะบุคลากร)
```sql
- id (UUID)
- user_id → users
- employee_id (รหัสพนักงาน)
- employment_type ('permanent', 'contract', 'temporary', 'part_time')
- education_level (วุฒิการศึกษา)
- major (สาขา), university
- teaching_license_number, teaching_license_expiry
- salary, bank_account, bank_name
- tax_id, social_security_id
- work_days (JSONB array)
- work_hours_start, work_hours_end
```

#### 8. **student_info** (ข้อมูลเฉพาะนักเรียน)
```sql
- id (UUID)
- user_id → users
- student_id (รหัสนักเรียน)
- grade_level, class_room, student_number
- parent_id → users
- enrollment_date, expected_graduation_date
- blood_type, allergies, medical_conditions
```

#### 9. **parent_info** (ข้อมูลเฉพาะผู้ปกครอง)
```sql
- id (UUID)
- user_id → users
- relationship ('father', 'mother', 'guardian')
- occupation, workplace, work_phone
- monthly_income
```

#### 10. **permissions** (สิทธิ์การใช้งาน)
```sql
- id (UUID)
- code (เช่น 'users.create', 'documents.approve')
- name (ชื่อภาษาไทย)
- module (หมวดหมู่)
- action ('view', 'create', 'edit', 'delete', 'approve')
```

---

## 🔌 Backend APIs

### Base URL
```
http://localhost:8081
```

### Authentication
ทุก API ต้องใส่ cookie `auth_token` (ยกเว้น login)

### Staff Management APIs

#### 1. List Staff
```http
GET /api/staff?search={query}&status={status}&page={n}&page_size={n}
```

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "id": "uuid",
      "employee_id": "EMP001",
      "first_name": "สมชาย",
      "last_name": "ใจดี",
      "roles": ["ครูผู้สอน"],
      "departments": ["ฝ่ายวิชาการ"],
      "status": "active"
    }
  ],
  "total": 100,
  "page": 1,
  "page_size": 20,
  "total_pages": 5
}
```

#### 2. Get Staff Profile
```http
GET /api/staff/{id}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "uuid",
    "national_id": "1234567890123",
    "email": "somchai@school.com",
    "title": "นาย",
    "first_name": "สมชาย",
    "last_name": "ใจดี",
    "nickname": "โจ้",
    "phone": "081-234-5678",
    "user_type": "staff",
    "status": "active",
    "staff_info": {
      "employee_id": "EMP001",
      "employment_type": "permanent",
      "education_level": "ปริญญาโท",
      "major": "คณิตศาสตร์ศึกษา",
      "university": "มหาวิทยาลัยเชียงใหม่"
    },
    "roles": [
      {
        "id": "uuid",
        "code": "TEACHER",
        "name": "ครูผู้สอน",
        "category": "teaching",
        "level": 10,
        "is_primary": true
      },
      {
        "id": "uuid",
        "code": "DEPT_HEAD",
        "name": "หัวหน้าฝ่าย",
        "category": "administrative",
        "level": 50,
        "is_primary": false
      }
    ],
    "departments": [
      {
        "id": "uuid",
        "code": "ACADEMIC",
        "name": "ฝ่ายวิชาการ",
        "position": "head",
        "is_primary_department": true
      }
    ],
    "teaching_assignments": [
      {
        "id": "uuid",
        "subject": "คณิตศาสตร์",
        "grade_level": "ม.1",
        "class_code": "M1-1",
        "class_name": "ม.1/1",
        "is_homeroom_teacher": true,
        "hours_per_week": 5,
        "academic_year": "2567",
        "semester": "1"
      }
    ],
    "permissions": [
      "users.view",
      "grades.edit",
      "attendance.mark",
      "documents.approve_dept"
    ]
  }
}
```

#### 3. Create Staff
```http
POST /api/staff
Content-Type: application/json

{
  "national_id": "1234567890123",
  "email": "somchai@school.com",
  "password": "Password123!",
  "title": "นาย",
  "first_name": "สมชาย",
  "last_name": "ใจดี",
  "nickname": "โจ้",
  "phone": "081-234-5678",
  "hired_date": "2024-01-01",
  "staff_info": {
    "employee_id": "EMP001",
    "employment_type": "permanent",
    "education_level": "ปริญญาโท",
    "major": "คณิตศาสตร์ศึกษา",
    "university": "มหาวิทยาลัยเชียงใหม่"
  },
  "role_ids": ["uuid-role-teacher", "uuid-role-dept-head"],
  "primary_role_id": "uuid-role-teacher",
  "department_assignments": [
    {
      "department_id": "uuid-dept-academic",
      "position": "head",
      "is_primary": true,
      "responsibilities": "บริหารงานวิชาการ"
    }
  ]
}
```

**Response:**
```json
{
  "success": true,
  "message": "สร้างบุคลากรสำเร็จ",
  "data": {
    "id": "uuid"
  }
}
```

#### 4. Update Staff
```http
PUT /api/staff/{id}
Content-Type: application/json

{
  "title": "ดร.",
  "first_name": "สมชาย",
  "last_name": "ใจดี",
  "phone": "081-234-5678"
}
```

#### 5. Delete Staff (Soft Delete)
```http
DELETE /api/staff/{id}
```

**Response:**
```json
{
  "success": true,
  "message": "ลบบุคลากรสำเร็จ"
}
```

### Role Management APIs

#### 1. List Roles
```http
GET /api/roles
```

#### 2. Get Role
```http
GET /api/roles/{id}
```

#### 3. Create Role
```http
POST /api/roles
Content-Type: application/json

{
  "code": "LIBRARIAN",
  "name": "บรรณารักษ์",
  "name_en": "Librarian",
  "description": "จัดการห้องสมุด",
  "category": "operational",
  "level": 15,
  "permissions": ["library.manage", "users.view"]
}
```

#### 4. Update Role
```http
PUT /api/roles/{id}
```

### Department Management APIs

#### 1. List Departments
```http
GET /api/departments
```

#### 2. Get Department
```http
GET /api/departments/{id}
```

#### 3. Create Department
```http
POST /api/departments
Content-Type: application/json

{
  "code": "IT",
  "name": "ฝ่ายเทคโนโลยีสารสนเทศ",
  "name_en": "IT Department",
  "description": "จัดการระบบคอมพิวเตอร์",
  "phone": "053-123456",
  "email": "it@school.ac.th",
  "location": "อาคาร 2 ชั้น 1"
}
```

#### 4. Update Department
```http
PUT /api/departments/{id}
```

---

## 🎨 Frontend Pages

### 1. Staff List Page
**Path:** `/staff`

**Features:**
- ✅ แสดงรายชื่อบุคลากรทั้งหมด
- ✅ Search (ค้นหาชื่อ, นามสกุล, รหัสพนักงาน)
- ✅ Pagination
- ✅ Status badge (active/inactive)
- ✅ Quick actions (View, Edit, Delete)
- ✅ Responsive design

**Screenshot:** (รอ capture)

### 2. Staff Profile Page
**Path:** `/staff/{id}`

**Features:**
- ✅ แสดงข้อมูลส่วนตัวครบถ้วน
- ✅ แสดงบทบาทและตำแหน่ง (พร้อม badge หลัก)
- ✅ แสดงฝ่ายที่สังกัด (พร้อมตำแหน่งในฝ่าย)
- ✅ แสดงวิชาที่สอน (สำหรับครู)
- ✅ แสดงข้อมูลการศึกษาและการทำงาน
- ✅ Edit button

**Screenshot:** (รอ capture)

### 3. Staff Create/Edit Form
**Path:** `/staff/new`, `/staff/{id}/edit`

**Status:** 🚧 To be implemented

**Features (Planned):**
- Multi-step form wizard
- Role selection (multi-select)
- Department assignment
- Teaching assignment (for teachers)
- Form validation
- Auto-complete fields

---

## 📁 File Structure

### Backend (Rust)
```
backend-school/
├── migrations/
│   └── 005_create_staff_management.sql
├── src/
│   ├── handlers/
│   │   ├── staff.rs       # Staff CRUD handlers
│   │   ├── roles.rs       # Role & Department handlers
│   │   └── mod.rs
│   ├── models/
│   │   ├── staff.rs       # All models & types
│   │   └── mod.rs
│   └── main.rs            # Routes configuration
```

### Frontend (SvelteKit)
```
frontend-school/
└── src/
    ├── lib/
    │   └── api/
    │       └── staff.ts           # API client
    └── routes/
        └── (app)/
            └── staff/
                ├── +page.svelte       #  List page
                └── [id]/
                    └── +page.svelte   # Profile page
```

---

## 🔄 ตัวอย่างการใช้งาน (Use Cases)

### Use Case 1: ครูสมชาย - ครูที่สอน + หัวหน้าฝ่ายวิชาการ + ครูที่ปรึกษา

**ข้อมูลในระบบ:**
```
users:
  - ชื่อ: นายสมชาย ใจดี
  - รหัสพนักงาน: EMP001
  - user_type: staff

user_roles:
  - TEACHER (primary)
  - DEPT_HEAD

department_members:
  - ฝ่ายวิชาการ (หัวหน้าฝ่าย, primary)

teaching_assignments:
  - คณิตศาสตร์ ม.1/1 (ครูที่ปรึกษา)
 - คณิตศาสตร์ ม.1/2
```

**Permissions:**
- users.view, users.edit
- students.view
- grades.view, grades.edit
- attendance.mark
- documents.approve_dept

---

### Use Case 2: ครูสมหญิง - ครูสอน + ธุรการ + รองหัวหน้าห้องสมุด

**ข้อมูลในระบบ:**
```
user_roles:
  - TEACHER (primary)
  - SECRETARY
  - LIBRARIAN

department_members:
  - ฝ่ายวิชาการ (member)
  - ฝ่ายบริหารทั่วไป (member)
  - ห้องสมุด (deputy_head)

teaching_assignments:
  - ภาษาไทย ม.2/1
```

---

### Use Case 3: ผู้อำนวยการ - บริหาร + อนุมัติ + สอน (บางครั้ง)

**ข้อมูลในระบบ:**
```
user_roles:
  - DIRECTOR (primary, level 100)
  - TEACHER

department_members:
  - ฝ่ายบริหารทั่วไป (head)

teaching_assignments:
  - หน้าที่พลเมือง ม.6 (co_teacher)

Permissions:
  - users.* (all)
  - documents.approve
  - finance.approve
  - และอื่นๆ ทั้งหมด
```

---

## 🚀 การ Deploy

### Prerequisites
1. PostgreSQL 14+
2. Rust 1.70+
3. Node.js 18+

### Backend Setup
```bash
cd backend-school

# Set environment variables
cp .env.example .env
# แก้ไข DATABASE_URL

# Run migrations
sqlx migrate run

# Build & Run
cargo run --release
```

**Server จะรันที่:** `http://localhost:8081`

### Frontend Setup
```bash
cd frontend-school

# Install dependencies
npm install

# Set environment variables
echo "VITE_API_URL=http://localhost:8081" > .env

# Run dev server
npm run dev
```

**UI จะรันที่:** `http://localhost:5173`

---

## ✅ Checklist

### Backend
- [x] Database migration (005_create_staff_management.sql)
- [x] Models & Types (staff.rs)
- [x] Staff CRUD handlers
- [x] Role CRUD handlers
- [x] Department CRUD handlers
- [x] API routes configuration
- [x] Compile successfully
- [x] No SQL injection vulnerabilities (using bind parameters)

### Frontend
- [x] API client (staff.ts)
- [x] Staff List UI
- [x] Staff Profile UI
- [x] Sidebar navigation
- [x] TypeScript types
- [x] svelte-check pass
- [ ] Staff Create/Edit form (TODO)
- [ ] Role management UI (TODO)
- [ ] Department management UI (TODO)

---

## 🎯 Next Steps (แนะนำ)

### Phase 1: เสร็จแล้ว ✅
- [x] Database design
- [x] Backend APIs (Staff, Role, Department)
- [x] Frontend List & Profile pages

### Phase 2: ต่อไป (สำคัญ)
1. **Create/Edit Staff Form**
   - Multi-step wizard
   - Role & Department selector
   - Teaching assignment builder
   - Form validation

2. **Permission Middleware**
   - Check permissions from user's roles
   - Protect API endpoints
   - Frontend permission-based UI

3. **Testing**
   - Unit tests (Rust)
   - Integration tests (API)
   - E2E tests (Frontend)

### Phase 3: Enhancement
1. **Advanced Features**
   - Bulk import (CSV/Excel)
   - Export to PDF
   - Advanced search & filters
   - Staff analytics dashboard

2. **Document Management Integration**
   - Workflow approval system
   - Document routing based on roles/departments

3. **Performance**
   - Add database indexes
   - Implement caching
   - Optimize queries

---

## 🐛 Known Issues & Limitations

1. **Frontend Form ยังไม่มี** - ต้องสร้างหน้าฟอร์มเพิ่ม/แก้ไข
2. **Permission Checking ยังไม่ได้ใช้** - Backend มี permissions แต่ยังไม่ได้ enforce
3. **File Upload ยังไม่มี** - รูปภาพ profile ยังไม่ได้ implement
4. **Audit Log ยังไม่มี** - ไม่มีการบันทึกการเปลี่ยนแปลงข้อมูล
5. **Soft Delete Only** - ไม่มี hard delete (ตั้งใจ เพื่อเก็บประวัติ)

---

## 📚 References

- [Rust Axum Documentation](https://docs.rs/axum)
- [SQLx Documentation](https://docs.rs/sqlx)
- [SvelteKit Documentation](https://kit.svelte.dev)
- [PostgreSQL Documentation](https://www.postgresql.org/docs/)

---

## 👥 Contributors

- **Your Name** - Initial work

---

## 📄 License

Proprietary - SchoolOrbit Project

---

**Last Updated:** 20 ธันวาคม 2567  
**Version:** 1.0.0
