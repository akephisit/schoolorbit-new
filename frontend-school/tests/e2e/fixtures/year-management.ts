import { expect, type Page, type Route, type Locator } from '@playwright/test';
import { sessionState } from './session-state';
import type { components } from '../../../src/lib/api/generated/school-api';
type S = components['schemas'];
export const id = (number: number) => `10000000-0000-4000-8000-${String(number).padStart(12, '0')}`;
export const yearUrl = `/staff/academic/year-lifecycle?academicYearId=${id(1)}`;
const time = '2026-11-01T00:00:00Z';
export const sourceYear: S['YearLifecycleContext'] = {
	academicYearId: id(1),
	year: 2569,
	name: 'ปีการศึกษา 2569',
	status: 'closed',
	rowVersion: 2
};
export const targetYear: S['YearLifecycleContext'] = {
	academicYearId: id(2),
	year: 2570,
	name: 'ปีการศึกษา 2570',
	status: 'planning',
	rowVersion: 1
};
export function roster(studentCount = 5): S['YearRolloverPreview'] {
	return {
		sourceYear,
		targetYear,
		outcomeEffectiveOn: '2026-11-01',
		targetPlacementStartsOn: '2026-11-02',
		sourceChecksum: 'a'.repeat(64),
		canConfirm: false,
		findings: [],
		grades: [
			{ id: id(10), levelType: 'primary', year: 6, isActive: true },
			{ id: id(11), levelType: 'secondary', year: 1, isActive: true },
			{ id: id(12), levelType: 'secondary', year: 3, isActive: true },
			{ id: id(13), levelType: 'secondary', year: 4, isActive: true }
		],
		programs: [
			{
				id: id(20),
				code: 'P',
				name: 'แผนประถม',
				curriculumLevelId: id(21),
				levelName: 'ประถม',
				editionName: 'หลักสูตร 2569',
				gradeLevelIds: [id(10)]
			},
			{
				id: id(22),
				code: 'S',
				name: 'แผนมัธยม',
				curriculumLevelId: id(23),
				levelName: 'มัธยม',
				editionName: 'หลักสูตร 2569',
				gradeLevelIds: [id(11), id(12), id(13)]
			}
		],
		homerooms: [
			{
				id: id(30),
				name: 'ม.1/1',
				gradeLevelId: id(11),
				studyProgramId: id(22),
				capacity: 40,
				occupiedCount: 0
			}
		],
		students: Array.from({ length: studentCount }, (_, index) => ({
			studentAcademicYearId: id(40 + index),
			studentId: id(50 + index),
			studentCode: `S${index + 1}`,
			studentName:
				['ปอ เรียนต่อ', 'มิว ซ้ำชั้น', 'ดาว จบการศึกษา', 'ฟ้า ย้ายออก', 'กานต์ พักไว้'][index] ??
				`นักเรียน ${index + 1}`,
			sourceGradeLevelId: index === 2 ? id(12) : id(10),
			sourceStudyProgramId: index === 2 ? id(22) : id(20),
			sourceHomeroomId: id(60),
			sourceHomeroomName: index === 2 ? 'ม.3/1' : 'ป.6/1',
			sourceRowVersion: 1,
			existingTargetStudentYearId: null,
			decision: null,
			suggestedDecision: null
		}))
	};
}
export async function fulfill(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-csrf' },
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}
export async function mockYearManagement(
	page: Page,
	options: {
		reader?: boolean;
		active?: boolean;
		conflict?: boolean;
		uncertain?: boolean;
		delayWorkspace?: Promise<void>;
		studentCount?: number;
	} = {}
) {
	const observations = {
		reads: [] as string[],
		previews: [] as S['YearRolloverPreviewInput'][],
		confirms: [] as S['ConfirmYearRolloverInput'][],
		transitions: [] as S['TermTransitionRequest'][],
		policies: [] as S['UpdateAcademicRetentionPolicy'][]
	};
	let currentSource = {
		...sourceYear,
		status: options.active ? ('active' as const) : ('closed' as const)
	};
	const terms: S['YearTermLifecycleState'][] = [
		{
			academicTermId: id(3),
			name: 'ภาคเรียนที่ 1',
			sequence: 1,
			status: options.active ? 'active' : 'closed',
			rowVersion: 1,
			includedInYearResult: true,
			blocksYearClosure: true
		},
		{
			academicTermId: id(4),
			name: 'ภาคเรียนที่ 2',
			sequence: 2,
			status: 'planning',
			rowVersion: 1,
			includedInYearResult: true,
			blocksYearClosure: true
		}
	];
	const policy: S['AcademicRetentionPolicy'] = {
		enabled: false,
		primaryBoundary: 'year',
		secondaryBoundary: 'term',
		graceDays: 90,
		effectiveFrom: null,
		rowVersion: 1,
		updatedBy: null
	};
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const path = new URL(route.request().url()).pathname;
			const method = route.request().method();
			if (method === 'GET') observations.reads.push(path);
			if (path === '/api/auth/me')
				return fulfill(route, {
					id: id(99),
					username: 'year-manager',
					firstName: 'วิชา',
					lastName: 'การ',
					displayName: 'วิชา การ',
					userType: 'staff',
					status: 'active',
					session: sessionState(),
					profileImageFileId: null,
					primaryRoleName: null,
					permissions: options.reader
						? [
								'academic_lifecycle.read.school',
								'academic_promotion.read.school',
								'academic_context.read.school'
							]
						: ['*']
				} satisfies S['CurrentUserResponse']);
			if (path === '/api/academic/context/options')
				return fulfill(route, {
					defaultAcademicYearId: id(1),
					defaultAcademicTermId: null,
					years: [currentSource, targetYear].map((year) => ({
						id: year.academicYearId,
						year: year.year,
						name: year.name,
						isVisible: true,
						status: year.status
					})),
					terms: terms.map((term) => ({
						id: term.academicTermId,
						academicYearId: id(1),
						sequence: term.sequence,
						code: String(term.sequence),
						name: term.name,
						termType: 'regular',
						status: term.status,
						isVisible: true,
						includedInYearResult: true,
						blocksYearClosure: true
					}))
				} satisfies S['AcademicContextOptions']);
			if (path === '/api/academic/setup/workspace')
				return fulfill(route, {
					years: [],
					terms: [],
					bellSchedules: [],
					contextSettings: {
						defaultAcademicYearId: id(1),
						defaultAcademicTermId: null,
						rowVersion: 1
					}
				} satisfies S['AcademicSetupWorkspace']);
			if (path === '/api/academic/lifecycle/year-rollovers/options')
				return fulfill(route, { targetYears: [targetYear] } satisfies S['YearRolloverOptions']);
			if (path === '/api/academic/lifecycle/year-rollovers/preview') {
				const input: S['YearRolloverPreviewInput'] = route.request().postDataJSON();
				observations.previews.push(input);
				const base = roster(options.studentCount ?? 5);
				return fulfill(route, {
					...base,
					sourceYear: currentSource,
					outcomeEffectiveOn: input.outcomeEffectiveOn,
					targetPlacementStartsOn: input.targetPlacementStartsOn,
					students: base.students.map((student) => ({
						...student,
						decision:
							input.decisions.find(
								(row) => row.studentAcademicYearId === student.studentAcademicYearId
							)?.decision ?? null
					})),
					findings:
						currentSource.status === 'closed'
							? []
							: [
									{
										code: 'rollover.source_year_open',
										count: 1,
										severity: 'blocking',
										message: 'ปิดปีต้นทางก่อนยืนยันรายชื่อ',
										resolutionUrl: null
									}
								],
					canConfirm:
						currentSource.status === 'closed' &&
						input.decisions.length === (options.studentCount ?? 5)
				} satisfies S['YearRolloverPreview']);
			}
			if (path === '/api/academic/lifecycle/year-rollovers/confirm') {
				const input: S['ConfirmYearRolloverInput'] = route.request().postDataJSON();
				observations.confirms.push(input);
				if (options.conflict && observations.confirms.length === 1)
					return fulfill(route, 'ข้อมูลเปลี่ยนแล้ว', 409);
				if (options.uncertain && observations.confirms.length === 1) return route.abort('failed');
				return fulfill(route, {
					requestId: input.requestId,
					runId: id(70),
					sourceYearId: id(1),
					targetYearId: id(2),
					prepared: true,
					activated: false,
					studentCount: options.studentCount ?? 5,
					receipts: [],
					completedAt: time
				} satisfies S['YearRolloverOutcome']);
			}
			if (path === '/api/academic/lifecycle/promotion-runs')
				return fulfill(route, { runs: [], nextCursor: null } satisfies S['PromotionRunList']);
			if (path === `/api/academic/lifecycle/years/${id(1)}`) {
				if (options.delayWorkspace) await options.delayWorkspace;
				return fulfill(route, {
					context: currentSource,
					terms,
					findings: [],
					availableActions: currentSource.status === 'active' ? ['close'] : [],
					canClose: true,
					sourceChecksum: 'b'.repeat(64)
				} satisfies S['YearLifecycleWorkspace']);
			}
			if (path === `/api/academic/lifecycle/years/${id(1)}/transitions`) {
				currentSource = { ...sourceYear, status: 'closed' };
				return fulfill(route, {
					requestId: route.request().postDataJSON().requestId,
					action: 'close',
					context: currentSource,
					completedAt: time
				} satisfies S['YearTransitionOutcome']);
			}
			const term = terms.find((candidate) =>
				path.startsWith(`/api/academic/lifecycle/terms/${candidate.academicTermId}`)
			);
			if (term) {
				const context: S['TermLifecycleContext'] = {
					academicTermId: term.academicTermId,
					academicYearId: id(1),
					bellScheduleId: id(80),
					blocksYearClosure: true,
					includedInYearResult: true,
					operationDate: '2026-11-01',
					sequence: term.sequence,
					termName: term.name,
					termRowVersion: 1,
					termStatus: term.status,
					yearName: sourceYear.name,
					yearRowVersion: 2,
					yearStatus: currentSource.status
				};
				if (path.endsWith('/transitions')) {
					const input: S['TermTransitionRequest'] = route.request().postDataJSON();
					observations.transitions.push(input);
					term.status = input.action === 'activate' ? 'active' : 'closed';
					return fulfill(route, {
						requestId: input.requestId,
						action: input.action,
						context: { ...context, termStatus: term.status },
						completedAt: time
					} satisfies S['TermTransitionOutcome']);
				}
				if (path.endsWith('/activation'))
					return fulfill(route, {
						context,
						canActivate: true,
						eligiblePlacements: 0,
						plannedStudents: 0,
						findings: [],
						opensYear: false,
						predecessor: null,
						sourceChecksum: 'c'.repeat(64),
						policy: {
							requireHomeroomPlacements: false,
							requirePublishedOfferings: false,
							requirePublishedTimetable: false,
							rowVersion: 1
						}
					} satisfies S['TermActivationWorkspace']);
				return fulfill(route, {
					context,
					findings: [],
					sourceChecksum: 'd'.repeat(64),
					availableActions: ['close']
				} satisfies S['TermLifecycleWorkspace']);
			}
			if (path.endsWith('/retention'))
				return fulfill(route, {
					policy,
					wholeTermCleanup: true,
					terms: [
						{
							academicTermId: id(3),
							academicYearId: id(1),
							levelTypes: ['primary', 'secondary'],
							requiredBoundary: 'year',
							closedAt: time,
							eligibleAt: null,
							eligible: false,
							blockedReasons: ['policy_disabled', 'outside_policy_effective_period'],
							impact: {
								attendanceRecords: 12,
								evidenceFiles: 2,
								evidenceBytes: 1024,
								behaviorEntries: 3,
								attendanceArchived: false
							},
							preservesAttendanceSummaries: true,
							preservesBehaviorTotals: true,
							job: null
						}
					]
				} satisfies S['AcademicRetentionWorkspace']);
			if (path.endsWith('/retention-policy')) {
				const input: S['UpdateAcademicRetentionPolicy'] = route.request().postDataJSON();
				observations.policies.push(input);
				Object.assign(policy, input, { rowVersion: policy.rowVersion + 1, effectiveFrom: time });
				return fulfill(route, policy);
			}
			if (path === '/api/menu/user') return fulfill(route, { groups: [] });
			return fulfill(route, {});
		}
	);
	return observations;
}
export async function chooseOption(page: Page, control: Locator, name: string) {
	await control.click();
	const contentId = await control.getAttribute('aria-controls');
	if (!contentId) throw new Error('Select content must have an accessible owner');
	await page.locator(`[id="${contentId}"]`).getByRole('option', { name, exact: true }).click();
	await expect(control).toHaveAttribute('aria-expanded', 'false');
}
export async function selectOption(page: Page, label: string, name: string) {
	await chooseOption(page, page.getByLabel(label, { exact: true }), name);
}

