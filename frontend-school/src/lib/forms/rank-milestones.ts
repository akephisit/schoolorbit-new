import type { components } from '#lib/api/generated/school-api.js';
export const RANK_MILESTONE_LABELS = {
	future: 'ยังไม่ครบระยะเวลาตามเกณฑ์ปกติ',
	due_soon: 'ใกล้ครบระยะเวลาใน 90 วัน',
	time_reached_pending_review: 'ครบระยะเวลาแล้ว · รอตรวจคุณสมบัติ',
	incomplete: 'ข้อมูลยังไม่ครบสำหรับคำนวณ',
	unsupported: 'ยังไม่รองรับเกณฑ์ของบุคลากรกลุ่มนี้',
	no_next_rank: 'ไม่มีวิทยฐานะลำดับถัดไป'
} satisfies Record<components['schemas']['RankMilestoneStatus'], string>;
