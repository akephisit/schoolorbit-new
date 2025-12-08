# GitHub Container Registry Setup

คำแนะนำสำหรับ build และ push Docker images ไป GitHub Container Registry (ghcr.io)

---

## 🔑 Setup GitHub Token

### 1. สร้าง Personal Access Token

1. GitHub → Settings → Developer settings → Personal access tokens → Tokens (classic)
2. Generate new token (classic)
3. Permissions:
   - ✅ `write:packages` - Upload packages
   - ✅ `read:packages` - Download packages
   - ✅ `delete:packages` - Delete packages
4. Copy token (เก็บไว้ปลอดภัย)

### 2. ตั้งค่า Repository Secrets

1. Repository → Settings → Secrets and variables → Actions
2. New repository secret:
   - Name: `GHCR_TOKEN`
   - Value: (paste your token)

---

## 🚀 GitHub Actions Workflow

สร้างไฟล์ `.github/workflows/build-and-push.yml`:

```yaml
name: Build and Push Docker Images

on:
  push:
    branches: [ main ]
    paths:
      - 'backend-admin/**'
      - 'backend-school/**'
  workflow_dispatch:

env:
  REGISTRY: ghcr.io
  IMAGE_OWNER: akephisit

jobs:
  build-backend-admin:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write
    
    steps:
      - name: Checkout
        uses: actions/checkout@v4
      
      - name: Log in to GitHub Container Registry
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_OWNER }}/schoolorbit-backend-admin
          tags: |
            type=ref,event=branch
            type=sha,prefix={{branch}}-
            type=raw,value=latest,enable={{is_default_branch}}
      
      - name: Build and push
        uses: docker/build-push-action@v5
        with:
          context: .
          file: ./backend-admin/Dockerfile
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}

  build-backend-school:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write
    
    steps:
      - name: Checkout
        uses: actions/checkout@v4
      
      - name: Log in to GitHub Container Registry
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_OWNER }}/schoolorbit-backend-school
          tags: |
            type=ref,event=branch
            type=sha,prefix={{branch}}-
            type=raw,value=latest,enable={{is_default_branch}}
      
      - name: Build and push
        uses: docker/build-push-action@v5
        with:
          context: .
          file: ./backend-school/Dockerfile
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
```

---

## 🏷️ Image Tags

Workflow จะสร้าง tags:
- `latest` - สำหรับ main branch
- `main-abc1234` - SHA commit
- `main` - branch name

**ตัวอย่าง:**
```
ghcr.io/akephisit/schoolorbit-backend-admin:latest
ghcr.io/akephisit/schoolorbit-backend-admin:main
ghcr.io/akephisit/schoolorbit-backend-admin:main-abc1234
```

---

## 🔒 Package Visibility

### ตั้งค่า Package เป็น Public

1. GitHub → Profile → Packages
2. เลือก package (schoolorbit-backend-admin)
3. Package settings → Change visibility → Public
4. ยืนยัน

**ทำซ้ำกับ backend-school**

---

## 🐳 Pull Images

### Public package (ไม่ต้อง login)

```bash
docker pull ghcr.io/akephisit/schoolorbit-backend-admin:latest
docker pull ghcr.io/akephisit/schoolorbit-backend-school:latest
```

### Private package (ต้อง login)

```bash
# Login
echo $GHCR_TOKEN | docker login ghcr.io -u USERNAME --password-stdin

# Pull
docker pull ghcr.io/akephisit/schoolorbit-backend-admin:latest
```

---

## 📝 Manual Build & Push

### Build locally

```bash
# backend-admin
docker build -f backend-admin/Dockerfile \
  -t ghcr.io/akephisit/schoolorbit-backend-admin:latest .

# backend-school
docker build -f backend-school/Dockerfile \
  -t ghcr.io/akephisit/schoolorbit-backend-school:latest .
```

### Push to registry

```bash
# Login
echo $GHCR_TOKEN | docker login ghcr.io -u akephisit --password-stdin

# Push
docker push ghcr.io/akephisit/schoolorbit-backend-admin:latest
docker push ghcr.io/akephisit/schoolorbit-backend-school:latest
```

---

## ✅ Verify

### Check images exist

```bash
# List packages
gh api /user/packages

# Or visit:
https://github.com/akephisit?tab=packages
```

### Test pull

```bash
docker pull ghcr.io/akephisit/schoolorbit-backend-admin:latest
docker images | grep schoolorbit
```

---

## 🔄 Portainer Auto-Update

### Webhook Setup

1. Portainer → Stacks → backend-admin → Webhook
2. Enable "Update this stack from a webhook"
3. Copy webhook URL

### GitHub Workflow (Add after push)

```yaml
- name: Trigger Portainer Update
  run: |
    curl -X POST ${{ secrets.PORTAINER_WEBHOOK_URL }}
```

**Result:** Push code → Build image → Deploy อัตโนมัติ! 🚀

---

## 🐛 Troubleshooting

### "Permission denied" when pushing

```bash
# Check token permissions
# Token needs: write:packages

# Re-login
docker logout ghcr.io
echo $GHCR_TOKEN | docker login ghcr.io -u USERNAME --password-stdin
```

### Image ไม่อัพเดท

```bash
# Force pull
docker pull ghcr.io/akephisit/schoolorbit-backend-admin:latest --no-cache

# Portainer
# Stacks → Select → Re-pull image
```

---

**Ready to use GitHub Container Registry!** 🎉
