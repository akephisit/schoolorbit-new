# 📋 SchoolOrbit - Project Plan & Progress

สร้างเมื่อ: 20 ธันวาคม 2567  
อัปเดตล่าสุด: 20 ธันวาคม 2567 23:32

---

## 🎯 Project Vision

**SchoolOrbit** คือระบบบริหารจัดการโรงเรียนแบบ Multi-Tenant SaaS ที่ครอบคลุมทุกด้านของการบริหารโรงเรียน ตั้งแต่การจัดการบุคลากร นักเรียน ตารางเรียน คะแนน ไปจนถึงการเงินและเอกสาร

### Core Features (Target)
1. ✅ **Multi-Tenant Architecture** - แยก database ต่อโรงเรียน
2. ✅ **Authentication & Authorization** - Login, JWT, Role-based permissions
3. 🔄 **Staff Management** - จัดการบุคลากร (In Progress - 80%)
4. ⏳ **Student Management** - จัดการนักเรียน
5. ⏳ **Attendance System** - เช็คชื่อ
6. ⏳ **Grading System** - คะแนนและผลการเรียน
7. ⏳ **Timetable Management** - ตารางเรียน/สอน
8. ⏳ **Finance Management** - การเงิน
9. ⏳ **Document Management** - เอกสารและระบบอนุมัติ
10. ⏳ **Parent Portal** - ระบบสำหรับผู้ปกครอง

---

## 📊 Overall Progress: **35%**

```
[████████████░░░░░░░░░░░░░░░░░░░░░░] 35%

Infrastructure:     ████████████████████ 100%
Authentication:     ████████████████████ 100%  
Staff Management:   ████████████████░░░░  80%
Student Management: ░░░░░░░░░░░░░░░░░░░░   0%
Attendance:         ░░░░░░░░░░░░░░░░░░░░   0%
Grading:            ░░░░░░░░░░░░░░░░░░░░   0%
Timetable:          ░░░░░░░░░░░░░░░░░░░░   0%
Finance:            ░░░░░░░░░░░░░░░░░░░░   0%
Documents:          ░░░░░░░░░░░░░░░░░░░░   0%
Parent Portal:      ░░░░░░░░░░░░░░░░░░░░   0%
```

---

# 📅 Timeline & Milestones

## Phase 0: Setup & Infrastructure ✅ (Dec 1-10, 2024)

### ✅ Completed Tasks

#### 1. Project Setup
- [x] Initialize monorepo structure
- [x] Setup Rust backend (`backend-admin`, `backend-school`)
- [x] Setup SvelteKit frontend (`frontend-admin`, `frontend-school`)
- [x] Configure Tailwind CSS + shadcn/ui
- [x] Git repository initialization

#### 2. Database Architecture
- [x] Design multi-tenant architecture
- [x] Setup PostgreSQL (Neon)
- [x] Create `backend-admin` database (schools registry)
- [x] Per-tenant databases (isolated per school)
- [x] SQLx migrations setup

**Migration Files Created:**
```
backend-admin/migrations/
├── 001_create_schools.sql
└── 002_create_deployments.sql

backend-school/migrations/
├── 001_create_users.sql
├── 002_create_classes.sql
├── 003_create_enrollments.sql
└── 004_create_attendance.sql (basic)
```

#### 3. Backend Infrastructure
- [x] Axum web framework setup
- [x] JWT authentication
- [x] Cookie-based sessions
- [x] CORS configuration (via nginx)
- [x] Connection pooling (per-tenant)
- [x] Migration tracker system
- [x] Lazy pool loading
- [x] Internal API authentication

**Key Files:**
```
backend-school/src/
├── main.rs
├── db/
│   ├── mod.rs
│   ├── pool_manager.rs
│   └── school_mapping.rs
├── middleware/
│   ├── auth.rs
│   └── internal_auth.rs
└── utils/
    └── subdomain.rs
```

#### 4. Authentication System
- [x] Login/Logout APIs
- [x] Password hashing (bcrypt)
- [x] JWT token generation
- [x] Auth middleware
- [x] Get current user endpoint

**APIs:**
- `POST /api/auth/login`
- `POST /api/auth/logout`
- `GET /api/auth/me`

#### 5. School Provisioning System
- [x] Admin panel for creating schools
- [x] Subdomain-based routing
- [x] Database provisioning API
- [x] Migration management
- [x] GitHub Actions integration (deploy frontend)
- [x] SSE (Server-Sent Events) for real-time logs

