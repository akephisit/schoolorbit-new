# 🎓 SchoolOrbit - Complete School Management System

Multi-tenant school management system with file storage, built with Rust (Backend) and SvelteKit (Frontend).

## 🏗️ Architecture

```
schoolorbit-new/
├── backend-admin/      # Admin service (tenant management)
├── backend-school/     # School service (multi-tenant)
├── frontend-school/    # SvelteKit frontend
└── docker-compose.yml  # Development environment
```

## ✨ Features

### Core Features
- 🏫 Multi-tenant architecture (one database per school)
- 👥 User management (Staff, Students, Admin)
- 🔐 Role-based permissions
- 📊 Dynamic menu system
- 🔒 Data encryption at rest
- 📱 PWA support

### File Storage (NEW! ✨)
- ☁️ Cloudflare R2 integration
- 📁 Path-based storage architecture
- 🖼️ Automatic image processing (resize, thumbnails)
- ✅ File validation & type checking
- 🔒 SHA-256 checksum verification
- 🗑️ Soft delete with cleanup
- 🌐 CDN-ready URL generation

## 🚀 Quick Start

### Prerequisites
- Docker & Docker Compose
- Cloudflare R2 Account (for file uploads)

### Setup

1. **Clone & Configure**
   ```bash
   git clone <repo-url>
   cd schoolorbit-new
   cp .env.example .env
   ```

2. **Configure R2 (Required for file uploads)**
   
   Get your credentials from [Cloudflare Dashboard](https://dash.cloudflare.com):
   - Go to R2 > Manage R2 API Tokens
   - Create API Token with Read & Write permissions
   - Create a bucket named `schoolorbit-files`
   - Copy credentials to `.env`:
   
   ```bash
   R2_ACCOUNT_ID=your-account-id
   R2_ACCESS_KEY_ID=your-access-key
   R2_SECRET_ACCESS_KEY=your-secret-key
   R2_BUCKET_NAME=schoolorbit-files
   R2_PUBLIC_URL=https://pub-xxxxx.r2.dev
   ```

3. **Start Services**
   ```bash
   docker-compose up -d
   ```

4. **Access Services**
   - Admin API: http://localhost:8080
   - School API: http://localhost:8081
   - Frontend: Configure separately

## 📚 Documentation

### Backend Services
- [Backend Admin](./backend-admin/README.md) - Tenant management
- [Backend School](./backend-school/README.md) - Multi-tenant school service
- [File Storage System](./backend-school/docs/FILE_STORAGE.md) - Complete file storage guide

### Setup Guides
- [R2 Setup Script](./backend-school/scripts/setup_r2.sh) - Interactive R2 configuration

## 🔧 Development

### Backend (Rust)
```bash
cd backend-school
cargo build
cargo run
```

### Frontend (SvelteKit)
```bash
cd frontend-school
npm install
npm run dev
```

## 📦 File Storage Usage

### Upload File
```bash
curl -X POST http://localhost:8081/api/files/upload \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@photo.jpg" \
  -F "file_type=profile_image"
```

### Response
```json
{
  "success": true,
  "file": {
    "id": "uuid",
    "filename": "uuid.jpg",
    "original_filename": "photo.jpg",
    "url": "https://cdn.schoolorbit.app/school-abc/users/profiles/uuid.jpg",
    "thumbnail_url": "https://cdn.schoolorbit.app/school-abc/users/profiles/thumbnails/uuid_thumb.jpg",
    "file_size": 102400,
    "mime_type": "image/jpeg",
    "width": 1920,
    "height": 1080
  }
}
```

## 🔐 Security

- ✅ JWT-based authentication
- ✅ AES-256 encryption for sensitive data
- ✅ Per-tenant data isolation
- ✅ File validation & virus scan ready
- ✅ CORS protection
- ✅ Rate limiting ready

## 🌐 Environment Variables

See [.env.example](./.env.example) for complete list.

### Required
- `ENCRYPTION_KEY` - Database encryption key (min 32 chars)
- `R2_ACCOUNT_ID` - Cloudflare R2 account ID
- `R2_ACCESS_KEY_ID` - R2 access key
- `R2_SECRET_ACCESS_KEY` - R2 secret key
- `R2_BUCKET_NAME` - R2 bucket name
- `R2_PUBLIC_URL` - R2 public URL

### Optional
- `CDN_URL` - CDN URL for better performance
- `MAX_FILE_SIZE_MB` - Max file size (default: 10)
- `MAX_PROFILE_IMAGE_SIZE_MB` - Max profile image size (default: 5)

## 🧪 Testing

### Backend
```bash
cd backend-school
cargo test
```

### Integration Test
```bash
# Test file upload
./backend-school/scripts/setup_r2.sh
```

## 📈 Performance

- Multi-tenant connection pooling
- Lazy migration loading
- CDN-ready architecture
- Automatic image optimization
- Efficient storage paths

## 🤝 Contributing

1. Fork the repository
2. Create feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open Pull Request

## 📝 License

[Your License Here]

## 🆘 Support

For issues and questions:
- 📖 Check [Documentation](./backend-school/docs/)
- 🐛 Open an [Issue](../../issues)
- 💬 Contact: [Your Contact]

---

**Built with ❤️ using Rust 🦀 and SvelteKit ⚡**
