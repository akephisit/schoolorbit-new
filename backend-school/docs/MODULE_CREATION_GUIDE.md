# 🚀 วิธีเพิ่มระบบใหม่พร้อม Feature Toggle และ Menu

คู่มือนี้จะแนะนำวิธีการเพิ่มระบบใหม่เข้าสู่โปรเจค โดยจะครอบคลุมทั้ง permissions, feature toggles, menu items และการเชื่อมต่อทุกส่วน

---

## 📋 ตัวอย่าง: เพิ่มระบบ "การส่งการบ้าน" (Homework)

เราจะใช้ระบบการบ้านเป็นตัวอย่างในการอธิบาย

---

## ขั้นตอนที่ 1: เพิ่ม Permissions 🔐

สร้าง migration สำหรับ permissions ของระบบใหม่

**ไฟล์:** `backend-school/migrations/XXX_homework_permissions.sql`

```sql
-- Add homework module permissions
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  -- Basic permissions
  ('homework.read.own', 'ดูการบ้านของตัวเอง', 'homework', 'read', 'own', 'ดูการบ้านของห้องที่สอน'),
  ('homework.read.all', 'ดูการบ้านทั้งหมด', 'homework', 'read', 'all', 'ดูการบ้านทุกห้อง'),
  ('homework.create.all', 'สร้างการบ้าน', 'homework', 'create', 'all', 'สร้างการบ้านใหม่'),
  ('homework.update.own', 'แก้ไขการบ้านของตัวเอง', 'homework', 'update', 'own', 'แก้ไขการบ้านที่สร้าง'),
  ('homework.update.all', 'แก้ไขการบ้านทั้งหมด', 'homework', 'update', 'all', 'แก้ไขการบ้านทุกรายการ'),
  ('homework.delete.own', 'ลบการบ้านของตัวเอง', 'homework', 'delete', 'own', 'ลบการบ้านที่สร้าง'),
  ('homework.delete.all', 'ลบการบ้านทั้งหมด', 'homework', 'delete', 'all', 'ลบการบ้านทุกรายการ'),
  ('homework.grade.own', 'ให้คะแนนการบ้านของตัวเอง', 'homework', 'grade', 'own', 'ให้คะแนนการบ้านของห้องที่สอน'),
  ('homework.grade.all', 'ให้คะแนนการบ้านทั้งหมด', 'homework', 'grade', 'all', 'ให้คะแนนการบ้านทุกห้อง')
ON CONFLICT (code) DO NOTHING;
```

**💡 เคล็ดลับ:**
- ใช้ pattern: `module.action.scope`
- Module = ชื่อระบบ (เช่น homework, attendance, grades)
- Action = create, read, update, delete, หรือ custom (เช่น grade, approve)
- Scope = own, department, all

---

## ขั้นตอนที่ 2: เพิ่ม Feature Toggle 🎚️

เพิ่ม feature toggle ในฐานข้อมูล

**ไฟล์:** `backend-school/migrations/XXX_homework_feature.sql`

```sql
-- Add homework feature toggle
INSERT INTO feature_toggles (id, code, name, name_en, module, is_enabled)
VALUES (
    gen_random_uuid(),
    'homework_system',
    'ระบบการบ้าน',
    'Homework System',
    'homework',  -- ⚠️ สำคัญ! ต้องตรงกับ module ใน permissions
    true  -- เริ่มต้นเปิดใช้งาน
)
ON CONFLICT (code) DO NOTHING;
```

**💡 เคล็ดลับ:**
- `module` ต้องตรงกับ module ใน permissions
- `code` ควรเป็น snake_case และไม่ซ้ำ
- `is_enabled = true` ถ้าพร้อมใช้งาน, `false` ถ้ายังไม่พร้อม

---

## ขั้นตอนที่ 3: เพิ่ม Menu Items 📋

เพิ่มเมนูเข้าสู่ระบบ (2 วิธี)

### วิธีที่ 1: ใช้ Migration (แนะนำ)

**ไฟล์:** `backend-school/migrations/XXX_homework_menu.sql`