**Admin Features:**
```
frontend-admin/
├── School List
├── Create School Form
├── Delete School (with confirmation)
├── Deploy School (GitHub Actions)
├── Real-time deployment logs (SSE)
└── Migration status dashboard
```

#### 6. Frontend Foundation
- [x] Layout components (Sidebar, Header)
- [x] Dashboard landing page
- [x] Login page
- [x] Responsive design
- [x] Dark mode support (準備済み)
- [x] Form components (bits-ui)
- [x] Icon system (lucide-svelte)

#### 7. DevOps & CI/CD
- [x] GitHub Actions workflow
  - Deploy school frontend to Cloudflare Workers
  - Automatic subdomain routing
- [x] Environment variables management
- [x] Docker configuration (backend)
- [x] Nginx reverse proxy config

**Workflows:**
```
.github/workflows/
├── deploy-admin.yml (manual deploy)
└── deploy-school-tenant.yml (triggered by API)
```

---

## Phase 1: Staff Management System ✅ 80% (Dec 15-20, 2024)

### ✅ Completed Tasks

#### 1. Database Design & Migration
- [x] Enhanced `users` table (multi-user-type support)
- [x] `roles` table (role definitions)
- [x] `user_roles` table (many-to-many with time periods)
- [x] `departments` table (organizational structure)
- [x] `department_members` table (staff assignments)
- [x] `teaching_assignments` table (teacher schedules)
- [x] `staff_info` table (staff-specific data)
- [x] `student_info` table (student-specific data)
- [x] `parent_info` table (parent-specific data)
- [x] `permissions` table (granular permissions)

**Migration:** `005_create_staff_management.sql` (700+ lines)

**Default Data Inserted:**
- 7 Default Roles (Teacher, Director, Vice Director, etc.)
- 5 Default Departments (Academic, Finance, etc.)
- 19 Default Permissions

**Key Features:**
- Multi-role support per user
- Multi-department membership
- Historical data (started_at/ended_at)
- Permission-based access control (prepared)

#### 2. Backend APIs (Rust)
- [x] Staff CRUD handlers
  - `GET /api/staff` - List with pagination & search
  - `GET /api/staff/:id` - Get full profile
  - `POST /api/staff` - Create (with transaction)
  - `PUT /api/staff/:id` - Update
  - `DELETE /api/staff/:id` - Soft delete

- [x] Role Management APIs
  - `GET /api/roles` - List all
  - `GET /api/roles/:id` - Get one
  - `POST /api/roles` - Create
  - `PUT /api/roles/:id` - Update

- [x] Department Management APIs
  - `GET /api/departments` - List all
  - `GET /api/departments/:id` - Get one
  - `POST /api/departments` - Create
  - `PUT /api/departments/:id` - Update

**Code Files:**
```
backend-school/src/
├── handlers/
│   ├── staff.rs (700 lines)
│   └── roles.rs (600 lines)
└── models/
    └── staff.rs (500 lines)
```

**Features:**
- Query-based (runtime) instead of macro (compile-time)
- Transaction support for data integrity
- Comprehensive error handling
- Subdomain-based multi-tenancy

#### 3. Frontend UI (SvelteKit)
- [x] API Client (`staff.ts`)
  - 13 functions with TypeScript types
  - Proper error handling
  - Cookie-based authentication

- [x] Staff List Page (`/staff`)
  - Pagination (20 per page)
  - Search (name, employee_id)
  - Status badges (active/inactive)
  - Quick actions (View, Edit, Delete)
  - Responsive table design
  - Empty states
  - Loading states

- [x] Staff Profile Page (`/staff/:id`)
  - 2-column layout
  - Personal information card
  - Staff work info card
  - Roles display (with primary badge)
  - Departments display (with position)
  - Teaching assignments (for teachers)
  - Edit button

- [x] Navigation
  - Added "บุคลากร" menu in Sidebar
  - Proper routing

**Code Files:**
```
frontend-school/src/
├── lib/api/
│   └── staff.ts (400 lines)
└── routes/(app)/staff/
    ├── +page.svelte (300 lines - List)
    └── [id]/
        └── +page.svelte (500 lines - Profile)
```

#### 4. Documentation
- [x] `STAFF_MANAGEMENT.md` (700+ lines)
  - Complete database schema documentation
  - API documentation with examples
  - Use cases (3 scenarios)
  - Deployment guide
  - Architecture overview
  - Known issues & limitations

### ⏳ Remaining Tasks (20%)

