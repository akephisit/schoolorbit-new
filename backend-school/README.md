# Backend School - Module-Based Permission System

This backend implements a flexible **module-based permission system** for managing multi-tenant school applications.

## 🎯 Key Features

- **Module-Based Permissions**: Fine-grained access control based on modules (e.g., `attendance.*`, `staff.*`)
- **Feature Toggles**: Enable/disable system features dynamically per module
- **Dynamic Menu System**: Menu items automatically filtered by user permissions
- **Multi-Tenant**: Separate database per school with shared admin database

## 📚 Documentation

- **[Module Creation Guide](docs/MODULE_CREATION_GUIDE.md)** - Complete guide for adding new modules with permissions, feature toggles, and menus

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+
- PostgreSQL 14+
- Environment variables configured (see `.env.example`)

### Running Locally

```bash
# Install dependencies
cargo build

# Run migrations
sqlx migrate run

# Start server
cargo run
```

Server will start on `http://localhost:3002`

## 🔐 Permission System

### How It Works

Users are assigned **roles**, and roles have **permissions**. Permissions follow the pattern:

```
module.action.scope
```

**Examples:**
- `staff.read.all` - Read all staff records
- `attendance.update.own` - Update attendance for own classes
- `*.*.all` - Superadmin wildcard (full access)

### Module-Based Access

The system uses **module-based filtering**:

- User with `attendance.*` permission → Can manage attendance features/menus
- User with `staff.read.own` → Can manage staff-related features/menus
- Admin with `*.*.all` → Full access to everything

## 🎚️ Feature Toggles

Feature toggles allow enabling/disabling entire modules:

```sql
-- Example: Toggle attendance system
UPDATE feature_toggles 
SET is_enabled = false 
WHERE code = 'attendance_tracking';
```

**API Endpoints:**
- `GET /api/admin/features` - List all feature toggles (filtered by permission)
- `POST /api/admin/features/{id}/toggle` - Toggle a feature on/off

## 📋 Menu System

Menus are dynamically generated based on user permissions:

```sql
-- Menu item requires 'homework' module permission
INSERT INTO menu_items (code, name, path, required_permission, ...)
VALUES ('homework_list', 'การบ้าน', '/homework', 'homework', ...);
```

**How it works:**
1. User requests `/api/menu/user`
2. Backend fetches user's permissions
3. Filters menu items by `required_permission`
4. Returns only accessible menu items

## 🛠️ Adding a New Module

See **[Module Creation Guide](docs/MODULE_CREATION_GUIDE.md)** for step-by-step instructions.

**Quick checklist:**
1. ✅ Create permissions (`XXX_module_permissions.sql`)
2. ✅ Create feature toggle (`XXX_module_feature.sql`)
3. ✅ Create menu items (`XXX_module_menu.sql`)
4. ✅ Grant permissions to roles
5. ✅ Run migrations
6. ✅ Test!

## 📁 Project Structure

```
backend-school/
├── migrations/          # Database migrations
│   ├── 010_scoped_permissions.sql
│   ├── 011_settings_permissions.sql
│   └── 012_admin_menu_items.sql
├── src/
│   ├── handlers/        # API handlers
│   │   ├── feature_toggles.rs  # Feature toggle management
│   │   ├── menu_admin.rs       # Menu CRUD operations
│   │   └── menu.rs             # User menu endpoint
│   ├── models/          # Data models
│   ├── middleware/      # Auth & other middleware
│   └── main.rs          # Application entry point
└── docs/
    └── MODULE_CREATION_GUIDE.md  # Detailed guide
```

## 🔧 Environment Variables

```bash
# Database
DATABASE_URL=postgresql://user:pass@localhost/school_db
ADMIN_DATABASE_URL=postgresql://user:pass@localhost/admin_db

# JWT
JWT_SECRET=your-secret-key

# Server
PORT=3002
```

## 🧪 Testing

```bash
# Run tests
cargo test

# Check code
cargo check

# Format code
cargo fmt
```

## 📖 API Documentation

### Feature Toggles

| Method | Endpoint | Description | Permission |
|--------|----------|-------------|------------|
| GET | `/api/admin/features` | List all features | Any module permission |
| GET | `/api/admin/features/{id}` | Get feature details | Module permission |
| PUT | `/api/admin/features/{id}` | Update feature | Module permission |
| POST | `/api/admin/features/{id}/toggle` | Toggle on/off | Module permission |

### Menu Administration

| Method | Endpoint | Description | Permission |
|--------|----------|-------------|------------|
| GET | `/api/admin/menu/groups` | List menu groups | Authenticated |
| POST | `/api/admin/menu/groups` | Create group | Authenticated |
| PUT | `/api/admin/menu/groups/{id}` | Update group | Authenticated |
| DELETE | `/api/admin/menu/groups/{id}` | Delete group | Authenticated |
| GET | `/api/admin/menu/items` | List menu items | Module permission |
| POST | `/api/admin/menu/items` | Create menu item | Module permission |
| PUT | `/api/admin/menu/items/{id}` | Update menu item | Module permission |
| DELETE | `/api/admin/menu/items/{id}` | Delete menu item | Module permission |

### User Menu

| Method | Endpoint | Description | Permission |
|--------|----------|-------------|------------|
| GET | `/api/menu/user` | Get user's accessible menu | Authenticated |

## 🤝 Contributing

When adding new features:

1. Follow the module-based permission pattern
2. Create appropriate migrations
3. Update relevant documentation
4. Test with different permission levels

## 📝 License

[Your License Here]

## 🆘 Support

For questions or issues, please refer to:
- [Module Creation Guide](docs/MODULE_CREATION_GUIDE.md)
- Project documentation
- Development team

---

**Last Updated:** 2025-12-24
