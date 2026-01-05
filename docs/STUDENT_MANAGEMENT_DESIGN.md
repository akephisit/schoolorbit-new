# Student Management System - Simplified Design

## 📋 สรุปสั้นๆ

ระบบจัดการนักเรียนที่ใช้ infrastructure ที่มีอยู่แล้วทั้งหมด:

### ✅ สิ่งที่มีอยู่แล้ว (ไม่ต้องสร้างใหม่)
- ✅ Database: `users` + `student_info` tables
- ✅ Permission System: Registry auto-sync จาก code
- ✅ Menu System: จัดการที่ Frontend
- ✅ Authentication: JWT + bcrypt

### 🆕 สิ่งที่ต้องเพิ่ม
1. **Permission definitions** - แก้ไขใน `registry.rs`
2. **STUDENT role** - เพิ่มใน migration
3. **Backend handlers** - API สำหรับนักเรียน
4. **Frontend pages** - Student portal + Admin management

---

## 1. เพิ่ม Permissions (Backend)

### แก้ไข: `backend-school/src/permissions/registry.rs`

```rust
pub mod codes {
    // ... existing codes ...
    
    // Student permissions (เพิ่มใหม่)
    pub const DASHBOARD: &str = "dashboard";
    pub const STUDENT_READ_OWN: &str = "student.read.own";
    pub const STUDENT_UPDATE_OWN: &str = "student.update.own";
    pub const STUDENT_READ_ALL: &str = "student.read.all";
    pub const STUDENT_CREATE: &str = "student.create";
    pub const STUDENT_UPDATE_ALL: &str = "student.update.all";
    pub const STUDENT_DELETE: &str = "student.delete";
}

pub const ALL_PERMISSIONS: &[PermissionDef] = &[
    // ... existing permissions ...
    
    // Dashboard
    PermissionDef {
        code: codes::DASHBOARD,
        name: "แดชบอร์ด",
        module: "dashboard",
        action: "read",
        scope: "own",
        description: "ดูหน้าแดชบอร์ด",
    },
    
    // Student permissions
    PermissionDef {
        code: codes::STUDENT_READ_OWN,
        name: "ดูข้อมูลตนเอง",
        module: "student",
        action: "read",
        scope: "own",
        description: "นักเรียนดูข้อมูลตนเอง",
    },
    PermissionDef {
        code: codes::STUDENT_UPDATE_OWN,
        name: "แก้ไขข้อมูลตนเอง",
        module: "student",
        action: "update",
        scope: "own",
        description: "นักเรียนแก้ไขข้อมูลตนเอง (จำกัดฟิลด์)",
    },
    PermissionDef {
        code: codes::STUDENT_READ_ALL,
        name: "ดูนักเรียนทั้งหมด",
        module: "student",
        action: "read",
        scope: "all",
        description: "ดูข้อมูลนักเรียนทั้งหมด (Admin/Staff)",
    },
    PermissionDef {
        code: codes::STUDENT_CREATE,
        name: "เพิ่มนักเรียน",
        module: "student",
        action: "create",
        scope: "all",
        description: "สร้างนักเรียนใหม่",
    },
    PermissionDef {
        code: codes::STUDENT_UPDATE_ALL,
        name: "แก้ไขนักเรียน",
        module: "student",
        action: "update",
        scope: "all",
        description: "แก้ไขข้อมูลนักเรียนทั้งหมด",
    },
    PermissionDef {
        code: codes::STUDENT_DELETE,
        name: "ลบนักเรียน",
        module: "student",
        action: "delete",
        scope: "all",
        description: "ลบนักเรียน",
    },
];
```

**📝 หมายเหตุ:** เมื่อ deploy ระบบจะ auto-sync permissions เหล่านี้ลง database

---

## 2. สร้าง STUDENT Role (Migration)

### สร้าง: `backend-school/migrations/013_student_management.sql`