#### 1. Staff Create/Edit Form 🔥 Priority HIGH
- [ ] Create Staff Form (`/staff/new`)
- [ ] Edit Staff Form (`/staff/:id/edit`)
- [ ] Multi-step wizard
  - Step 1: Personal Information
  - Step 2: Role Assignment
  - Step 3: Department Assignment  
  - Step 4: Teaching Assignment (if teacher)
- [ ] Form validation (Zod/Yup)
- [ ] Role multi-select component
- [ ] Department assignment builder
- [ ] Teaching schedule builder
- [ ] Image upload (profile picture)

**Estimated Time:** 4-5 hours

#### 2. Permission System Integration 🔥 Priority HIGH
- [ ] Backend permission middleware
  - Check user permissions from roles
  - Protect API endpoints by permission
  - Return 403 if unauthorized
- [ ] Frontend permission hooks
  - `hasPermission()` function
  - Conditional UI rendering
  - Hide/disable buttons based on permissions
- [ ] Permission testing

**Estimated Time:** 2-3 hours

#### 3. Role/Department Management UI
- [ ] Role List page (`/settings/roles`)
- [ ] Create/Edit Role form
- [ ] Department List page (`/settings/departments`)
- [ ] Create/Edit Department form
- [ ] Department hierarchy tree view

**Estimated Time:** 2-3 hours

#### 4. Additional Features
- [ ] Bulk import (CSV/Excel)
- [ ] Export to PDF/Excel
- [ ] Advanced filters (by role, department, status)
- [ ] Staff analytics dashboard
- [ ] Audit log (track changes)

**Estimated Time:** 6-8 hours

---

## Phase 2: Student Management System ⏳ 0% (Target: Dec 21-25)

### 🎯 Goals
Complete student management system similar to staff management

### 📋 Tasks

#### 1. Database & Backend
- [ ] Enhance `student_info` table (already exists)
- [ ] Create guardian relationship table
- [ ] Class enrollment management
- [ ] Student CRUD APIs
  - `GET /api/students` - List with filters
  - `GET /api/students/:id` - Get profile
  - `POST /api/students` - Register new student
  - `PUT /api/students/:id` - Update
  - `DELETE /api/students/:id` - Soft delete
- [ ] Enrollment APIs
  - Enroll student to class
  - Transfer student
  - Graduation handling

**Estimated Time:** 3-4 hours

#### 2. Frontend UI
- [ ] Student List page (`/students`)
- [ ] Student Profile page (`/students/:id`)
- [ ] Register Student form
- [ ] Edit Student form
- [ ] Enrollment history view
- [ ] Parent/Guardian information section
- [ ] Medical info display

**Estimated Time:** 4-5 hours

#### 3. Parent Portal Access
- [ ] Parent user type support
- [ ] View own children's data
- [ ] Parent login page
- [ ] Limited permissions

**Estimated Time:** 2-3 hours

---

## Phase 3: Attendance System ⏳ 0% (Target: Dec 26-28)

### 🎯 Goals
Digital attendance tracking for all classes

### 📋 Tasks

#### 1. Database & Backend
- [ ] Enhanced `attendance` table
  - Daily attendance
  - Class period attendance
- [ ] Attendance APIs
  - `POST /api/attendance` - Mark attendance
  - `GET /api/attendance/class/:id` - Get class attendance
  - `GET /api/attendance/student/:id` - Get student history
  - `GET /api/attendance/reports` - Statistics
- [ ] Absence management
  - Leave requests
  - Medical certificates
  - Excuse system

**Estimated Time:** 3-4 hours

#### 2. Frontend UI
- [ ] Attendance marking page (teacher view)
  - Quick mark all present
  - Individual mark (Present/Absent/Late/Excused)
  - QR code check-in
- [ ] Student attendance history
- [ ] Attendance reports
  - By class
  - By student
  - By date range
- [ ] Parent notification (when absent)

**Estimated Time:** 4-5 hours

---

## Phase 4: Grading System ⏳ 0% (Target: Dec 29-31)

### 🎯 Goals
Complete grade management and transcript generation

### 📋 Tasks

#### 1. Database & Backend
- [ ] `grades` table
  - Subject
  - Semester
  - Grade components (midterm, final, assignments)
  - Weighted scores
- [ ] `grade_components` table (configurable)
- [ ] Grade calculation rules
- [ ] Grading APIs
  - Enter grades
  - Calculate GPA
  - Generate transcript
  - Grade reports

**Estimated Time:** 4-5 hours

