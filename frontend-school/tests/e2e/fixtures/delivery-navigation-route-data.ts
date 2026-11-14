import { sessionState } from './session-state.ts';
import type { Page, Route } from '@playwright/test';
import type {
	DeliveryWorkspace,
	HomeroomDeliveryWorkspace,
	LearningOffering,
	CreateLearningOfferingRequest,
	DeliveryManagementOptions
} from '../../../src/lib/api/learning-delivery.js';

const uuid = (number: number) => `10000000-0000-4000-8000-${String(number).padStart(12, '0')}`;
export const navigationIds = {
	activeYear: uuid(1),
	activeTerm: uuid(2),
	year: uuid(3),
	term: uuid(4),
	version: uuid(5),
	roomA: uuid(6),
	roomB: uuid(7),
	grade: uuid(8),
	program: uuid(9),
	curriculum: uuid(10),
	math: uuid(11),
	thai: uuid(12),
	actor: uuid(13),
	sourceVersion: uuid(14),
	changeSet: uuid(15)
};
const definitions = [
	{
		id: uuid(20),
		kind: 'course' as const,
		code: 'ค23101',
		name: 'คณิตศาสตร์ปลาย',
		subjectType: 'BASIC',
		group: navigationIds.math
	},
	{
		id: uuid(21),
		kind: 'activity' as const,
		code: 'ACT-TECHNICAL-CLUB',
		name: 'ชุมนุม',
		subjectType: null,
		group: null
	},
	{
		id: uuid(22),
		kind: 'course' as const,
		code: 'ค20101',
		name: 'คณิตศาสตร์เพิ่มเติม',
		subjectType: 'ADDITIONAL',
		group: navigationIds.math
	},
	{
		id: uuid(23),
		kind: 'course' as const,
		code: 'ท11101',
		name: 'ภาษาไทยเก่า',
		subjectType: 'LEGACY-UNKNOWN',
		group: navigationIds.thai
	},
	{
		id: uuid(24),
		kind: 'course' as const,
		code: 'ค21101',
		name: 'คณิตศาสตร์ต้น',
		subjectType: 'BASIC',
		group: navigationIds.math
	},
	{
		id: uuid(25),
		kind: 'activity' as const,
		code: 'ACT-TECHNICAL-GUIDANCE',
		name: 'แนะแนว',
		subjectType: null,
		group: null
	}
];
export const navigationOfferingIds = {
	basic21: uuid(24),
	basic23: uuid(20),
	additional: uuid(22),
	unknown: uuid(23),
	club: uuid(21),
	guidance: uuid(25)
};

export function navigationDelivery(): DeliveryWorkspace {
	return {
		academicYearId: navigationIds.year,
		academicTermId: navigationIds.term,
		rowVersion: 1,
		courseMetadata: definitions.flatMap((item) => item.kind === 'course' ? [{subjectVersionId: item.id,subjectGroupId: item.group!,subjectGroupName: item.group === navigationIds.math ? 'คณิตศาสตร์' : 'ภาษาไทย'}] : []),
		offerings: definitions.map((item) => ({
			id: item.id, kind: item.kind, code: item.code, name: item.name, rowVersion: 1,
			owningOrganizationUnitId: item.group ?? navigationIds.program,
			weeklyPeriodTarget: 2, groups: [], homeroomIds: [], targets: [],
			catalog: item.kind === 'course' ? {
				kind: 'course' as const, subjectId: item.id,subjectVersionId: item.id,
				standardPeriodsPerWeek: 2, credit: '1.0',hours: '40',assessmentTotalScore: '100.00'
			} : {
				kind: 'activity' as const, activityId: item.id,activityVersionId: item.id,
				registrationType: 'assigned' as const,schedulingMode: 'synchronized' as const,
				hours: '20',attendanceRequirement: {minimumPercent: '80'},
				passCriteria: {requireAttendance:true,requireTeacherConfirmation:true,outcomes:['pass' as const,'fail' as const]}
			}
		}))
	};
}