```sql
-- ===================================================================
-- Migration 013: Student Management System
-- Description: เพิ่ม STUDENT role และ permissions
-- ===================================================================

-- เพิ่ม STUDENT role
INSERT INTO roles (code, name, name_en, category, level, permissions) VALUES
(
    'STUDENT',
    'นักเรียน',
    'Student',
    'student',
    1,
    ARRAY[
        'dashboard',
        'student.read.own',
        'student.update.own'
    ]
)
ON CONFLICT (code) DO UPDATE SET
    permissions = EXCLUDED.permissions,
    name = EXCLUDED.name,
    name_en = EXCLUDED.name_en,
    category = EXCLUDED.category;

-- เพิ่ม STUDENT_MANAGER role (สำหรับครู/Admin ที่จัดการนักเรียน)
INSERT INTO roles (code, name, name_en, category, level, permissions) VALUES
(
    'STUDENT_MANAGER',
    'ผู้จัดการนักเรียน',
    'Student Manager',
    'administrative',
    50,
    ARRAY[
        'dashboard',
        'student.read.all',
        'student.create',
        'student.update.all'
    ]
)
ON CONFLICT (code) DO UPDATE SET
    permissions = EXCLUDED.permissions,
    name = EXCLUDED.name,
    name_en = EXCLUDED.name_en;

-- เพิ่ม comment
COMMENT ON TABLE student_info IS 'ข้อมูลเฉพาะนักเรียน - ใช้ร่วมกับ users table';
```

---

## 3. Student Login (รองรับเลขบัตรประชาชน)

### Authentication Flow

นักเรียน Login ด้วย:
- **เลขบัตรประชาชน** 13 หลัก (ไม่ต้องมีขีด)
- **รหัสผ่าน**

### Backend: อัพเดต `src/handlers/auth.rs`

```rust
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub national_id: String,  // เฉพาะเลขบัตรประชาชน
    pub password: String,
}

pub async fn login(
    pool: web::Data<PgPool>,
    body: web::Json<LoginRequest>,
) -> Result<HttpResponse, ApiError> {
    // Find user by national_id only
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users 
         WHERE national_id = $1
         AND status = 'active'"
    )
    .bind(&body.national_id)
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or(ApiError::Unauthorized("Invalid credentials".to_string()))?;
    
    // Verify password
    let is_valid = bcrypt::verify(&body.password, &user.password_hash)?;
    if !is_valid {
        return Err(ApiError::Unauthorized("Invalid credentials".to_string()));
    }
    
    // Get user permissions
    let permissions = get_user_permissions(pool.get_ref(), &user.id).await?;
    
    // Generate JWT
    let token = generate_jwt_token(&user.id, &permissions)?;
    
    Ok(HttpResponse::Ok().json(LoginResponse {
        token,
        user,
        permissions,
    }))
}
```

### Frontend: `src/routes/login/+page.svelte`

```svelte
<script lang="ts">
    import { goto } from '$app/navigation';
    import { Button } from '$lib/components/ui/button';
    import { Input } from '$lib/components/ui/input';
    import { Label } from '$lib/components/ui/label';
    
    let nationalId = '';
    let password = '';
    let error = '';
    let loading = false;
    
    async function handleLogin() {
        loading = true;
        error = '';
        
        try {
            const response = await fetch('/api/auth/login', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ 
                    national_id: nationalId, 
                    password 
                })
            });
            
            if (!response.ok) {
                const data = await response.json();
                error = data.message || 'ข้อมูลไม่ถูกต้อง';
                return;
            }
            
            const data = await response.json();
            
            // Store auth data
            localStorage.setItem('auth_token', data.token);
            localStorage.setItem('user', JSON.stringify(data.user));
            
            // Redirect based on user type
            if (data.user.user_type === 'student') {
                goto('/student/dashboard');
            } else {
                goto('/dashboard');
            }
        } catch (err) {
            error = 'เกิดข้อผิดพลาด กรุณาลองใหม่อีกครั้ง';
        } finally {
            loading = false;
        }
    }
</script>

<div class="min-h-screen flex items-center justify-center bg-gradient-to-br from-blue-50 to-indigo-100">
    <div class="bg-white p-8 rounded-lg shadow-lg w-full max-w-md">
        <h1 class="text-2xl font-bold text-center mb-6">เข้าสู่ระบบ</h1>
        
        <form on:submit|preventDefault={handleLogin} class="space-y-4">
            <div>
                <Label for="national-id">เลขบัตรประชาชน</Label>
                <Input 
                    id="national-id"
                    type="text"
                    maxlength="13"
                    bind:value={nationalId}
                    placeholder="1234567890123"
                    disabled={loading}
                    required
                />
            </div>
            
            <div>
                <Label for="password">รหัสผ่าน</Label>
                <Input 
                    id="password"
                    type="password"
                    bind:value={password}
                    disabled={loading}
                    required
                />
            </div>
            
            {#if error}
                <p class="text-sm text-red-600">{error}</p>
            {/if}
            
            <Button type="submit" class="w-full" disabled={loading}>
                {loading ? 'กำลังเข้าสู่ระบบ...' : 'เข้าสู่ระบบ'}
            </Button>
        </form>
    </div>
</div>
```

