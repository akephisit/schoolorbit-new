# 📁 File Storage System Documentation

**Version:** 1.0  
**Date:** 2026-01-09  
**Migration:** 020_file_storage_system.sql

## 📋 Overview

SchoolOrbit uses a **path-based file storage system** with Cloudflare R2 (S3-compatible) for managing all uploaded files including:

- 👤 **User Profile Images**
- 📄 **Documents** (Transcripts, Certificates, ID Cards)
- 📚 **Course Materials** (Assignments, Resources)
- 🏫 **School Assets** (Logo, Banners)

## 🏗️ Architecture

### Storage Strategy: **Path-Based**

We store **relative paths** in the database, not full URLs. This provides:

✅ **Flexibility**: Change CDN/domain without database migration  
✅ **Multi-Environment**: Same paths work in dev/staging/production  
✅ **Cost-Effective**: Reduced database storage  
✅ **Migration-Friendly**: Easy to move between storage providers

### Storage Structure

```
R2 Bucket: schoolorbit-files/
│
├── school-{subdomain}/           # One folder per school (tenant isolation)
│   ├── users/
│   │   ├── profiles/            # Profile images
│   │   │   ├── {uuid}.jpg
│   │   │   └── thumbnails/      # Auto-generated thumbnails
│   │   │       └── {uuid}_thumb.jpg
│   │   └── documents/           # User documents
│   │       └── {user_id}/
│   │           ├── transcript.pdf
│   │           └── certificate.pdf
│   ├── courses/                 # Course materials
│   │   └── {course_id}/
│   │       ├── materials/
│   │       └── assignments/
│   ├── school/                  # School assets
│   │   ├── logo.png
│   │   ├── banner.jpg
│   │   └── documents/
│   └── temp/                    # Temporary files (auto-cleanup)
```

## 🗄️ Database Schema

### `files` Table

```sql
CREATE TABLE files (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(id),
    school_id VARCHAR(100),
    
    -- File Info
    filename VARCHAR(255) NOT NULL,
    original_filename VARCHAR(255) NOT NULL,
    file_size BIGINT NOT NULL,
    mime_type VARCHAR(100) NOT NULL,
    
    -- Storage (PATH, not URL!)
    storage_path TEXT NOT NULL UNIQUE,
    -- Example: "school-abc/users/profiles/550e8400.jpg"
    
    -- Classification
    file_type VARCHAR(50) NOT NULL,
    -- 'profile_image', 'document', 'transcript', etc.
    
    -- Image Metadata
    width INTEGER,
    height INTEGER,
    has_thumbnail BOOLEAN DEFAULT false,
    thumbnail_path TEXT,
    
    -- Lifecycle
    is_temporary BOOLEAN DEFAULT false,
    is_public BOOLEAN DEFAULT false,
    expires_at TIMESTAMPTZ,
    
    -- Security
    checksum VARCHAR(64),  -- SHA-256
    
    -- Audit
    uploaded_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ  -- Soft delete
);
```

### `users.profile_image_url` Field

**⚠️ Important:** Despite the name `profile_image_url`, this field now stores a **PATH**, not a full URL.

```sql
-- Example value:
profile_image_url = "school-abc/users/profiles/550e8400-e29b-41d4-a716-446655440000.jpg"

-- NOT:
profile_image_url = "https://cdn.schoolorbit.app/school-abc/users/..."
```

The backend converts paths to URLs at runtime using the configured base URL.

## ⚙️ Configuration

### Environment Variables (.env)

```bash
# Cloudflare R2 Configuration
R2_ACCOUNT_ID=your-cloudflare-account-id
R2_ACCESS_KEY_ID=your-r2-access-key-id
R2_SECRET_ACCESS_KEY=your-r2-secret-access-key
R2_BUCKET_NAME=schoolorbit-files
R2_PUBLIC_URL=https://pub-xxxxx.r2.dev
R2_REGION=auto

# Optional: CDN (recommended for production)
CDN_URL=https://cdn.schoolorbit.app

# File Limits
MAX_FILE_SIZE_MB=10
MAX_PROFILE_IMAGE_SIZE_MB=5
MAX_DOCUMENT_SIZE_MB=20

# Allowed Types
ALLOWED_IMAGE_TYPES=jpg,jpeg,png,webp,gif
ALLOWED_DOCUMENT_TYPES=pdf,doc,docx,xls,xlsx
```

