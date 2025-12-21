# Permission Requirements Documentation

**Last Updated:** 2025-12-22  
**Version:** 1.0

---

## 📋 Overview

This document describes the permission requirements for all API endpoints in the SchoolOrbit system. The permission system uses a hybrid model combining `user_type`, `role`, and `permissions`.

---

## 🔐 Permission Model

### Architecture
```
user_type (Data Layer)
    ↓
role (Functional Layer)
    ↓
permissions (Access Control Layer)
```

### Special Rules
1. **Admin User Type:** Users with `user_type = "admin"` bypass all permission checks
2. **Wildcard Permission:** Role with permission `"*"` grants all permissions
3. **Multiple Roles:** Users can have multiple roles; permissions accumulate
4. **Active Roles Only:** Only roles with `ended_at = NULL` are considered

---

## 🎯 API Endpoints & Required Permissions

### Staff Management

| Endpoint | Method | Permission | Description |
|----------|--------|------------|-------------|
| `/api/staff` | GET | `users.view` | List all staff members |
| `/api/staff/:id` | GET | `users.view` | Get staff profile |
| `/api/staff` | POST | `users.create` | Create new staff |
| `/api/staff/:id` | PUT | `users.edit` | Update staff information |
| `/api/staff/:id` | DELETE | `users.delete` | Remove staff (soft delete) |

**Example:**
```rust
// In handler
if !user.has_permission(&pool, "users.create").await? {
    return Err(StatusCode::FORBIDDEN);
}
```

---

### Role Management

| Endpoint | Method | Permission | Description |
|----------|--------|------------|-------------|
| `/api/roles` | GET | `roles.view` | List all roles |
| `/api/roles/:id` | GET | `roles.view` | Get role details |
| `/api/roles` | POST | `roles.manage` | Create new role |
| `/api/roles/:id` | PUT | `roles.manage` | Update role |
| `/api/roles/:id` | DELETE | `roles.manage` | Delete role |

---

### User Role Assignment

| Endpoint | Method | Permission | Description |
|----------|--------|------------|-------------|
| `/api/users/:id/roles` | GET | `users.view` | Get user's roles |
| `/api/users/:id/roles` | POST | `users.edit` | Assign role to user |
| `/api/users/:id/roles/:role_id` | DELETE | `users.edit` | Remove role from user |
| `/api/users/:id/permissions` | GET | `users.view` | Get user's permissions |

---

### Permissions Master Data

| Endpoint | Method | Permission | Description |
|----------|--------|------------|-------------|
| `/api/permissions` | GET | `users.view` | List all permissions |
| `/api/permissions/modules` | GET | `users.view` | Permissions grouped by module |

---

### Department Management

| Endpoint | Method | Permission | Description |
|----------|--------|------------|-------------|
| `/api/departments` | GET | `departments.view` | List departments |
| `/api/departments/:id` | GET | `departments.view` | Get department details |
| `/api/departments` | POST | `departments.manage` | Create department |
| `/api/departments/:id` | PUT | `departments.manage` | Update department |

---

## 📊 Permission Matrix by Role

| Permission | TEACHER | DEPT_HEAD | DIRECTOR | ADMIN |
|------------|---------|-----------|----------|-------|
| **Users** |
| `users.view` | ✅ | ✅ | ✅ | ✅ |
| `users.create` | ❌ | ❌ | ✅ | ✅ |
| `users.edit` | ❌ | ✅ (limited) | ✅ | ✅ |
| `users.delete` | ❌ | ❌ | ✅ | ✅ |
| **Students** |
| `students.view` | ✅ | ✅ | ✅ | ✅ |
| `students.create` | ❌ | ❌ | ✅ | ✅ |
| `students.edit` | ❌ | ✅ | ✅ | ✅ |
| **Grades** |
| `grades.view` | ✅ | ✅ | ✅ | ✅ |
| `grades.edit` | ✅ | ✅ | ✅ | ✅ |
| **Attendance** |
| `attendance.view` | ✅ | ✅ | ✅ | ✅ |
| `attendance.mark` | ✅ | ✅ | ✅ | ✅ |
| **Documents** |
| `documents.view` | ✅ | ✅ | ✅ | ✅ |
| `documents.create` | ✅ | ✅ | ✅ | ✅ |
| `documents.approve_dept` | ❌ | ✅ | ✅ | ✅ |
| `documents.approve` | ❌ | ❌ | ✅ | ✅ |
| **Finance** |
| `finance.view` | ❌ | ✅ | ✅ | ✅ |
| `finance.approve` | ❌ | ❌ | ✅ | ✅ |
| **Roles** |
| `roles.view` | ❌ | ❌ | ✅ | ✅ |
| `roles.manage` | ❌ | ❌ | ❌ | ✅ |
| **Library** |
| `library.manage` | ❌ | ❌ | ❌ | ✅ |

**Legend:**
- ✅ = Has permission
- ❌ = No permission
- ✅ (limited) = Has permission with restrictions

---

## 💻 Implementation Examples

### Backend (Rust)

#### Example 1: Check Single Permission
```rust
use crate::models::staff::UserPermissions;

async fn my_handler(user: User, pool: &PgPool) -> Result<Response> {
    // Check if user has permission
    if !user.has_permission(&pool, "users.create").await? {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "success": false,
                "error": "คุณไม่มีสิทธิ์สร้างผู้ใช้งาน"
            }))
        ));
    }
    
    // User has permission - continue
    // ...
    Ok(response)
}
```