#### 2. Frontend UI
- [ ] Grade entry page (teacher view)
  - Spreadsheet-like interface
  - Bulk entry
  - Auto-calculation
- [ ] Grade book view
- [ ] Student transcript page
- [ ] Grade reports
  - Class average
  - Grade distribution
  - Top performers
- [ ] Parent view (read-only grades)

**Estimated Time:** 5-6 hours

---

## Phase 5: Timetable Management ⏳ 0% (Target: Jan 2-5)

### 🎯 Goals
Complete schedule management for students, teachers, and rooms

### 📋 Tasks

#### 1. Database & Backend
- [ ] `timetable_slots` table
  - Day of week
  - Period number
  - Time range
- [ ] `class_schedules` table
  - Class + Subject + Teacher + Room + Slot
- [ ] Conflict detection logic
  - Teacher double-booking
  - Room conflicts
  - Student schedule conflicts
- [ ] Timetable APIs
  - Create/Edit schedules
  - Get timetable by class/teacher/room
  - Check conflicts

**Estimated Time:** 5-6 hours

#### 2. Frontend UI
- [ ] Timetable builder (admin view)
  - Drag & drop interface
  - Visual conflict warnings
  - Auto-suggest available slots
- [ ] Timetable view (responsive calendar)
  - Teacher view
  - Student view
  - Class view
  - Room view
- [ ] Print-friendly timetable
- [ ] Export to PDF

**Estimated Time:** 6-8 hours

---

## Phase 6: Finance Management ⏳ 0% (Target: Jan 6-10)

### 🎯 Goals
Handle tuition, fees, and payments

### 📋 Tasks

#### 1. Database & Backend
- [ ] `fee_structures` table
- [ ] `invoices` table
- [ ] `payments` table
- [ ] `payment_methods` table
- [ ] Finance APIs
  - Generate invoices
  - Record payments
  - Payment reports
  - Outstanding balances

**Estimated Time:** 4-5 hours

#### 2. Frontend UI
- [ ] Fee structure configuration
- [ ] Generate invoice page
- [ ] Record payment page
- [ ] Payment history
- [ ] Financial reports
  - Revenue reports
  - Outstanding payments
  - Payment analytics
- [ ] Parent payment portal

**Estimated Time:** 5-6 hours

---

## Phase 7: Document Management ⏳ 0% (Target: Jan 11-15)

### 🎯 Goals
Document workflow and approval system

### 📋 Tasks

#### 1. Database & Backend
- [ ] `documents` table
- [ ] `document_workflows` table
- [ ] `document_approvals` table
- [ ] Document APIs
  - Upload document
  - Submit for approval
  - Approve/Reject
  - Track status
- [ ] Workflow engine
  - Route based on role/department levels
  - Multi-step approval chains
  - Notifications

**Estimated Time:** 6-8 hours

#### 2. Frontend UI
- [ ] Document upload page
- [ ] Document list/browser
- [ ] Approval queue (my tasks)
- [ ] Document viewer
- [ ] Approval actions (approve/reject/comment)
- [ ] Document history/audit trail
- [ ] Template management

**Estimated Time:** 6-8 hours

---

## Phase 8: Advanced Features ⏳ 0% (Target: Jan 16-20)

### 📋 Tasks
- [ ] Analytics Dashboard
  - School KPIs
  - Student performance trends
  - Attendance trends
  - Financial overview
- [ ] Notification System
  - In-app notifications
  - Email notifications
  - SMS integration (optional)
- [ ] Reports Module
  - Custom report builder
  - Scheduled reports
  - Export formats (PDF, Excel, CSV)
- [ ] Settings & Configuration
  - School settings
  - Academic year management
  - System preferences
- [ ] Mobile App (optional)
  - React Native
  - Teacher app
  - Parent app

**Estimated Time:** 10-15 hours

---

## Phase 9: Testing & Quality Assurance ⏳ 0% (Target: Jan 21-25)

### 📋 Tasks
- [ ] Unit Tests (Backend)
  - Test all handlers
  - Test business logic
  - Test database operations
- [ ] Integration Tests
  - API endpoint tests
  - Authentication flows
  - Multi-tenant isolation
- [ ] E2E Tests (Frontend)
  - Playwright/Cypress setup
  - Critical user flows
  - Form submissions
- [ ] Performance Testing
  - Load testing (Artillery/k6)
  - Database query optimization
  - Frontend performance audit
- [ ] Security Audit
  - SQL injection prevention
  - XSS prevention
  - CSRF protection
  - Permission bypasses