---

## 4. Backend APIs

### สร้าง: `backend-school/src/handlers/students.rs`

```rust
use actix_web::{web, HttpRequest, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;
use crate::middleware::auth::extract_user_id;
use crate::permissions::registry::codes;

// =========================================
// Student Self-Service APIs
// =========================================

/// GET /api/student/profile - นักเรียนดูข้อมูลตนเอง
#[require_permission(codes::STUDENT_READ_OWN)]
pub async fn get_own_profile(
    pool: web::Data<PgPool>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let user_id = extract_user_id(&req)?;
    
    let student = sqlx::query!(
        r#"
        SELECT 
            u.id, u.national_id, u.email, u.first_name, u.last_name,
            u.title, u.nickname, u.phone, u.date_of_birth, u.gender,
            u.address, u.profile_image_url,
            s.student_id, s.grade_level, s.class_room, s.student_number,
            s.blood_type, s.allergies, s.medical_conditions
        FROM users u
        LEFT JOIN student_info s ON u.id = s.user_id
        WHERE u.id = $1 AND u.user_type = 'student'
        "#,
        user_id
    )
    .fetch_one(pool.get_ref())
    .await?;
    
    Ok(HttpResponse::Ok().json(student))
}

/// PUT /api/student/profile - นักเรียนแก้ไขข้อมูลตนเอง (จำกัดฟิลด์)
#[require_permission(codes::STUDENT_UPDATE_OWN)]
pub async fn update_own_profile(
    pool: web::Data<PgPool>,
    req: HttpRequest,
    body: web::Json<UpdateOwnProfileRequest>,
) -> Result<HttpResponse, ApiError> {
    let user_id = extract_user_id(&req)?;
    
    // อัพเดตเฉพาะฟิลด์ที่นักเรียนสามารถแก้ไขได้
    sqlx::query!(
        r#"
        UPDATE users
        SET 
            phone = COALESCE($2, phone),
            address = COALESCE($3, address),
            nickname = COALESCE($4, nickname),
            updated_at = NOW()
        WHERE id = $1
        "#,
        user_id,
        body.phone.as_ref(),
        body.address.as_ref(),
        body.nickname.as_ref()
    )
    .execute(pool.get_ref())
    .await?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "อัพเดตข้อมูลสำเร็จ"
    })))
}

// =========================================
// Admin/Staff Student Management APIs
// =========================================

/// GET /api/students - รายชื่อนักเรียนทั้งหมด
#[require_permission(codes::STUDENT_READ_ALL)]
pub async fn list_students(
    pool: web::Data<PgPool>,
    query: web::Query<ListStudentsQuery>,
) -> Result<HttpResponse, ApiError> {
    let students = sqlx::query!(
        r#"
        SELECT 
            u.id, u.first_name, u.last_name,
            s.student_id, s.grade_level, s.class_room,
            u.status
        FROM users u
        INNER JOIN student_info s ON u.id = s.user_id
        WHERE u.user_type = 'student'
        ORDER BY s.grade_level, s.class_room, s.student_number
        LIMIT $1 OFFSET $2
        "#,
        query.limit.unwrap_or(50) as i64,
        query.offset.unwrap_or(0) as i64
    )
    .fetch_all(pool.get_ref())
    .await?;
    
    Ok(HttpResponse::Ok().json(students))
}

/// POST /api/students - เพิ่มนักเรียนใหม่
#[require_permission(codes::STUDENT_CREATE)]
pub async fn create_student(
    pool: web::Data<PgPool>,
    body: web::Json<CreateStudentRequest>,
) -> Result<HttpResponse, ApiError> {
    let mut tx = pool.begin().await?;
    
    // 1. Hash password
    let password_hash = bcrypt::hash(&body.password, 12)?;
    
    // 2. สร้าง user
    let user_id = sqlx::query_scalar!(
        r#"
        INSERT INTO users (
            national_id, email, password_hash,
            first_name, last_name, title, user_type, status
        ) VALUES ($1, $2, $3, $4, $5, $6, 'student', 'active')
        RETURNING id
        "#,
        body.national_id,
        body.email.as_ref(),
        password_hash,
        body.first_name,
        body.last_name,
        body.title.as_ref()
    )
    .fetch_one(&mut *tx)
    .await?;
    
    // 3. สร้าง student_info
    sqlx::query!(
        r#"
        INSERT INTO student_info (
            user_id, student_id, grade_level, class_room, student_number
        ) VALUES ($1, $2, $3, $4, $5)
        "#,
        user_id,
        body.student_id,
        body.grade_level.as_ref(),
        body.class_room.as_ref(),
        body.student_number
    )
    .execute(&mut *tx)
    .await?;
    
    // 4. Assign STUDENT role
    let student_role_id = sqlx::query_scalar!(
        "SELECT id FROM roles WHERE code = 'STUDENT'"
    )
    .fetch_one(&mut *tx)
    .await?;
    
    sqlx::query!(
        r#"
        INSERT INTO user_roles (user_id, role_id, is_primary)
        VALUES ($1, $2, true)
        "#,
        user_id,
        student_role_id
    )
    .execute(&mut *tx)
    .await?;
    
    tx.commit().await?;
    
    Ok(HttpResponse::Created().json(serde_json::json!({
        "success": true,
        "id": user_id
    })))
}

/// PUT /api/students/:id - แก้ไขข้อมูลนักเรียน
#[require_permission(codes::STUDENT_UPDATE_ALL)]
pub async fn update_student(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateStudentRequest>,
) -> Result<HttpResponse, ApiError> {
    let student_id = path.into_inner();
    
    let mut tx = pool.begin().await?;
    
    // Update users table
    sqlx::query!(
        r#"
        UPDATE users
        SET 
            email = COALESCE($2, email),
            first_name = COALESCE($3, first_name),
            last_name = COALESCE($4, last_name),
            phone = COALESCE($5, phone),
            address = COALESCE($6, address),
            updated_at = NOW()
        WHERE id = $1
        "#,
        student_id,
        body.email.as_ref(),
        body.first_name.as_ref(),
        body.last_name.as_ref(),
        body.phone.as_ref(),
        body.address.as_ref()
    )
    .execute(&mut *tx)
    .await?;
    
    // Update student_info table
    sqlx::query!(
        r#"
        UPDATE student_info
        SET 
            grade_level = COALESCE($2, grade_level),
            class_room = COALESCE($3, class_room),
            updated_at = NOW()
        WHERE user_id = $1
        "#,
        student_id,
        body.grade_level.as_ref(),
        body.class_room.as_ref()
    )
    .execute(&mut *tx)
    .await?;
    
    tx.commit().await?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true
    })))
}

/// DELETE /api/students/:id - ลบนักเรียน (soft delete)
#[require_permission(codes::STUDENT_DELETE)]
pub async fn delete_student(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let student_id = path.into_inner();
    
    sqlx::query!(
        "UPDATE users SET status = 'inactive', updated_at = NOW() WHERE id = $1",
        student_id
    )
    .execute(pool.get_ref())
    .await?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true
    })))
}

// =========================================
// Request/Response structs
// =========================================

#[derive(Deserialize)]
pub struct UpdateOwnProfileRequest {
    pub phone: Option<String>,
    pub address: Option<String>,
    pub nickname: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateStudentRequest {
    pub national_id: String,
    pub email: Option<String>,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub title: Option<String>,
    pub student_id: String,
    pub grade_level: Option<String>,
    pub class_room: Option<String>,
    pub student_number: Option<i32>,
}

#[derive(Deserialize)]
pub struct UpdateStudentRequest {
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub grade_level: Option<String>,
    pub class_room: Option<String>,
}

#[derive(Deserialize)]
pub struct ListStudentsQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}
```

