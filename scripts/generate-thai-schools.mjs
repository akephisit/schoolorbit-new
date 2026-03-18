/**
 * generate-thai-schools.mjs
 *
 * ดาวน์โหลดข้อมูลโรงเรียนทั่วประเทศจาก OBEC Open Data
 * แล้วสร้างไฟล์ thai-schools.json สำหรับ SchoolCombobox
 *
 * วิธีใช้:
 *   node scripts/generate-thai-schools.mjs
 *
 * Output:
 *   frontend-school/src/lib/data/thai-schools.json
 */

import { writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const OUTPUT_PATH = join(__dirname, '../frontend-school/src/lib/data/thai-schools.json');

const OBEC_API = 'https://opendata.edudev.in.th/v1/OBEC_SCHOOL_007';

console.log('📥 กำลังดาวน์โหลดข้อมูลโรงเรียนจาก OBEC Open Data...');
console.log(`   URL: ${OBEC_API}`);
console.log('   (อาจใช้เวลาสักครู่ ข้อมูลมีขนาดใหญ่)\n');

let rawData;
try {
  const res = await fetch(OBEC_API);
  if (!res.ok) throw new Error(`HTTP ${res.status}: ${res.statusText}`);
  rawData = await res.json();
} catch (err) {
  console.error('❌ ดาวน์โหลดไม่สำเร็จ:', err.message);
  console.error('\n💡 ลองวิธีสำรอง:');
  console.error('   1. เปิด https://data.go.th/dataset/thailand-school');
  console.error('   2. ดาวน์โหลดไฟล์ CSV/JSON');
  console.error('   3. รัน: node scripts/generate-thai-schools.mjs --file <path-to-downloaded-file>');
  process.exit(1);
}

// รองรับหลาย format ที่อาจได้จาก API
const records = Array.isArray(rawData)
  ? rawData
  : rawData?.result ?? rawData?.data ?? rawData?.records ?? [];

if (records.length === 0) {
  console.error('❌ ไม่พบข้อมูลในไฟล์ ตรวจสอบ API response format');
  console.log('Raw response keys:', Object.keys(rawData));
  process.exit(1);
}

console.log(`✅ ดาวน์โหลดสำเร็จ: ${records.length.toLocaleString()} รายการ`);
console.log('🔍 ตัวอย่าง field ที่มี:', Object.keys(records[0]).slice(0, 10).join(', '));

// Map field names — OBEC API อาจใช้ชื่อ field แตกต่างกัน
function getSchoolName(record) {
  return (
    record.schoolName ||
    record.school_name ||
    record.SchoolName ||
    record.SCHOOL_NAME ||
    record.ชื่อโรงเรียน ||
    record.name ||
    ''
  ).trim();
}

function getProvince(record) {
  return (
    record.province ||
    record.province_name ||
    record.ProvinceName ||
    record.PROVINCE_NAME ||
    record.จังหวัด ||
    ''
  ).trim();
}

// แปลงและ deduplicate
const seen = new Set();
const schools = records
  .map((r) => ({
    name: getSchoolName(r),
    province: getProvince(r),
  }))
  .filter((s) => {
    if (!s.name || !s.province) return false;
    const key = s.name;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  })
  .sort((a, b) => a.province.localeCompare(b.province, 'th') || a.name.localeCompare(b.name, 'th'));

console.log(`\n📊 หลัง filter & deduplicate: ${schools.length.toLocaleString()} โรงเรียน`);

// ตัวอย่าง
console.log('\n📋 ตัวอย่าง 3 รายการแรก:');
schools.slice(0, 3).forEach((s) => console.log(`   ${s.name} (${s.province})`));

writeFileSync(OUTPUT_PATH, JSON.stringify(schools, null, 2), 'utf-8');

const fileSizeKB = Math.round(
  Buffer.byteLength(JSON.stringify(schools)) / 1024
);
console.log(`\n✅ บันทึกแล้ว: ${OUTPUT_PATH}`);
console.log(`   ขนาดไฟล์: ~${fileSizeKB} KB`);