```sql
-- Add homework menu items
DO $$
DECLARE
    homework_group_id UUID;
    homework_main_id UUID;
BEGIN
    -- 1. Get or create menu group (ถ้ายังไม่มี)
    INSERT INTO menu_groups (id, code, name, name_en, icon, display_order, is_active)
    VALUES (
        gen_random_uuid(),
        'homework',
        'การบ้าน',
        'Homework',
        'BookOpen',  -- ใช้ชื่อ icon จาก lucide-svelte
        30,  -- ลำดับการแสดง (เลขน้อย = บนสุด)
        true
    )
    ON CONFLICT (code) DO UPDATE SET name = EXCLUDED.name
    RETURNING id INTO homework_group_id;
    
    -- ถ้า group มีอยู่แล้ว ดึง id มาใช้
    IF homework_group_id IS NULL THEN
        SELECT id INTO homework_group_id FROM menu_groups WHERE code = 'homework';
    END IF;
    
    -- 2. Add main menu item (รายการหลัก)
    INSERT INTO menu_items (
        id, code, name, name_en, path, icon,
        group_id, required_permission, display_order, is_active
    )
    VALUES (
        gen_random_uuid(),
        'homework_list',
        'รายการการบ้าน',
        'Homework List',
        '/homework',
        'List',
        homework_group_id,
        'homework',  -- ⚠️ ต้องตรง module! ใครมี homework.* ก็เห็น
        1,
        true
    )
    ON CONFLICT (code) DO NOTHING
    RETURNING id INTO homework_main_id;
    
    -- 3. Add sub-menu items (เมนูย่อย)
    INSERT INTO menu_items (
        id, code, name, name_en, path, icon,
        group_id, parent_id, required_permission, display_order, is_active
    )
    VALUES 
    (
        gen_random_uuid(),
        'homework_create',
        'สร้างการบ้าน',
        'Create Homework',
        '/homework/new',
        'Plus',
        homework_group_id,
        homework_main_id,  -- ⚠️ ระบุ parent_id
        'homework',
        2,
        true
    ),
    (
        gen_random_uuid(),
        'homework_report',
        'รายงานการบ้าน',
        'Homework Report',
        '/homework/report',
        'BarChart',
        homework_group_id,
        homework_main_id,
        'homework',
        3,
        true
    )
    ON CONFLICT (code) DO NOTHING;
    
END $$;
```

### วิธีที่ 2: ใช้ Admin UI

1. ไปที่ `/admin/menu`
2. คลิก "เพิ่มเมนู"
3. กรอกข้อมูล:
   - **รหัส:** `homework_list`
   - **ชื่อ (ไทย):** รายการการบ้าน
   - **Path:** `/homework`
   - **Icon:** `List`
   - **Module:** `homework` ⚠️ สำคัญ!
   - **Group:** เลือกกลุ่มที่ต้องการ

---

## ขั้นตอนที่ 4: รัน Migrations 🗄️

```bash
# Backend จะรันอัตโนมัติเมื่อ start
cd backend-school
cargo run

# หรือใช้ sqlx cli
sqlx migrate run
```

**ตรวจสอบ:**
```sql
-- ตรวจสอบ permissions
SELECT * FROM permissions WHERE module = 'homework';

-- ตรวจสอบ feature toggle
SELECT * FROM feature_toggles WHERE module = 'homework';

-- ตรวจสอบ menu items
SELECT mi.*, mg.name as group_name
FROM menu_items mi
JOIN menu_groups mg ON mi.group_id = mg.id
WHERE mi.required_permission = 'homework';
```

---

## ขั้นตอนที่ 5: เพิ่ม Permissions ให้ Role 👥

```sql
-- เพิ่ม homework permissions ให้ครู
UPDATE roles
SET permissions = array_append(permissions, 'homework.read.own')
WHERE name = 'ครู';

-- เพิ่มให้ admin
UPDATE roles
SET permissions = permissions || ARRAY[
    'homework.read.all',
    'homework.create.all',
    'homework.update.all',
    'homework.delete.all',
    'homework.grade.all'
]::varchar[]
WHERE name = 'ผู้ดูแลระบบ';
```

---

## ขั้นตอนที่ 6: เช็คใน Frontend 🎨

### ที่ 1: เช็ค Feature Toggle

ใน Frontend ต้องเช็ค feature toggle ก่อนแสดงฟีเจอร์

**ตัวอย่าง:** `src/routes/(app)/homework/+page.svelte`

```typescript
import { getFeature } from '$lib/api/feature-toggles';

let homeworkEnabled = $state(false);

$effect(() => {
    checkFeature();
});

async function checkFeature() {
    try {
        // เช็คว่าระบบเปิดใช้งานหรือไม่
        const feature = await getFeature('homework_system');
        homeworkEnabled = feature.is_enabled;
        
        if (!homeworkEnabled) {
            // Redirect หรือแสดง message
            toast.warning('ระบบการบ้านปิดใช้งานชั่วคราว');
        }
    } catch (error) {
        console.error('Failed to check feature:', error);
    }
}
```

### ที่ 2: เมนูจะแสดงอัตโนมัติ

เมนูจะแสดงอัตโนมัติถ้า:
- ✅ User มี permission ใน module `homework.*`
- ✅ Menu item มี `is_active = true`
- ✅ Menu item มี `required_permission = 'homework'`

**ไม่ต้องเขียนโค้ดเพิ่ม!** Sidebar component จะดึงจาก API `/api/menu/user` อัตโนมัติ

---

## ขั้นตอนที่ 7: ทดสอบระบบ ✅

### เช็คใน Admin Dashboard

