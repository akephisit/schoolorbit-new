export type RubricItemType = 'rating' | 'text';

export interface RubricFormItem {
	localId: string;
	label: string;
	description: string;
	itemType: RubricItemType;
	required: boolean;
	sortOrder: number;
}

export interface RubricFormSection {
	localId: string;
	title: string;
	description: string;
	sortOrder: number;
	items: RubricFormItem[];
}

export interface RubricResponseDraft {
	ratingScore: string;
	textResponse: string;
}

export interface RubricDraftSummary {
	ratingItemCount: number;
	answeredRatingCount: number;
	totalScore: number;
	maxScore: number;
	percentage: number | null;
	qualityLabel: string;
}

export interface RubricSectionProgress {
	requiredCount: number;
	answeredRequiredCount: number;
	ratingCount: number;
	answeredRatingCount: number;
}

const paperRubricSections: Array<{
	title: string;
	items: string[];
}> = [
	{
		title: '1. ลักษณะการปฏิบัติงาน',
		items: [
			'1.1 การตรงต่อเวลา',
			'1.2 การควบคุมความเป็นระเบียบในชั้นเรียน',
			'1.3 การรักษาความสะอาดในชั้นเรียน'
		]
	},
	{
		title: '2. บุคลิกภาพ',
		items: [
			'2.1 การแต่งกายสุภาพ เหมาะสม',
			'2.2 การใช้น้ำเสียงมีความชัดเจน',
			'2.3 ความเชื่อมั่นในตนเอง',
			'2.4 การใช้ภาษาสื่อสารและสร้างบรรยากาศการเรียนรู้'
		]
	},
	{
		title: '3. การจัดกิจกรรมการเรียนรู้',
		items: [
			'3.1 แจ้งจุดประสงค์การเรียนรู้รายชั่วโมง',
			'3.2 เนื้อหาสอดคล้องกับจุดประสงค์การเรียนรู้/ผลการเรียนรู้',
			'3.3 จัดกิจกรรมการเรียนรู้อย่างเป็นลำดับขั้นตอนตามแผนการจัดการเรียนรู้',
			'3.4 มีกิจกรรมการเรียนรู้ด้วยวิธีการที่หลากหลาย',
			'3.5 มีการตั้งคำถามที่กระตุ้นให้ผู้เรียนใช้กระบวนการคิด และร่วมแสดงความคิดเห็น',
			'3.6 ใช้สื่อที่สอดคล้องและเหมาะสมกับสาระการเรียนรู้',
			'3.7 มีการสอดแทรกคุณลักษณะอันพึงประสงค์ คุณธรรมจริยธรรม และความรู้ทั่วไป',
			'3.8 จัดบรรยากาศการเรียนรู้ที่ดึงดูดความสนใจ ก่อให้เกิดความกระตือรือร้น',
			'3.9 มีการให้การเสริมแรงเชิงบวกในชั้นเรียน',
			'3.10 มีการสรุปเนื้อหาได้ตรงตามจุดประสงค์การเรียนรู้',
			'3.11 การชี้แนะการเรียนรู้/การศึกษาค้นคว้า และแหล่งเรียนรู้ค้นคว้าเพิ่มเติม'
		]
	},
	{
		title: '4. การวัดและประเมินผล',
		items: ['4.1 สอดคล้องและครอบคลุมจุดประสงค์การเรียนรู้', '4.2 การประเมินหลากหลายวิธี']
	}
];

export function createPaperSupervisionRubricSections(): RubricFormSection[] {
	const ratingSections = paperRubricSections.map((section, sectionIndex) => ({
		localId: `paper-section-${sectionIndex + 1}`,
		title: section.title,
		description: '',
		sortOrder: sectionIndex + 1,
		items: section.items.map((label, itemIndex) => ({
			localId: `paper-section-${sectionIndex + 1}-item-${itemIndex + 1}`,
			label,
			description: '',
			itemType: 'rating' as const,
			required: true,
			sortOrder: itemIndex + 1
		}))
	}));

	return [
		...ratingSections,
		{
			localId: 'paper-section-comments',
			title: 'ความคิดเห็นและข้อเสนอแนะ',
			description: '',
			sortOrder: ratingSections.length + 1,
			items: [
				{
					localId: 'paper-comments-item-1',
					label: 'ความคิดเห็นและข้อเสนอแนะ',
					description: '',
					itemType: 'text',
					required: false,
					sortOrder: 1
				}
			]
		}
	];
}

export function createBlankRubricSection(sortOrder: number): RubricFormSection {
	return {
		localId: `section-${sortOrder}-${crypto.randomUUID()}`,
		title: `หมวดที่ ${sortOrder}`,
		description: '',
		sortOrder,
		items: [createBlankRubricItem('rating', 1)]
	};
}

export function createBlankRubricItem(itemType: RubricItemType, sortOrder: number): RubricFormItem {
	return {
		localId: `item-${sortOrder}-${crypto.randomUUID()}`,
		label: itemType === 'rating' ? `หัวข้อประเมิน ${sortOrder}` : 'ข้อเสนอแนะ',
		description: '',
		itemType,
		required: itemType === 'rating',
		sortOrder
	};
}

export function qualityLevelFromPercentage(percentage: number | null | undefined): string {
	if (percentage === null || percentage === undefined || Number.isNaN(percentage)) return '-';
	if (percentage >= 90) return 'ดีมาก';
	if (percentage >= 80) return 'ดี';
	if (percentage >= 70) return 'พอใช้';
	if (percentage >= 60) return 'ควรปรับปรุง';
	return 'ไม่ผ่าน';
}

export function calculateRubricDraftSummary(
	sections: RubricFormSection[],
	drafts: Record<string, RubricResponseDraft>,
	ratingMax: number
): RubricDraftSummary {
	const ratingItems = sections.flatMap((section) =>
		section.items.filter((item) => item.itemType === 'rating')
	);
	const totalScore = ratingItems.reduce((sum, item) => {
		const score = Number(drafts[item.localId]?.ratingScore || 0);
		return sum + (Number.isFinite(score) ? score : 0);
	}, 0);
	const answeredRatingCount = ratingItems.filter((item) =>
		Boolean(drafts[item.localId]?.ratingScore)
	).length;
	const maxScore = ratingItems.length * ratingMax;
	const percentage = maxScore > 0 ? Math.round((totalScore / maxScore) * 10000) / 100 : null;

	return {
		ratingItemCount: ratingItems.length,
		answeredRatingCount,
		totalScore,
		maxScore,
		percentage,
		qualityLabel: qualityLevelFromPercentage(percentage)
	};
}

export function sectionRubricProgress(
	section: RubricFormSection,
	drafts: Record<string, RubricResponseDraft>
): RubricSectionProgress {
	const requiredItems = section.items.filter((item) => item.required);
	const ratingItems = section.items.filter((item) => item.itemType === 'rating');
	const isAnswered = (item: RubricFormItem) => {
		const draft = drafts[item.localId];
		if (!draft) return false;
		return item.itemType === 'rating'
			? Boolean(draft.ratingScore)
			: Boolean(draft.textResponse.trim());
	};

	return {
		requiredCount: requiredItems.length,
		answeredRequiredCount: requiredItems.filter(isAnswered).length,
		ratingCount: ratingItems.length,
		answeredRatingCount: ratingItems.filter(isAnswered).length
	};
}
