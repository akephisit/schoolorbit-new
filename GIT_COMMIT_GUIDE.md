# Git Commit Guide

คำแนะนำการ commit สำหรับการ refactor architecture

## 📋 สิ่งที่เปลี่ยนแปลง

### 1. Architecture Refactoring
- แยก database lifecycle ไป backend-school
- backend-admin เป็น orchestrator เท่านั้น
- Inter-service communication ผ่าน HTTP API

### 2. Files Removed
- `backend-admin/src/services/neon.rs` → moved to backend-school
- `backend-admin/templates/` → moved to backend-school/migrations

### 3. New Service
- `backend-school/` - Database lifecycle management service
- Handles Neon API, migrations, database provisioning

### 4. Configuration Updates
- Updated .gitignore (comprehensive patterns)
- Created backend-school/.env.example
- Updated backend-admin/.env.example (removed Neon vars)

---

## 🎯 Suggested Commit Messages

### Option 1: Single Commit (Recommended for small teams)

```bash
git add .
git commit -m "refactor: separate database lifecycle into backend-school service

BREAKING CHANGE: Database provisioning now handled by backend-school

- Create backend-school microservice for database management
- Move Neon API client from backend-admin to backend-school
- Implement inter-service communication via HTTP
- Update environment variable configuration
- Improve .gitignore patterns
- Remove unused code and templates

backend-admin now orchestrates via backend-school API
Requires backend-school to be running on port 8081"
```

### Option 2: Multiple Commits (Recommended for detailed history)

```bash
# Commit 1: Infrastructure
git add .gitignore
git commit -m "chore: improve .gitignore patterns

- Add comprehensive Rust workspace patterns
- Add SvelteKit and Wrangler ignores
- Remove tracked target/ directory"

# Commit 2: New Service
git add backend-school/
git commit -m "feat: create backend-school database service

- New microservice for database lifecycle management
- Handles Neon API for database provisioning
- Runs migrations on database creation
- Exposes API on port 8081"

# Commit 3: Refactor backend-admin
git add backend-admin/
git commit -m "refactor: delegate database provisioning to backend-school

BREAKING CHANGE: Requires backend-school service

- Remove Neon API client from backend-admin
- Update deployment service to call backend-school API
- Remove unused migration templates
- Update environment configuration"

# Commit 4: Documentation
git add *.md
git commit -m "docs: add inter-service communication and cleanup guides

- Add INTER_SERVICE_COMMUNICATION.md
- Add CLEANUP_SUMMARY.md
- Update deployment documentation"
```

---

## ✅ Pre-Commit Checklist

- [ ] All files build successfully
- [ ] No compilation errors
- [ ] Environment examples updated
- [ ] Unused files removed
- [ ] .gitignore comprehensive
- [ ] Documentation updated

---

## 🚀 Post-Commit Actions

### Local Testing
```bash
# Terminal 1
cd backend-school
cargo run --release

# Terminal 2
cd backend-admin
cargo run --release

# Test school creation
curl -X POST http://localhost:8080/api/v1/schools ...
```

### Deployment
1. Deploy backend-school first
2. Update backend-admin environment (BACKEND_SCHOOL_URL)
3. Deploy backend-admin
4. Test end-to-end flow

---

## 📊 Impact Summary

**Lines Changed:**
- Added: ~500 lines (backend-school)
- Removed: ~200 lines (cleanup)
- Modified: ~100 lines (refactoring)

**Services:**
- Before: 1 service (backend-admin)
- After: 2 services (backend-admin + backend-school)

**Complexity:**
- Separation of concerns: ✅ Improved
- Testability: ✅ Improved
- Scalability: ✅ Improved

---

**Ready to commit!** 🎉
