-- Supply prices are integer satang; bundles intentionally resolve current versions on every read.
CREATE TABLE supply_categories (
 id UUID PRIMARY KEY DEFAULT uuidv7(), name VARCHAR(120) NOT NULL CHECK(length(btrim(name))>0),
 normalized_name TEXT NOT NULL UNIQUE, revision BIGINT NOT NULL DEFAULT 1 CHECK(revision>0)
);
INSERT INTO supply_categories(name,normalized_name) VALUES
 ('วัสดุสำนักงาน','วัสดุสำนักงาน'),('วัสดุการศึกษา','วัสดุการศึกษา'),('วัสดุคอมพิวเตอร์','วัสดุคอมพิวเตอร์'),
 ('วัสดุงานบ้านงานครัว','วัสดุงานบ้านงานครัว'),('วัสดุงานช่าง','วัสดุงานช่าง'),('วัสดุอื่น ๆ','วัสดุอื่น ๆ');
CREATE TABLE supply_items (
 id UUID PRIMARY KEY DEFAULT uuidv7(), name VARCHAR(200) NOT NULL CHECK(length(btrim(name))>0),
 normalized_name TEXT NOT NULL, description TEXT NOT NULL CHECK(length(description)<=5000),
 category_id UUID NOT NULL REFERENCES supply_categories(id) ON DELETE RESTRICT,
 unit VARCHAR(80) NOT NULL CHECK(length(btrim(unit))>0), normalized_unit TEXT NOT NULL,
 image_file_id UUID UNIQUE REFERENCES files(id) ON DELETE RESTRICT,
 revision BIGINT NOT NULL DEFAULT 1 CHECK(revision>0), created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(), UNIQUE(normalized_name,normalized_unit)
);
CREATE INDEX supply_items_category_idx ON supply_items(category_id,normalized_name,id);
CREATE TABLE supply_item_versions (
 id UUID PRIMARY KEY DEFAULT uuidv7(), item_id UUID NOT NULL REFERENCES supply_items(id) ON DELETE RESTRICT,
 version INTEGER NOT NULL CHECK(version>0), price_satang BIGINT NOT NULL CHECK(price_satang BETWEEN 0 AND 999999999999),
 shop VARCHAR(300) NOT NULL CHECK(length(btrim(shop))>0), revision BIGINT NOT NULL DEFAULT 1 CHECK(revision>0),
 created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT, created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(item_id,version)
);
CREATE TABLE supply_favorites (
 user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE, item_id UUID NOT NULL REFERENCES supply_items(id) ON DELETE CASCADE,
 PRIMARY KEY(user_id,item_id)
);
CREATE TABLE supply_bundles (
 id UUID PRIMARY KEY DEFAULT uuidv7(), name VARCHAR(200) NOT NULL CHECK(length(btrim(name))>0), shared BOOLEAN NOT NULL,
 owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT, revision BIGINT NOT NULL DEFAULT 1 CHECK(revision>0),
 created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX supply_bundles_owner_idx ON supply_bundles(owner_user_id,shared,id);
CREATE TABLE supply_bundle_entries (
 bundle_id UUID NOT NULL REFERENCES supply_bundles(id) ON DELETE CASCADE, item_id UUID NOT NULL REFERENCES supply_items(id) ON DELETE RESTRICT,
 quantity INTEGER NOT NULL CHECK(quantity BETWEEN 1 AND 1000000), PRIMARY KEY(bundle_id,item_id)
);
CREATE TABLE supply_proposals (
 id UUID PRIMARY KEY DEFAULT uuidv7(), name VARCHAR(200) NOT NULL CHECK(length(btrim(name))>0), description TEXT NOT NULL CHECK(length(description)<=5000),
 category_id UUID NOT NULL REFERENCES supply_categories(id) ON DELETE RESTRICT, unit VARCHAR(80) NOT NULL CHECK(length(btrim(unit))>0),
 image_file_id UUID UNIQUE REFERENCES files(id) ON DELETE RESTRICT, price_satang BIGINT NOT NULL CHECK(price_satang BETWEEN 0 AND 999999999999),
 shop VARCHAR(300) NOT NULL CHECK(length(btrim(shop))>0), source_file_id UUID UNIQUE REFERENCES files(id) ON DELETE RESTRICT,
 source_url TEXT CHECK(source_url IS NULL OR length(source_url)<=2048),
 status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN('pending','approved','rejected')),
 proposed_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT, reviewed_by UUID REFERENCES users(id) ON DELETE RESTRICT,
 reviewed_at TIMESTAMPTZ, rejection_reason VARCHAR(2000), item_id UUID REFERENCES supply_items(id) ON DELETE RESTRICT,
 revision BIGINT NOT NULL DEFAULT 1 CHECK(revision>0), created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 CHECK(source_file_id IS NULL OR source_url IS NULL), CHECK(image_file_id IS NULL OR image_file_id IS DISTINCT FROM source_file_id),
 CHECK((status='pending' AND reviewed_by IS NULL AND reviewed_at IS NULL AND item_id IS NULL AND rejection_reason IS NULL)
 OR(status='approved' AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL AND item_id IS NOT NULL AND rejection_reason IS NULL AND source_file_id IS NULL AND source_url IS NULL AND image_file_id IS NULL)
 OR(status='rejected' AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL AND item_id IS NULL AND rejection_reason IS NOT NULL AND source_file_id IS NULL AND source_url IS NULL AND image_file_id IS NULL))
);
CREATE INDEX supply_proposals_own_idx ON supply_proposals(proposed_by,created_at DESC,id DESC);
CREATE INDEX supply_proposals_pending_idx ON supply_proposals(created_at,id) WHERE status='pending';
INSERT INTO permissions(code,name,module,action,scope,description) VALUES
 ('supply_catalog.read.school','ดูทะเบียนวัสดุ','supply_catalog','read','school','ดูวัสดุ ราคา และจัดชุดวัสดุส่วนตัว'),
 ('supply_catalog.manage.school','จัดการทะเบียนวัสดุ','supply_catalog','manage','school','จัดหมวดหมู่ วัสดุ ราคา และชุดวัสดุส่วนกลาง'),
 ('supply_catalog.request.own','เสนอพัสดุใหม่','supply_catalog','request','own','เสนอและติดตามวัสดุของตนเอง'),
 ('supply_catalog.approve.school','ตรวจสอบวัสดุที่เสนอ','supply_catalog','approve','school','ตรวจสอบ แก้ไข อนุมัติ และปฏิเสธวัสดุที่เสนอ')
ON CONFLICT(code) DO UPDATE SET name=EXCLUDED.name,description=EXCLUDED.description,is_active=true;
INSERT INTO role_permissions(role_id,permission_id)
 SELECT r.id,p.id FROM roles r CROSS JOIN permissions p WHERE r.code IN('ADMIN','SUPER_ADMIN') AND p.module='supply_catalog' ON CONFLICT DO NOTHING;
INSERT INTO organization_permission_grants(organization_unit_id,permission_id,position_code)
 SELECT u.id,p.id,NULL FROM organization_units u CROSS JOIN permissions p WHERE u.is_active AND p.code IN('supply_catalog.read.school','supply_catalog.request.own') ON CONFLICT DO NOTHING;
INSERT INTO organization_permission_grants(organization_unit_id,permission_id,position_code)
 SELECT u.id,p.id,NULL FROM organization_units u CROSS JOIN permissions p WHERE u.is_active AND u.code='BUD-AST' AND p.code IN('supply_catalog.manage.school','supply_catalog.approve.school') ON CONFLICT DO NOTHING;