export async function openRoster(page: Page) {
	await page.goto(yearUrl);
	await page.getByRole('button', { name: 'ขึ้นปีการศึกษาใหม่', exact: true }).click();
	await selectOption(page, 'ปีปลายทาง', 'ปีการศึกษา 2570');
	for (const [label, day] of [
		['วันสิ้นสุดสถานะปีต้นทาง', 1],
		['วันเริ่มจัดห้องปีใหม่', 2]
	] as const) {
		await page.getByRole('button', { name: label, exact: true }).click();
		await page
			.locator('[data-calendar-day]:not([data-outside-month]):visible')
			.filter({ hasText: new RegExp(`^${day}$`) })
			.click();
		await expect(page.locator('[data-calendar-day]:visible')).toHaveCount(0);
	}
	await page.getByRole('button', { name: 'ดูตัวอย่างรายชื่อนักเรียน', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'ปอ เรียนต่อ S1', exact: true })).toBeVisible();
}
export async function setAllDecisions(page: Page) {
	const dialog = page.getByRole('dialog');
	const students = dialog.locator('section').filter({ has: page.locator('h3') });
	const names = ['ปอ เรียนต่อ', 'มิว ซ้ำชั้น', 'ดาว จบการศึกษา', 'ฟ้า ย้ายออก', 'กานต์ พักไว้'];
	const outcomes = ['เลื่อนชั้น', 'ซ้ำชั้น', 'จบการศึกษา', 'ย้ายออก', 'พักไว้'];
	for (let index = 0; index < names.length; index++) {
		const row = students.filter({ hasText: names[index] });
		await chooseOption(page, row.getByLabel('ผลปีใหม่', { exact: true }), outcomes[index]);
		if (index < 2) {
			if (index === 0) {
				await chooseOption(page, row.getByLabel('ชั้นปลายทาง', { exact: true }), 'ม.1');
			}
			await chooseOption(
				page,
				row.getByLabel('แผนการเรียนปลายทาง', { exact: true }),
				index === 0 ? 'แผนมัธยม · หลักสูตร 2569' : 'แผนประถม · หลักสูตร 2569'
			);
		}
		await row.getByRole('button', { name: 'เสร็จแล้ว', exact: true }).click();
	}
	await dialog.getByRole('button', { name: 'ตรวจตัวอย่างล่าสุด', exact: true }).click();
	await expect(
		dialog.getByRole('button', { name: 'ยืนยันเตรียมรายชื่อนักเรียน', exact: true })
	).toBeEnabled();
}