### อัพเดต: `backend-school/src/main.rs`

```rust
mod handlers {
    // ... existing handlers ...
    pub mod students;  // เพิ่มบรรทัดนี้
}

fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg
        // ... existing routes ...
        
        // Student self-service
        .route("/api/student/profile", web::get().to(handlers::students::get_own_profile))
        .route("/api/student/profile", web::put().to(handlers::students::update_own_profile))
        
        // Admin student management
        .route("/api/students", web::get().to(handlers::students::list_students))
        .route("/api/students", web::post().to(handlers::students::create_student))
        .route("/api/students/{id}", web::put().to(handlers::students::update_student))
        .route("/api/students/{id}", web::delete().to(handlers::students::delete_student));
}
```

---

## 5. Frontend Pages

### Student Portal

#### `src/routes/student/+layout.svelte` - Layout สำหรับนักเรียน

```svelte
<script lang="ts">
    import { page } from '$app/stores';
    import { goto } from '$app/navigation';
    import { onMount } from 'svelte';
    
    let user = $state(null);
    
    onMount(() => {
        const userData = localStorage.getItem('user');
        if (!userData) {
            goto('/login');
            return;
        }
        
        user = JSON.parse(userData);
        
        // Check if user is student
        if (user.user_type !== 'student') {
            goto('/dashboard');
        }
    });
</script>

<div class="min-h-screen flex">
    <!-- Sidebar -->
    <aside class="w-64 bg-white shadow-lg">
        <div class="p-6">
            <h2 class="text-xl font-bold">Student Portal</h2>
            <p class="text-sm text-gray-600">{user?.first_name} {user?.last_name}</p>
        </div>
        
        <nav class="mt-6">
            <a href="/student/dashboard" class="block px-6 py-3 hover:bg-blue-50">
                📊 แดชบอร์ด
            </a>
            <a href="/student/profile" class="block px-6 py-3 hover:bg-blue-50">
                👤 ข้อมูลส่วนตัว
            </a>
        </nav>
    </aside>
    
    <!-- Main Content -->
    <main class="flex-1 p-8 bg-gray-50">
        <slot />
    </main>
</div>
```

