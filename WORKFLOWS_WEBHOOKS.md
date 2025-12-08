# GitHub Workflows + Portainer Webhooks Setup

คำแนะนำสำหรับตั้งค่า Auto-deployment

---

## 🎯 Workflows Overview

### 1. deploy-backend-admin.yml
- **Triggers:** เมื่อ push ไป `main` และมีการแก้ไขใน `backend-admin/`
- **Actions:**
  - Build Docker image
  - Push ไป `ghcr.io/akephisit/schoolorbit-backend-admin:latest`
  - Trigger Portainer webhook

### 2. deploy-backend-school.yml
- **Triggers:** เมื่อ push ไป `main` และมีการแก้ไขใน `backend-school/`
- **Actions:**
  - Build Docker image
  - Push ไป `ghcr.io/akephisit/schoolorbit-backend-school:latest`
  - Trigger Portainer webhook

---

## 🔧 Portainer Webhook Setup

### Step 1: สร้าง Webhook ใน Portainer

#### สำหรับ backend-admin stack:

1. Portainer → Stacks → **backend-admin**
2. Scroll down → **Webhooks**
3. Enable "Update this stack from a webhook"
4. **Copy webhook URL** (เช่น: `https://portainer.your-domain.com/api/webhooks/abc123`)

#### สำหรับ backend-school stack:

1. Portainer → Stacks → **backend-school**
2. Scroll down → **Webhooks**
3. Enable "Update this stack from a webhook"
4. **Copy webhook URL** (เช่น: `https://portainer.your-domain.com/api/webhooks/def456`)

---

### Step 2: เพิ่ม Secrets ใน GitHub

1. Repository → **Settings** → **Secrets and variables** → **Actions**
2. New repository secret:

**Secret 1:**
- Name: `PORTAINER_WEBHOOK_URL`
- Value: `https://portainer.your-domain.com/api/webhooks/abc123`
  (webhook URL ของ backend-admin)

**Secret 2:**
- Name: `PORTAINER_WEBHOOK_URL_SCHOOL`
- Value: `https://portainer.your-domain.com/api/webhooks/def456`
  (webhook URL ของ backend-school)

---

## 🚀 How It Works

### Scenario 1: แก้ไข backend-admin

```bash
# แก้ไข code
vim backend-admin/src/main.rs

# Commit & Push
git add .
git commit -m "feat: update backend-admin"
git push origin main
```

**ผลลัพธ์:**
```
1. GitHub Actions triggers: deploy-backend-admin.yml
2. Build Docker image
3. Push to ghcr.io/akephisit/schoolorbit-backend-admin:latest
4. Trigger Portainer webhook (PORTAINER_WEBHOOK_URL)
5. Portainer pulls new image and restarts stack
```

### Scenario 2: แก้ไข backend-school

```bash
# แก้ไข code
vim backend-school/src/main.rs

# Commit & Push
git add .
git commit -m "feat: update backend-school"
git push origin main
```

**ผลลัพธ์:**
```
1. GitHub Actions triggers: deploy-backend-school.yml
2. Build Docker image
3. Push to ghcr.io/akephisit/schoolorbit-backend-school:latest
4. Trigger Portainer webhook (PORTAINER_WEBHOOK_URL_SCHOOL)
5. Portainer pulls new image and restarts stack
```

### Scenario 3: แก้ไขทั้ง 2 อัน

```bash
# แก้ไขทั้ง 2
vim backend-admin/src/main.rs
vim backend-school/src/main.rs

# Commit & Push
git add .
git commit -m "feat: update both services"
git push origin main
```

**ผลลัพธ์:**
```
1. GitHub Actions triggers: ทั้ง 2 workflows พร้อมกัน
2. Build images แยกกัน (parallel)
3. Deploy แยกกัน
```

---

## ✅ Verification

### 1. ตรวจสอบ GitHub Actions

Repository → **Actions**
- เห็น workflows running/completed
- Check logs

### 2. ตรวจสอบ Images

```bash
# Pull latest
docker pull ghcr.io/akephisit/schoolorbit-backend-admin:latest
docker pull ghcr.io/akephisit/schoolorbit-backend-school:latest

# Check image date
docker images | grep schoolorbit
```

### 3. ตรวจสอบ Portainer

Stacks → Select stack → **Event log**
- เห็น "Stack updated via webhook"
- Container restarted

---

## 🐛 Troubleshooting

### Webhook ไม่ทำงาน

```bash
# Test webhook manually
curl -X POST "https://portainer.your-domain.com/api/webhooks/abc123"

# Check response:
# - 200 OK = success
# - 401 Unauthorized = invalid webhook URL
# - 404 Not Found = stack ไม่มี
```

### Image ไม่อัพเดท

```bash
# ใน Portainer stack settings:
# Always pull image: ✅ ON
# Re-pull image on webhook: ✅ ON
```

### Workflow ไม่ trigger

```bash
# Check paths in workflow
# ต้อง match กับไฟล์ที่แก้:
paths:
  - 'backend-admin/**'  # แก้อะไรก็ได้ใน folder นี้

# Test manual trigger:
# Actions → Select workflow → Run workflow
```

---

## 📊 Workflow Status Badge

เพิ่ม badge ใน README:

```markdown
![Backend Admin](https://github.com/YOUR_USERNAME/schoolorbit-new/actions/workflows/deploy-backend-admin.yml/badge.svg)
![Backend School](https://github.com/YOUR_USERNAME/schoolorbit-new/actions/workflows/deploy-backend-school.yml/badge.svg)
```

---

## 🔒 Security Best Practices

### 1. Protected Branches

Repository → Settings → Branches → **Add rule**
- Branch name: `main`
- Require pull request reviews: ✅
- Require status checks: ✅ (select: deploy-backend-admin, deploy-backend-school)

### 2. Environment Protection

Actions → Environments → **production**
- Required reviewers: (add yourself)
- Deployment protection rules

### 3. Webhook Security

- ใช้ HTTPS เท่านั้น
- Webhook URL เป็น secret (อย่าเผยแพร่)
- ถ้าเป็นไปได้ ใช้ Portainer authentication token

---

## 📝 Quick Reference

| Service | Workflow File | Secret Name | Image |
|---------|--------------|-------------|-------|
| backend-admin | deploy-backend-admin.yml | `PORTAINER_WEBHOOK_URL` | ghcr.io/akephisit/schoolorbit-backend-admin |
| backend-school | deploy-backend-school.yml | `PORTAINER_WEBHOOK_URL_SCHOOL` | ghcr.io/akephisit/schoolorbit-backend-school |

---

**Auto-deployment พร้อมใช้งาน!** 🎉