#### Example 2: Check Multiple Permissions
```rust
async fn edit_important_data(user: User, pool: &PgPool) -> Result<Response> {
    // Check multiple permissions
    let has_edit = user.has_permission(&pool, "data.edit").await?;
    let has_approve = user.has_permission(&pool, "data.approve").await?;
    
    if !has_edit && !has_approve {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Continue...
}
```

#### Example 3: Check Role
```rust
async fn dept_head_only(user: User, pool: &PgPool) -> Result<Response> {
    // Check if user has specific role
    if !user.has_role(&pool, "DEPT_HEAD").await? {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Continue...
}
```

#### Example 4: Get All Permissions
```rust
async fn show_user_permissions(user: User, pool: &PgPool) -> Result<Response> {
    // Get all user's permissions
    let permissions = user.get_permissions(&pool).await?;
    
    Ok(Json(json!({
        "permissions": permissions
    })))
}
```

---

### API Examples (cURL)

#### Assign Role to User
```bash
curl -X POST "https://school-api.schoolorbit.app/api/users/{user_id}/roles" \
  -H "Content-Type: application/json" \
  -H "X-Subdomain: snwsb" \
  -H "Origin: https://snwsb.schoolorbit.app" \
  -b cookies.txt \
  -d '{
    "role_id": "uuid-of-role",
    "is_primary": true,
    "started_at": "2025-01-01"
  }'
```

#### Get User's Permissions
```bash
curl "https://school-api.schoolorbit.app/api/users/{user_id}/permissions" \
  -H "X-Subdomain: snwsb" \
  -H "Origin: https://snwsb.schoolorbit.app" \
  -b cookies.txt
```

**Response:**
```json
{
  "success": true,
  "data": [
    "users.view",
    "users.create",
    "students.view",
    "grades.edit",
    "attendance.mark"
  ]
}
```

---

## 🔧 Common Scenarios

### Scenario 1: Teacher Can Grade Students
```
User: อ.สมชาย
Role: TEACHER
Permissions: ["students.view", "grades.edit", "attendance.mark"]

✅ Can: View students, Edit grades, Mark attendance
❌ Cannot: Create users, Delete students, Approve budgets
```

### Scenario 2: Department Head Has Extended Access
```
User: อ.สมหญิง
Roles: [TEACHER, DEPT_HEAD]
Permissions: [
  // From TEACHER:
  "students.view", "grades.edit", "attendance.mark",
  // From DEPT_HEAD:
  "users.edit", "documents.approve_dept", "finance.view"
]

✅ Can: Everything a teacher can + Edit staff, Approve dept documents, View finance
❌ Cannot: Delete users, Approve school-wide budgets
```

### Scenario 3: Director Has Full Control
```
User: ผอ.สมศักดิ์
Role: DIRECTOR
Permissions: ["*"] // Wildcard

✅ Can: Everything
```

---

## 🚨 Error Responses

### 401 Unauthorized
```json
{
  "success": false,
  "error": "กรุณาเข้าสู่ระบบ"
}
```
**Cause:** No valid JWT token

---

### 403 Forbidden
```json
{
  "success": false,
  "error": "คุณไม่มีสิทธิ์เข้าถึงฟีเจอร์นี้"
}
```
**Cause:** User doesn't have required permission

---

### 500 Internal Server Error
```json
{
  "success": false,
  "error": "เกิดข้อผิดพลาดในการตรวจสอบสิทธิ์"
}
```
**Cause:** Database error during permission check

---

## 📈 Performance

- **Permission Check Time:** ~30ms (without cache)
- **Expected with Cache:** <5ms (future optimization)
- **Database Queries:** 2-3 per permission check
  1. Get user's roles
  2. Get roles' permissions
  3. Check against requested permission

---

## 🔄 Future Enhancements

- [ ] Permission caching (5-minute TTL)
- [ ] Audit logging for permission checks
- [ ] Time-based permissions (temporary access)
- [ ] Department-specific permissions
- [ ] Permission inheritance

---

## 📚 References

- **Implementation:** Phase 1 Complete (2025-12-21)
- **Testing:** Phase 2 In Progress (2025-12-22)
- **Database Migration:** `005_create_staff_management.sql`
- **Models:** `backend-school/src/models/staff.rs`
- **Middleware:** `backend-school/src/middleware/permission.rs`

---

## 📞 Support

For questions or issues with permissions:
1. Check this document first
2. Review the permission matrix
3. Test with `/api/users/:id/permissions` endpoint
4. Check backend logs for permission check errors

---

**Document Version:** 1.0  
**Last Reviewed:** 2025-12-22

---

## ✅ Implementation Status (Updated 2025-12-22)

### Staff Management Handlers
All 5 staff management handlers now have permission checks implemented:

- ✅ `list_staff` → requires `users.view`
- ✅ `get_staff_profile` → requires `users.view`
- ✅ `create_staff` → requires `users.create`
- ✅ `update_staff` → requires `users.edit`
- ✅ `delete_staff` → requires `users.delete`

**Implementation:** Uses `check_user_permission()` helper function  
**Status:** Production ready  
**Test Coverage:** Manual testing completed

### Helper Function Used
```rust
async fn check_user_permission(
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
    required_permission: &str,
) -> Result<User, Response>
```

This function:
1. Extracts JWT from cookie
2. Verifies token validity  
3. Fetches user from database
4. Checks permission using UserPermissions trait
5. Returns user if authorized, or error response if not

**Benefits:**
- Reusable across all handlers
- Consistent error messages
- Clean code separation
- Easy to maintain

---