#### `src/routes/student/dashboard/+page.svelte` - Dashboard นักเรียน

```svelte
<script lang="ts">
    let user = $state(null);
    
    onMount(() => {
        const userData = localStorage.getItem('user');
        user = userData ? JSON.parse(userData) : null;
    });
</script>

<div>
    <h1 class="text-3xl font-bold mb-6">แดชบอร์ด</h1>
    
    <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
        <div class="bg-white p-6 rounded-lg shadow">
            <h3 class="text-lg font-semibold mb-2">ข้อมูลนักเรียน</h3>
            <p>รหัส: {user?.student_id || '-'}</p>
            <p>ชั้น: {user?.grade_level || '-'}/{user?.class_room || '-'}</p>
        </div>
        
        <div class="bg-white p-6 rounded-lg shadow">
            <h3 class="text-lg font-semibold mb-2">การเข้าเรียน</h3>
            <p class="text-2xl font-bold text-green-600">95%</p>
        </div>
        
        <div class="bg-white p-6 rounded-lg shadow">
            <h3 class="text-lg font-semibold mb-2">คะแนนเฉลี่ย</h3>
            <p class="text-2xl font-bold text-blue-600">3.45</p>
        </div>
    </div>
</div>
```

#### `src/routes/student/profile/+page.svelte` - Profile นักเรียน

