# Timetable PDF — 2×2 Grid Layout Option

**Date:** 2026-05-11
**Status:** Design approved (pending review)

## Goal

ให้ user เลือกได้ตอน export ตารางสอน/ตารางเรียน PDF ว่าจะ:
- **1 ตาราง/หน้า** (default — เหมือนเดิม รายละเอียดเต็ม)
- **4 ตาราง/หน้า** (2×2 grid — สำหรับเปรียบเทียบหลายห้อง/หลายครูพร้อมกัน)

Use case: เปรียบเทียบตารางหลายห้องเรียน/หลายครูข้างกัน เพื่อดูความสัมพันธ์ของคาบต่าง ๆ

## UI Changes — `timetable/+page.svelte`

ใน `exportModal` เพิ่ม radio group "รูปแบบหน้า":
- `1 ตาราง/หน้า` (default)
- `4 ตาราง/หน้า (2×2)`

State: `let exportLayout = $state<'full' | 'grid-2x2'>('full')`

ส่ง `{ layout: exportLayout }` ไปยัง `generateTimetablePDF`

## PDF Generator API — `pdf.ts`

```ts
interface GeneratePdfOptions {
    layout?: 'full' | 'grid-2x2';
}

generateTimetablePDF(
    pages: TimetablePage[],
    fileName?: string,
    options?: GeneratePdfOptions
): Promise<void>
```

Default `layout = 'full'` — backwards compatible

## Full Mode (existing — no change)

แต่ละ `TimetablePage` → 1 หน้า PDF, ข้อมูลครบ (subject + teacher + classroom + room)

## Grid 2×2 Mode

จัด `pages` เป็น chunks ละ 4 → 1 chunk = 1 หน้า PDF

### Page structure (A4 landscape)
```
+-----------------------------------+
| [Logo] ตารางเรียน ภาค X/YYYY      |  ← Title block (full width, 1 ครั้ง/หน้า)
|-----------------------------------|
| ม.1/1                | ม.1/2      |  ← mini titles
| +------+ +-----+...  | +------+...|
| | mini timetable A   | mini B     |
| +------+ +-----+...  | +------+...|
|-----------------------------------|
| ม.1/3                | ม.1/4      |
| ... mini timetable C | mini D     |
|-----------------------------------|
| Footer                            |
+-----------------------------------+
```

### Cell budget (per mini)
A4 landscape usable: 821.89 × 545pt
- Top title block: ~45pt
- Footer: ~15pt
- Gap between minis: 10pt × 2 = 20pt
- Remaining: 821.89 × 465pt → split 2×2 = ~400 × 232pt per mini

### Mini-table styling (override จาก full)
| Property | Full | Grid-2×2 |
|----------|------|----------|
| paddingLeft/Right | 2pt | 1pt |
| paddingTop/Bottom | 2pt | 1pt |
| vLineWidth | 1pt | 0.5pt |
| Row height | 50pt | ~33pt (5 rows × 33 = 165pt, fits ~232 - header 25 = 207pt area) |
| dayCol width | 40pt | 25pt content |
| subject_code fontSize | 8pt | 5pt |
| subject_name fontSize | 7pt | 4pt |
| day label fontSize | 10pt | 6pt |
| period header fontSize | 8pt/7pt | 5pt/4pt |

### Mini-cell content (simplified)
- **COURSE**:
  - line 1: subject_code (bold, 5pt)
  - line 2: subject_name_th — truncate ที่ 8 code points แล้วใส่ "…" ถ้าตัด (4pt)
- **ACTIVITY**: title truncate ที่ 8 code points + "…" (5pt, bold)
- **ครู / ห้อง / ห้องเรียน**: ซ่อนใน mini mode (ไม่พอที่)
- Helper: `truncate(s, max=8) → s.length > max ? s.slice(0, max) + '…' : s`

### Width formula (mini)
ใช้สมการเดียวกับ full mode แต่ใส่ค่า padding/border ใหม่:
```
N = periods.length + 1
offsetsTotal = (1+1) × N + 0.5 × (N+1)
miniContentWidth = 400 (allocated per-mini area)
maxSumWidths = miniContentWidth - offsetsTotal - 1 (safety)
periodWidth = (maxSumWidths - dayCol) / periods.length
```

ที่ 14 periods: periodWidth ≈ 24pt → subject_code 5pt ~15pt fit, subject_name truncate fit

### Last page handling
ถ้า `pages.length % 4 !== 0` → หน้าสุดท้ายมี mini เหลือ (1-3 ตาราง) ช่องที่เหลือ blank

## Implementation Files
- `frontend-school/src/lib/utils/pdf.ts` — เพิ่ม layout option, mini-table builder
- `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte` — UI radio + pass option

## Non-goals (YAGNI)
- ไม่ทำ custom grid (1×2, 3×3, etc.) — แค่ binary choice
- ไม่ทำ font-scaling อัตโนมัติตามจำนวนคาบ (จะทำแยกถ้าจำเป็น)
- ไม่แสดง teacher/room ใน mini mode (truncate aggressively เพื่อให้พอที่)