export function navigationWorkspace(): HomeroomDeliveryWorkspace {
	return {
		academicYearId: navigationIds.year,
		academicTermId: navigationIds.term,
		rowVersion: 1,
		homerooms: [navigationIds.roomA, navigationIds.roomB].map((roomId, index) => ({
			homeroom: { id: roomId, name: `ม.1/${index + 1}` },
			gradeLevel: {
				id: navigationIds.grade,
				name: 'มัธยมศึกษาปีที่ 1',
				code: 'M1',
				short_name: 'ม.1',
				level_type: 'secondary',
				level_order: 301
			},
			studyProgram: {
				id: navigationIds.program,
				code: 'MATH',
				name: 'แผนคณิตศาสตร์',
				editionId: navigationIds.curriculum,
				levelName: 'หลักสูตรมัธยม',
				curriculumLevelId: navigationIds.curriculum,
				displayOrder: 1,
				editionName: 'หลักสูตร 2570',
				gradeLevelIds: [navigationIds.grade]
			},
			curriculumLevelId: navigationIds.curriculum,
			expectedCount: 5,
			readyCount: index === 0 ? 5 : 4,
			blockers: [],
			items: definitions
				.filter((item) => item.id !== navigationOfferingIds.guidance)
				.map((item) => ({
					requirementId: item.id,
					catalogVersionId: item.id,
					resourceKind: item.kind,
					code: item.code,
					name: item.name,
					subjectType: item.subjectType,
					requirementKind:
						item.subjectType === 'BASIC' ? ('elective' as const) : ('required' as const),
					standardPeriodsPerWeek: item.kind === 'course' ? 2 : null,
					weeklyPeriodTarget: 2,
					schedulingMode: item.kind === 'activity' ? ('synchronized' as const) : null,
					offeringId: item.id,
					offeringState: 'present' as const,
					groupMode: 'normal' as const,
					teacherState: 'assigned' as const,
					alignmentStates: ['matches_curriculum' as const],
					groups: []
				})),
			extraOfferings: [
				{
					offeringId: navigationOfferingIds.guidance,
					catalogVersionId: navigationOfferingIds.guidance,
					resourceKind: 'activity' as const,
					code: 'ACT-TECHNICAL-GUIDANCE',
					name: 'แนะแนว',
					startsOn: '2027-05-01',
					endsOn: null,
					weeklyPeriodTarget: 2,
					alignmentStates: ['extra_offering' as const]
				}
			]
		})),
		unlinked: [
			{
				offeringId: navigationOfferingIds.guidance,
				groupId: null,
				resourceKind: 'activity',
				code: 'ACT-TECHNICAL-UNLINKED',
				name: 'แนะแนวที่ยังไม่จัดกลุ่ม',
				reason: 'ยังไม่มีกลุ่มเรียน'
			}
		]
	};
}

export const navigationOverviewPath = () =>
	`/staff/academic/delivery?academicYearId=${navigationIds.year}&academicTermId=${navigationIds.term}`;