```svelte
<script lang="ts">
    import { onMount } from 'svelte';
    import { Button } from '$lib/components/ui/button';
    import { Input } from '$lib/components/ui/input';
    import { Label } from '$lib/components/ui/label';
    
    let student = $state(null);
    let editing = $state(false);
    let loading = $state(false);
    
    let phone = $state('');
    let address = $state('');
    let nickname = $state('');
    
    onMount(async () => {
        await loadProfile();
    });
    
    async function loadProfile() {
        const response = await fetch('/api/student/profile', {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('auth_token')}`
            }
        });
        
        if (response.ok) {
            student = await response.json();
            phone = student.phone || '';
            address = student.address || '';
            nickname = student.nickname || '';
        }
    }
    
    async function handleSave() {
        loading = true;
        
        try {
            const response = await fetch('/api/student/profile', {
                method: 'PUT',
                headers: {
                    'Content-Type': 'application/json',
                    'Authorization': `Bearer ${localStorage.getItem('auth_token')}`
                },
                body: JSON.stringify({ phone, address, nickname })
            });
            
            if (response.ok) {
                await loadProfile();
                editing = false;
            }
        } finally {
            loading = false;
        }
    }
</script>

<div class="max-w-3xl">
    <h1 class="text-3xl font-bold mb-6">ข้อมูลส่วนตัว</h1>
    
    {#if student}
        <div class="bg-white p-6 rounded-lg shadow space-y-6">
            <!-- ข้อมูลพื้นฐาน (ไม่สามารถแก้ไขได้) -->
            <div>
                <h2 class="text-xl font-semibold mb-4">ข้อมูลพื้นฐาน</h2>
                <div class="grid grid-cols-2 gap-4">
                    <div>
                        <Label>ชื่อ-นามสกุล</Label>
                        <p class="mt-1">{student.first_name} {student.last_name}</p>
                    </div>
                    <div>
                        <Label>รหัสนักเรียน</Label>
                        <p class="mt-1">{student.student_id}</p>
                    </div>
                    <div>
                        <Label>ระดับชั้น</Label>
                        <p class="mt-1">{student.grade_level}/{student.class_room}</p>
                    </div>
                </div>
            </div>
            
            <!-- ข้อมูลที่แก้ไขได้ -->
            <div>
                <h2 class="text-xl font-semibold mb-4">ข้อมูลติดต่อ</h2>
                
                {#if editing}
                    <div class="space-y-4">
                        <div>
                            <Label for="nickname">ชื่อเล่น</Label>
                            <Input id="nickname" bind:value={nickname} />
                        </div>
                        
                        <div>
                            <Label for="phone">เบอร์โทรศัพท์</Label>
                            <Input id="phone" bind:value={phone} />
                        </div>
                        
                        <div>
                            <Label for="address">ที่อยู่</Label>
                            <textarea 
                                id="address"
                                bind:value={address}
                                class="w-full border rounded p-2"
                                rows="3"
                            ></textarea>
                        </div>
                        
                        <div class="flex gap-2">
                            <Button on:click={handleSave} disabled={loading}>
                                {loading ? 'กำลังบันทึก...' : 'บันทึก'}
                            </Button>
                            <Button variant="outline" on:click={() => editing = false}>
                                ยกเลิก
                            </Button>
                        </div>
                    </div>
                {:else}
                    <div class="grid grid-cols-2 gap-4">
                        <div>
                            <Label>ชื่อเล่น</Label>
                            <p class="mt-1">{student.nickname || '-'}</p>
                        </div>
                        <div>
                            <Label>เบอร์โทร</Label>
                            <p class="mt-1">{student.phone || '-'}</p>
                        </div>
                        <div class="col-span-2">
                            <Label>ที่อยู่</Label>
                            <p class="mt-1">{student.address || '-'}</p>
                        </div>
                    </div>
                    
                    <Button class="mt-4" on:click={() => editing = true}>
                        แก้ไขข้อมูล
                    </Button>
                {/if}
            </div>
        </div>
    {/if}
</div>
```

### Admin Student Management

#### `src/routes/admin/students/+page.svelte` - รายชื่อนักเรียน

```svelte
<script lang="ts">
    import { onMount } from 'svelte';
    import { Button } from '$lib/components/ui/button';
    
    let students = $state([]);
    
    onMount(async () => {
        const response = await fetch('/api/students', {
            headers: {
                'Authorization': `Bearer ${localStorage.getItem('auth_token')}`
            }
        });
        
        if (response.ok) {
            students = await response.json();
        }
    });
</script>

<div>
    <div class="flex justify-between items-center mb-6">
        <h1 class="text-3xl font-bold">จัดการนักเรียน</h1>
        <Button href="/admin/students/new">+ เพิ่มนักเรียน</Button>
    </div>
    
    <div class="bg-white rounded-lg shadow overflow-hidden">
        <table class="w-full">
            <thead class="bg-gray-50">
                <tr>
                    <th class="px-6 py-3 text-left">รหัสนักเรียน</th>
                    <th class="px-6 py-3 text-left">ชื่อ-นามสกุล</th>
                    <th class="px-6 py-3 text-left">ชั้น</th>
                    <th class="px-6 py-3 text-left">สถานะ</th>
                    <th class="px-6 py-3 text-left">จัดการ</th>
                </tr>
            </thead>
            <tbody>
                {#each students as student}
                    <tr class="border-t">
                        <td class="px-6 py-4">{student.student_id}</td>
                        <td class="px-6 py-4">{student.first_name} {student.last_name}</td>
                        <td class="px-6 py-4">{student.grade_level}/{student.class_room}</td>
                        <td class="px-6 py-4">
                            <span class="px-2 py-1 text-xs rounded {student.status === 'active' ? 'bg-green-100 text-green-800' : 'bg-gray-100 text-gray-800'}">
                                {student.status}
                            </span>
                        </td>
                        <td class="px-6 py-4">
                            <Button size="sm" variant="outline" href="/admin/students/{student.id}/edit">
                                แก้ไข
                            </Button>
                        </td>
                    </tr>
                {/each}
            </tbody>
        </table>
    </div>
</div>
```

---

## สรุปการออกแบบ

### ✅ สิ่งที่ไม่ต้องทำ
- ❌ ไม่ต้องสร้าง tables ใหม่ (`student_forms`, `student_form_submissions`)
- ❌ ไม่ต้อง INSERT menu items ใน migration (จัดการที่ Frontend)
- ❌ ไม่ต้อง INSERT permissions ใน migration (auto-sync จาก registry)

### ✅ สิ่งที่ต้องทำ
1. **เพิ่ม permissions** ใน `registry.rs` (5-10 นาที)
2. **สร้าง migration** สำหรับ STUDENT role (5 นาที)
3. **สร้าง Student handlers** (1-2 ชั่วโมง)
4. **สร้าง Frontend pages** (2-3 ชั่วโมง)

### Total Time: ~3-5 ชั่วโมง

---

## ขั้นตอนการ Implement

1. **แก้ไข `registry.rs`** - เพิ่ม student permissions
2. **สร้าง migration 013** - เพิ่ม STUDENT role
3. **สร้าง `handlers/students.rs`** - Backend APIs
4. **สร้าง frontend pages** - Student portal + Admin management
5. **Test** - ทดสอบ login และการทำงาน

พร้อมให้ผมช่วย implement ไหมครับ? 🚀