**Estimated Time:** 15-20 hours

---

## Phase 10: Deployment & Production ⏳ 0% (Target: Jan 26-31)

### 📋 Tasks
- [ ] Production Database Setup
  - Neon PostgreSQL (production tier)
  - Backup strategy
  - Migration rollback plan
- [ ] Deploy Backend Services
  - `backend-admin` to Fly.io/Railway
  - `backend-school` to Fly.io/Railway
  - Environment variables
  - SSL certificates
  - Custom domain
- [ ] Deploy Frontend
  - `frontend-admin` to Cloudflare Pages
  - `frontend-school` to Cloudflare Workers
  - Subdomain routing
  - CDN configuration
- [ ] Monitoring & Logging
  - Error tracking (Sentry)
  - Performance monitoring (DataDog/NewRelic)
  - Log aggregation
  - Uptime monitoring
- [ ] Documentation
  - User manual
  - Admin guide
  - API documentation (Swagger)
  - Deployment guide
- [ ] Launch Preparation
  - User training
  - Data migration (if needed)
  - Support system
  - Feedback collection

**Estimated Time:** 10-15 hours

---

# 📊 Summary Statistics

## Code Written (So Far)

```
Backend (Rust):
├── Migrations:        ~1,500 lines SQL
├── Handler Code:      ~3,000 lines (.rs)
├── Models:            ~1,000 lines (.rs)
├── Middleware:        ~500 lines (.rs)
├── Utilities:         ~300 lines (.rs)
└── Total Backend:     ~6,300 lines

Frontend (SvelteKit):
├── Components:        ~2,000 lines (.svelte)
├── Pages:             ~3,500 lines (.svelte)
├── API Clients:       ~800 lines (.ts)
├── Utilities:         ~200 lines (.ts)
└── Total Frontend:    ~6,500 lines

Documentation:
└── Markdown:          ~2,000 lines (.md)

GRAND TOTAL:           ~14,800 lines
```

## Features Completed vs Remaining

```
✅ Completed:     7 features (35%)
🔄 In Progress:   1 feature  (5%)
⏳ Remaining:     12 features (60%)
```

## Time Invested vs Estimated Remaining

```
Time Spent:      ~40 hours
Time Remaining:  ~120 hours (estimated)
Total Project:   ~160 hours (2 months part-time)
```

---

# 🎯 Recommended Next Steps

## Immediate (This Week)

1. **Test Current System** (2 hours)
   - Run migrations
   - Test all Staff APIs
   - Test Frontend UI
   - Fix any bugs found

2. **Complete Staff Management** (6-8 hours)
   - Create/Edit Staff Form
   - Permission Middleware
   - Role/Department UI

3. **Start Student Management** (8 hours)
   - Database & APIs
   - Basic UI (List + Profile)

## Short Term (Next 2 Weeks)

4. **Student Management** (Complete remaining)
5. **Attendance System** (Full implementation)
6. **Grading System** (Core features)

## Medium Term (End of January)

7. **Timetable Management**
8. **Finance Management**
9. **Document Management**
10. **Testing & Deployment**

---

# 🚧 Known Issues & Technical Debt

## Current Issues
1. ⚠️ Staff Create/Edit form missing (80% → 100%)
2. ⚠️ Permission checking not enforced yet
3. ⚠️ No file upload (profile pictures)
4. ⚠️ No audit logging
5. ⚠️ Limited error messages (UX)
6. ⚠️ No bulk operations

## Technical Debt
1. 📝 Need more comprehensive tests
2. 📝 API documentation (Swagger/OpenAPI)
3. 📝 Better error handling in frontend
4. 📝 Toast notifications system
5. 📝 Loading states improvement
6. 📝 Offline support (PWA)

---

# 📞 Contact & Resources

## Project Links
- **GitHub Repository:** (To be set)
- **Production URL:** https://schoolorbit.app
- **Admin Panel:** https://admin.schoolorbit.app
- **API Docs:** (To be published)

## Tech Stack
- **Backend:** Rust, Axum, SQLx, PostgreSQL
- **Frontend:** SvelteKit, TypeScript, Tailwind CSS
- **Deployment:** Cloudflare Workers, Fly.io, GitHub Actions
- **Database:** Neon PostgreSQL

---

**Last Updated:** 20 ธันวาคม 2567 23:32  
**Project Start:** 1 ธันวาคม 2567  
**Estimated Completion:** 31 มกราคม 2568 (2 months)
