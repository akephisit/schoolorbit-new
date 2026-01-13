# Migration Plan: National ID to Username

This document outlines the step-by-step plan to migrate the authentication system from using `national_id` (Thai Citizen ID) to a safe `username` based system.

## 1. Database Migration (Backend)
- [ ] Create a new migration file `migrations/018_add_username_column.sql`.
- [ ] Add `username` column to `users` table: `VARCHAR(50) UNIQUE`.
- [ ] **Data Migration Script (within SQL):**
    - Update **Students**: Set `username` = `'S' || student_info.student_id`.
    - Update **Staff**: Set `username` = `'T' || LPAD(ROW_NUMBER() OVER (ORDER BY created_at)::TEXT, 4, '0')` (Example: T0001, T0002) for existing staff who don't have a specific code.
    - Update **Parents**: Set `username` = `'P' || users.phone` (if phone exists) OR `'P' || users.national_id` (temporary fallback).
    - **Make column NOT NULL** after backfilling.
- [ ] Add Index on `username` for fast lookups.

## 2. Backend Code Updates (`backend-school`)

### Models (`src/models/auth.rs` & others)
- [ ] Update `User` struct to include `username`.
- [ ] Update `LoginRequest` struct:
    - Replace `national_id` with `username`.
    - Update validation logic (remove 13-digit check).
- [ ] Update `Claims` struct (JWT):
    - Remove `national_id`.
    - Add `username`.
    - Add `role` or `user_type` explicitly if needed for frontend logic.

### Handlers (`src/handlers/`)
#### `auth.rs` (Login)
- [ ] Change SQL query in `login` function to search by `username = $1` instead of `national_id_hash`.
- [ ] Update error messages (e.g., "Username or password incorrect").
- [ ] Update JWT token generation to use the new `username` claim.

#### `students.rs` (Create Student)
- [ ] Update `CreateStudentRequest` to accept `student_id` (already there) and auto-generate `username` as `S + student_id` inside the handler.
- [ ] Ensure `username` is saved to `users` table.

#### `staff.rs` (Create Staff)
- [ ] Update `CreateStaffRequest`.
    - Option A: Admin manually inputs Username.
    - Option B: Auto-generate `T` + `Running Number` (Requires a sequence or logic).
    - **Decision:** Let's add an optional `username` field. If not provided, auto-generate based on a new sequence `staff_code_seq`.

#### `provision.rs` (Tenant Creation)
- [ ] Update admin creation logic to set a default username (e.g., `admin`).

## 3. Frontend Code Updates (`frontend-school`)

### Login Page (`src/routes/login/+page.svelte`)
- [ ] Change Label "National ID" -> "Username".
- [ ] Remove 13-digit validation.
- [ ] Update `handleSubmit` to send `{ username, password }`.
- [ ] Update placeholder to show examples: `S67001`, `T105`.

### Staff Management (`src/routes/(app)/staff/manage/new/+page.svelte`)
- [ ] Add "Username" input field (Auto-filled if possible, or manual).
- [ ] Validation logic updates.

### Student Management (`src/routes/(app)/staff/students/new/+page.svelte`)
- [ ] Show the auto-generated Username (Read-only or Editable) based on Student ID.

### Profile Pages (Student & Staff)
- [ ] Display "Username" in the profile section.

## 4. Verification & Testing
- [ ] Run migration `sqlx migrate run`.
- [ ] Verify `users` table has populated usernames.
- [ ] Test Login with:
    - Student: `S...`
    - Staff: `T...`
- [ ] Test Create New Student/Staff.
- [ ] Verify JWT token contents (using jwt.io or log) to ensure `national_id` is gone.

## 5. Security Cleanup
- [ ] (Optional Future Step) Evaluate if `national_id` column needs to be kept encrypted or moved to `*_info` tables strictly for record-keeping, avoiding `users` table bloat. (Current plan: keep it for record but stop using for Auth).