### Getting Cloudflare R2 Credentials

1. Go to [Cloudflare Dashboard](https://dash.cloudflare.com)
2. Navigate to **R2 Object Storage**
3. Click **Manage R2 API Tokens**
4. Create a new API token with:
   - **Permissions**: Object Read & Write
   - **Apply to specific buckets**: Select your bucket
5. Copy the credentials:
   - Account ID
   - Access Key ID
   - Secret Access Key

## 🔄 Path → URL Conversion

### Backend (Rust)

```rust
// Helper function to convert path to URL
pub fn get_file_url(path: Option<&str>) -> Option<String> {
    path.map(|p| {
        let base_url = env::var("CDN_URL")
            .or_else(|_| env::var("R2_PUBLIC_URL"))
            .expect("R2_PUBLIC_URL or CDN_URL must be set");
        
        format!("{}/{}", base_url.trim_end_matches('/'), p)
    })
}

// Usage in API response
let user = get_user_from_db(id).await?;
let profile_url = get_file_url(user.profile_image_url.as_deref());

Json(UserResponse {
    id: user.id,
    name: user.first_name,
    profile_image_url: profile_url,  // Full URL sent to frontend
})
```

### Frontend (SvelteKit)

Frontend receives full URLs and uses them directly:

```svelte
<script>
  export let user;
</script>

<!-- user.profile_image_url is already a full URL -->
<img 
  src={user.profile_image_url || '/default-avatar.png'} 
  alt={user.name} 
/>
```

## 📊 File Types

| Type | Description | Storage Path | Max Size |
|------|-------------|--------------|----------|
| `profile_image` | User avatars | `school-{id}/users/profiles/` | 5 MB |
| `document` | General documents | `school-{id}/users/documents/{user_id}/` | 20 MB |
| `transcript` | Academic transcripts | `school-{id}/users/documents/{user_id}/` | 20 MB |
| `certificate` | Certificates | `school-{id}/users/documents/{user_id}/` | 20 MB |
| `course_material` | Teaching materials | `school-{id}/courses/` | 10 MB |
| `assignment` | Student assignments | `school-{id}/courses/` | 10 MB |
| `school_logo` | School branding | `school-{id}/school/` | 2 MB |
| `school_banner` | School banners | `school-{id}/school/` | 5 MB |

## 🔒 Security Best Practices

### 1. File Validation

Always validate:
- ✅ File type (MIME type checking)
- ✅ File size limits
- ✅ File extension
- ✅ Image dimensions (for images)

### 2. Access Control

- **Public Files**: Profile images, school logos
- **Private Files**: Documents, transcripts (use pre-signed URLs)

### 3. Virus Scanning (Recommended)

Consider integrating ClamAV or similar for uploaded files.

### 4. Checksums

Store SHA-256 checksums for file integrity verification.

## 🧹 Cleanup & Maintenance

### Temporary Files

Files with `is_temporary = true` should be cleaned up automatically:

```sql
-- Find expired temporary files
SELECT * FROM files
WHERE is_temporary = true
  AND expires_at < NOW()
  AND deleted_at IS NULL;
```

### Soft Deletes

Files are soft-deleted first (marked with `deleted_at`), then permanently removed after a grace period (e.g., 30 days).

### Orphaned Files

Periodically check for files in R2 that don't exist in the database.

## 📈 Future Enhancements

- [ ] **Image Optimization**: Automatic WebP conversion
- [ ] **Video Support**: Video uploads and streaming
- [ ] **Multi-Region**: Replicate to multiple R2 regions
- [ ] **Direct Upload**: Client → R2 direct upload (bypassing backend)
- [ ] **Thumbnails**: Automatic thumbnail generation for documents
- [ ] **Quotas**: Per-school storage quotas
- [ ] **Analytics**: Track storage usage per school

## 🔗 Related Documentation

- [Cloudflare R2 Docs](https://developers.cloudflare.com/r2/)
- [AWS S3 SDK for Rust](https://docs.rs/aws-sdk-s3/)
- Migration: `020_file_storage_system.sql`

## 📞 Support

For issues or questions, contact the SchoolOrbit development team.

---

**Last Updated:** 2026-01-09  
**Maintained By:** SchoolOrbit Team
