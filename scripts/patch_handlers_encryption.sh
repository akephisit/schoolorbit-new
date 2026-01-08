#!/bin/bash
# Auto-patch handlers for encryption support

set -e

echo "🔧 Patching handlers for encryption support..."

# File to patch
STAFF_FILE="backend-school/src/handlers/staff.rs"

echo "📝 Backing up original file..."
cp "$STAFF_FILE" "${STAFF_FILE}.backup"

echo "🔄 Applying encryption patches..."

# Patch 1: Add encryption setup in create_staff (after permission check)
# Find line with "Check permission" and add encryption setup after it
sed -i '/Check permission/,/}; *$/s/};/};\n\n    \/\/ Setup encryption key for encrypted columns\n    if let Err(e) = crate::utils::encryption::setup_encryption_key(\&pool).await {\n        eprintln!("❌ Encryption setup failed: {}", e);\n        return (\n            StatusCode::INTERNAL_SERVER_ERROR,\n            Json(json!({\n                "success": false,\n                "error": "ระบบไม่พร้อมใช้งาน"\n            })),\n        )\n            .into_response();\n    }/' "$STAFF_FILE"

# Patch 2: Update existing user check to decrypt national_id
sed -i 's/SELECT id, status FROM users WHERE national_id = \$1/SELECT id, status FROM users WHERE pgp_sym_decrypt(national_id, current_setting('\''app.encryption_key'\'')) = $1/g' "$STAFF_FILE"

# Patch 3: Update INSERT to encrypt national_id
sed -i 's/INSERT INTO users (/INSERT INTO users (/g' "$STAFF_FILE"
sed -i '/INSERT INTO users/,/RETURNING id/ {
    s/VALUES (\$1,/VALUES (\n                pgp_sym_encrypt($1, current_setting('\''app.encryption_key'\'')),/
}' "$STAFF_FILE"

# Patch 4: Update SELECT to decrypt national_id in get_staff_profile  
sed -i 's/SELECT id, national_id, email,/SELECT id, pgp_sym_decrypt(national_id, current_setting('\''app.encryption_key'\'')) as national_id, email,/g' "$STAFF_FILE"

echo "✅ Patches applied successfully!"
echo ""
echo "📋 Summary of changes:"
echo "  - Added encryption setup in create_staff"
echo "  - Updated duplicate check to decrypt national_id"
echo "  - Updated INSERT to encrypt national_id"
echo "  - Updated SELECT to decrypt national_id"
echo ""
echo "🔍 Next: cargo check to verify"