export async function mockDeliveryNavigation(
	page: Page,
	options: {
		manager?: boolean;
		continuation?: boolean;
		groupedCreation?: boolean;
		readiness?: 'course' | 'activity';
	} = {}
) {
	const reads: URL[] = [];
	const writes: string[] = [];
	const creations: CreateLearningOfferingRequest[] = [];
	let created = false;
	function offeringResource(id: string): LearningOffering {
		const item = navigationDelivery().offerings.find((offering) => offering.id === id)!;
		return {
			canManage: true,
			id: item.id,
			academicYearId: navigationIds.year,
			academicTermId: navigationIds.term,
			kind: item.kind,
			codeSnapshot: item.code,
			nameSnapshot: item.name,
			rowVersion: 1,
			weeklyPeriodTarget: item.weeklyPeriodTarget,
			targets: [],
			snapshot: item.catalog,
			migrated: false,
			owningOrganizationUnitId: item.owningOrganizationUnitId,
			createdAt: '2027-04-01T00:00:00Z',
			updatedAt: '2027-04-01T00:00:00Z'
		};
	}
	function currentDelivery(): DeliveryWorkspace {
		const workspace = navigationDelivery();
		if (options.groupedCreation && !created) workspace.offerings = workspace.offerings.filter((item) => item.id !== navigationOfferingIds.club);
		return workspace;
	}
	function workspaceResource(): HomeroomDeliveryWorkspace {
		const workspace = navigationWorkspace();
		if (options.groupedCreation && !created) for (const room of workspace.homerooms) {
			const activity = room.items.find((item) => item.offeringId === navigationOfferingIds.club)!;
			Object.assign(activity, {offeringId:null,offeringState:'missing',groupMode:'missing',teacherState:'missing_primary',alignmentStates:['curriculum_requirement_not_offered']});
		}
		return workspace;
	}
	async function reply(route: Route, data: unknown) {
		await route.fulfill({ json: { success: true, data } });
	}
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const request = route.request(),
				url = new URL(request.url()),
				path = url.pathname;
			if (request.method() !== 'GET') writes.push(`${request.method()} ${path}`);
			reads.push(url);
			if (path === '/api/auth/me')
				return reply(route, {
					id: navigationIds.actor,
					username: 'navigation-reader',
					firstName: 'ทดสอบ',
					lastName: 'การเปิดสอน',
					displayName: 'ทดสอบ การเปิดสอน',
					userType: 'staff',
					status: 'ACTIVE',
					session: sessionState(),
					permissions: options.manager ? ['*'] : ['learning_offering.read.school']
				});
			if (path === '/api/menu/user') return reply(route, { groups: [] });
			if (path === '/api/academic/context/options')
				return reply(route, {
					defaultAcademicYearId: navigationIds.activeYear,
					defaultAcademicTermId: navigationIds.activeTerm,
					years: [
						{
							isVisible: true,
							id: navigationIds.activeYear,
							name: 'ปีการศึกษา 2569',
							year: 2569,
							status: 'active'
						},
						{
							isVisible: true,
							id: navigationIds.year,
							name: 'ปีการศึกษา 2570',
							year: 2570,
							status: 'active'
						}
					],
					terms: [
						{
							isVisible: true,
							id: navigationIds.activeTerm,
							academicYearId: navigationIds.activeYear,
							name: 'ภาคเรียนที่ 1',
							code: '1',
							sequence: 1,
							termType: 'regular',
							status: 'active'
						},
						{
							isVisible: true,
							id: navigationIds.term,
							academicYearId: navigationIds.year,
							name: 'ภาคเรียนที่ 2',
							code: '2',
							sequence: 2,
							termType: 'regular',
							status: 'active'
						}
					]
				});
			if (path === '/api/academic/delivery/workspace') return reply(route, currentDelivery());
			if (path === '/api/academic/delivery/homerooms') return reply(route, workspaceResource());
			if (path.includes('delivery-versions') || path.includes('term-change-sets')) return route.fulfill({status:404,json:{success:false,error:'Retired operational endpoint'}});
			if (path === '/api/academic/delivery/management-options')
				return reply(route, {
					academicYearId: navigationIds.year,
					academicTermId: navigationIds.term,
					studyPrograms: [navigationWorkspace().homerooms[0].studyProgram],
					gradeLevels: [navigationWorkspace().homerooms[0].gradeLevel],
					homerooms: navigationWorkspace().homerooms.map((room) => room.homeroom),
					learningGroups: [],
					teachers: [],
					rooms: [],
					catalogVersions: [
						{
							id: navigationOfferingIds.club,
							kind: 'activity',
							subjectType: null,
							code: 'ACT-TECHNICAL-CLUB',
							label: 'ACT-TECHNICAL-CLUB — ชุมนุม (ฉบับ 3)',
							name: 'ชุมนุม',
							versionNo: 3,
							schedulingMode: 'synchronized',
							standardPeriodsPerWeek: null
						}
					] satisfies DeliveryManagementOptions['catalogVersions']
				});
			if (path === '/api/academic/offerings' && request.method() === 'POST') {
				creations.push(request.postDataJSON());
				created = true;
				return reply(route, offeringResource(navigationOfferingIds.club));
			}
			if (/\/offerings\/[^/]+\/groups$/.test(path)) return reply(route, []);
			if (/\/offerings\/[^/]+$/.test(path)) {
				return reply(route, offeringResource(path.split('/').at(-1)!));
			}
			return reply(route, {});
		}
	);
	return {
		reads,
		writes,
		creations,
		count: (path: string) => reads.filter((url) => url.pathname === path).length
	};
}
