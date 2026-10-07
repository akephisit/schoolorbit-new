export const ids = {
	year: '12000000-0000-4000-8000-000000000401',
	futureYear: '12000000-0000-4000-8000-000000000402',
	term: '22000000-0000-4000-8000-000000000401',
	deliveryVersion: '32000000-0000-4000-8000-000000000401',
	curriculum: '42000000-0000-4000-8000-000000000401',
	curriculumVersion: '52000000-0000-4000-8000-000000000401',
	clonedVersion: '52000000-0000-4000-8000-000000000402',
	program: '62000000-0000-4000-8000-000000000401',
	grade: '72000000-0000-4000-8000-000000000401',
	homeroom: '82000000-0000-4000-8000-000000000401',
	requirement: '92000000-0000-4000-8000-000000000401',
	catalogVersion: 'a2000000-0000-4000-8000-000000000401',
	offering: 'b2000000-0000-4000-8000-000000000401',
	extraOffering: 'b2000000-0000-4000-8000-000000000402',
	group: 'c2000000-0000-4000-8000-000000000401',
	user: 'd2000000-0000-4000-8000-000000000401'
};

export function edition(id = ids.curriculum, status: 'draft' | 'published' = 'published') {
	return {
		id,
		publicationCount: status === 'draft' ? 0 : 1,
		currentPublicationId: status === 'draft' ? null : 'f2000000-0000-4000-8000-000000000401',
		draftId: status === 'draft' ? 'f2000000-0000-4000-8000-000000000402' : null,
		name: `ฉบับปรับปรุง พุทธศักราช ${status === 'draft' ? 2570 : 2569}`,
		revisionYear: status === 'draft' ? 2570 : 2569,
		description: null,
		status,
		isActive: true,
		rowVersion: 4,
		migrated: false,
		publishedAt: status === 'published' ? '2026-05-01T00:00:00Z' : null,
		createdAt: '2026-04-01T00:00:00Z',
		updatedAt: '2026-08-30T00:00:00Z'
	};
}
export function curriculumVersion(id: string, status: 'draft' | 'published' = 'published') {
	return {
		id,
		editionId: status === 'draft' ? ids.futureYear : ids.curriculum,
		editionName: edition(ids.curriculum, status).name,
		draftId: status === 'draft' ? 'f2000000-0000-4000-8000-000000000402' : null,
		publicationId: status === 'draft' ? null : 'f2000000-0000-4000-8000-000000000401',
		revisionYear: status === 'draft' ? 2570 : 2569,
		code: 'LEVEL-M1',
		nameTh: 'ระดับมัธยมศึกษาตอนต้น',
		nameEn: null,
		gradeLevelIds: [ids.grade],
		description: null,
		status,
		isActive: true,
		rowVersion: 4,
		migrated: false,
		createdAt: '2026-04-01T00:00:00Z',
		updatedAt: '2026-08-30T00:00:00Z'
	};
}

export function curriculumStructure(version = curriculumVersion(ids.curriculumVersion)) {
	return {
		level: version,
		rowVersion: version.rowVersion,
		gradeLevels: [
			{
				id: ids.grade,
				code: 'M1',
				name: 'มัธยมศึกษาปีที่ 1',
				short_name: 'ม.1',
				level_type: 'secondary',
				level_order: 301
			}
		],
		termSlots: [
			{
				id: 'e2000000-0000-4000-8000-000000000401',
				curriculumLevelId: version.id,
				sequence: 1,
				name: 'ภาคเรียนที่ 1',
				termType: 'regular',
				typeOccurrence: 1,
				rowVersion: 1
			}
		],
		programs: [
			{
				id: ids.program,
				curriculumLevelId: version.id,
				code: 'DEFAULT',
				nameTh: 'แผนการเรียนพื้นฐาน',
				nameEn: null,
				isDefault: true,
				status: version.status,
				rowVersion: 1,
				createdAt: '2026-04-01T00:00:00Z',
				updatedAt: '2026-08-30T00:00:00Z'
			}
		],
		requirements: [
			{
				id: ids.requirement,
				studyProgramId: ids.program,
				gradeLevel: {
					id: ids.grade,
					code: 'M1',
					name: 'มัธยมศึกษาปีที่ 1',
					short_name: 'ม.1',
					level_type: 'secondary',
					level_order: 301
				},
				termSlotId: 'e2000000-0000-4000-8000-000000000401',
				resourceKind: 'course',
				catalogVersionId: ids.catalogVersion,
				code: 'ค21101',
				name: 'คณิตศาสตร์พื้นฐาน',
				requirementKind: 'required',
				section: 'basic_course',
				metrics: {
					credit: '0.50',
					totalHours: '20',
					weeklyUnit: 'periods_per_week',
					weeklyValue: '1'
				},
				displayOrder: 1
			}
		],
		validation: { blockers: [], warnings: [] }
	};
}