1. **Feature Toggles** (`/admin/features`)
   - เปิด/ปิดระบบการบ้าน
   - เห็น feature card "ระบบการบ้าน"

2. **Menu Management** (`/admin/menu`)
   - เห็น menu items ของการบ้าน
   - แก้ไข/ลบได้

### เช็คใน User Menu

1. **Login เป็นครู** (มี `homework.read.own`)
   - เห็นเมนู "การบ้าน"
   - เข้าได้ที่ `/homework`

2. **Login เป็น User ธรรมดา** (ไม่มี homework.*)
   - ❌ ไม่เห็นเมนู "การบ้าน"
   - ❌ เข้า `/homework` ไม่ได้ (403 Forbidden)

3. **ปิด Feature Toggle**
   - เมนูยังเห็นอยู่ แต่หน้า homework แสดง warning

---

## 🎯 สรุปไหลงาน (Quick Checklist)

เมื่อเพิ่มระบบใหม่:

- [ ] **1. Permissions** - สร้าง module permissions
- [ ] **2. Feature Toggle** - เพิ่มใน `feature_toggles` table
- [ ] **3. Menu Group** - สร้างกลุ่มเมนู (ถ้าไม่มี)
- [ ] **4. Menu Items** - เพิ่มรายการเมนู (main + sub-items)
- [ ] **5. Grant Permissions** - เพิ่ม permissions ให้ roles
- [ ] **6. Run Migrations** - รัน migrations ทั้งหมด
- [ ] **7. Frontend** - เช็ค feature toggle ในโค้ด
- [ ] **8. Test** - ทดสอบทุก role และทุกสถานะ

---

## 📚 ตัวอย่าง Module ที่มีอยู่

ดูตัวอย่างจาก modules ที่มีอยู่แล้ว:

| Module | Permissions | Feature Toggle | Menu Path |
|--------|-------------|----------------|-----------|
| **staff** | `staff.read.own` | `staff_management` | `/staff` |
| **attendance** | `attendance.update.all` | `attendance_tracking` | `/attendance` |
| **grades** | `grades.read.own` | `grade_management` | `/grades` |
| **students** | `students.read.all` | `student_profiles` | `/students` |

**ดูโค้ด:**
```bash
# ดู permissions
cat migrations/010_scoped_permissions.sql

# ดู feature toggles (ถ้ามี)
grep -r "feature_toggles" migrations/

# ดู menu items
grep -r "menu_items" migrations/
```

---

## ⚠️ ข้อควรระวัง

1. **Module Name ต้องตรงกัน**
   - Permissions: `module = 'homework'`
   - Feature Toggle: `module = 'homework'`
   - Menu Item: `required_permission = 'homework'`

2. **UUID ใช้ `gen_random_uuid()`**
   - ไม่ใช้ UUID แบบ hardcode

3. **ON CONFLICT DO NOTHING**
   - ป้องกันข้อมูลซ้ำเมื่อรัน migration ซ้ำ

4. **Display Order**
   - เลขน้อย = แสดงบนสุด
   - Settings menu ใช้ 999 (ล่างสุด)

5. **Icon Names**
   - ใช้ชื่อจาก [Lucide Icons](https://lucide.dev)
   - ตัวพิมพ์ใหญ่ขึ้นต้น: `BookOpen`, `Users`, `Settings`

---

## 🚀 Bonus: Template Script

สร้างไฟล์ helper:

**`create_module.sh`**
```bash
#!/bin/bash
MODULE=$1
MODULE_UPPER=$(echo $MODULE | tr '[:lower:]' '[:upper:]')

echo "Creating module: $MODULE"

# Generate migration files
cat > "migrations/$(date +%s)_${MODULE}_permissions.sql" << EOF
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('${MODULE}.read.all', 'ดู${MODULE_UPPER}ทั้งหมด', '${MODULE}', 'read', 'all', 'ดู${MODULE_UPPER}ทุกรายการ'),
  ('${MODULE}.create.all', 'สร้าง${MODULE_UPPER}', '${MODULE}', 'create', 'all', 'สร้าง${MODULE_UPPER}ใหม่'),
  ('${MODULE}.update.all', 'แก้ไข${MODULE_UPPER}', '${MODULE}', 'update', 'all', 'แก้ไข${MODULE_UPPER}'),
  ('${MODULE}.delete.all', 'ลบ${MODULE_UPPER}', '${MODULE}', 'delete', 'all', 'ลบ${MODULE_UPPER}')
ON CONFLICT (code) DO NOTHING;
EOF

echo "✅ Created permissions migration"
echo "📝 Edit the file to add more specific permissions"
```

**วิธีใช้:**
```bash
chmod +x create_module.sh
./create_module.sh homework
```

---

**หมายเหตุ:** ระบบนี้ยืดหยุ่นมาก - คุณสามารถปรับแต่ง permissions, feature toggles, และ menus ได้ตามความต้องการ!
