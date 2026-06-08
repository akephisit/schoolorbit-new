-- เปลี่ยนจาก logo_url (full URL) เป็น logo_path (storage path)
-- เพื่อให้ independent จาก storage provider
ALTER TABLE school_settings RENAME COLUMN logo_url TO logo_path;
