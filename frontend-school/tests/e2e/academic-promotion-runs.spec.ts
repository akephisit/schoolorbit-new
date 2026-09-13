import { expect, test, type Page, type Route } from '@playwright/test';
import type { components } from '../../src/lib/api/generated/school-api';

test.use({ serviceWorkers: 'block' });
type Schema = components['schemas'];
const id = (n: number) => `10000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
const time = '2026-09-11T12:00:00Z';
const root = '/api/academic/lifecycle/promotion-runs';
const pageUrl = `/staff/academic/promotion/${id(3)}`;

function fixture(): Schema['PromotionRunWorkspace'] {
	return {
		run: {
			id: id(3),
			sourceYearId: id(1),
			targetYearId: id(2),
			policyId: id(6),
			status: 'calculated',
			rowVersion: 2,
			createdBy: id(20),
			createdAt: time,
			updatedAt: time,
			reviewedBy: null,
			reviewedAt: null,
			approvedBy: null,
			approvedAt: null,
			approvalId: null,
			executedBy: null,
			executedAt: null
		},
		sourceYear: {
			academicYearId: id(1),
			year: 2569,
			name: 'ปีการศึกษา 2569',
			startDate: '2026-05-01',
			endDate: '2027-04-30',
			status: 'active',
			rowVersion: 1
		},
		targetYear: {
			academicYearId: id(2),
			year: 2570,
			name: 'ปีการศึกษา 2570',
			startDate: '2027-05-01',
			endDate: '2028-04-30',
			status: 'planning',
			rowVersion: 1
		},
		approvalChecksum: 'a'.repeat(64),
		students: [
			{
				studentCode: 'E2E-001',
				studentName: 'นักเรียน ทดสอบ',
				annualResultCurrent: true,
				needsRecalculation: false,
				receipt: null,
				item: {
					id: id(5),
					runId: id(3),
					sourceYearId: id(1),
					targetYearId: id(2),
					studentAcademicYearId: id(7),
					studentId: id(4),
					sourceGradeLevelId: id(10),
					sourceStudyProgramId: id(12),
					sourceRowVersion: 1,
					annualRevisionId: id(8),
					sourceChecksum: 'b'.repeat(64),
					recommendation: {
						suggestedOutcome: 'promote',
						targetGradeLevelId: id(11),
						targetStudyProgramId: id(12),
						findings: []
					},
					existingTargetStudentYearId: null,
					decision: null,
					reviewedBy: null,
					reviewedAt: null,
					status: 'calculated',
					rowVersion: 1,
					createdAt: time,
					updatedAt: time
				}
			}
		]
	};
}
async function reply(route: Route, data: unknown, status = 200) {
	await route.fulfill({
		status,
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-csrf' },
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}
async function mock(
	page: Page,
	level: 'reader' | 'manager' | 'approver' | 'executor' | 'corrector' = 'reader',
	conflict = false,
	calculateConflict = false,
	loseExecutionResponse = false,
	impactScenario: 'changed' | 'empty' | 'changed-page' | null = null,
	impactResolveConflict = false
) {
	const current = fixture();
	const writes: Array<{ path: string; body: unknown }> = [];
	const impactReads: string[] = [];
	let impactResolution: Schema['PromotionImpactResolution'] | null = null;
	let completedCommand: {
		input: Schema['ExecutePromotionRunInput'];
		result: Schema['PromotionExecutionResult'];
	} | null = null;
	if (level === 'approver' || level === 'executor') {
		current.run.status = level === 'approver' ? 'reviewed' : 'approved';
		current.run.rowVersion = 3;
		current.students[0].item.status = 'reviewed';
		current.students[0].item.decision = {
			outcome: 'promote',
			targetGradeLevelId: id(11),
			targetStudyProgramId: id(12),
			targetHomeroomId: null,
			reason: null,
			condition: null
		};
		current.students[0].item.reviewedBy = id(20);
		current.students[0].item.reviewedAt = time;
		if (level === 'executor') {
			current.run.approvalId = id(30);
			current.run.approvedBy = id(20);
			current.run.approvedAt = time;
		}
	}
	if (impactScenario) {
		current.run.status = 'completed';
		current.run.approvalId = id(30);
		current.run.approvedBy = id(20);
		current.run.approvedAt = time;
		current.run.executedBy = id(20);
		current.run.executedAt = time;
		current.students[0].item.status = 'executed';
		current.students[0].item.decision = {
			outcome: 'promote',
			targetGradeLevelId: id(11),
			targetStudyProgramId: id(12),
			targetHomeroomId: id(14),
			reason: null,
			condition: null
		};
		current.students[0].receipt = {
			itemId: id(5),
			runId: id(3),
			requestId: id(31),
			approvalId: id(30),
			targetStudentYearId: id(40),
			targetPlacementId: id(41),
			sourceRowVersion: 1,
			executedBy: id(20),
			executedAt: time
		};
		current.students[0].annualResultCurrent = impactScenario === 'empty';
	}
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const path = url.pathname;
			const method = route.request().method();
			if (path === '/api/auth/me')
				return reply(route, {
					id: id(20),
					username: 'E2E-LIFECYCLE-run',
					firstName: 'วิชาการ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'active',
					profileImageFileId: null,
					primaryRoleName: null,
					permissions: [
						'academic_promotion.read.school',
						...(level === 'reader'
							? []
							: level === 'corrector'
								? ['academic_promotion.correct.school']
								: [
										`academic_promotion.${level === 'manager' ? 'manage' : level === 'approver' ? 'approve' : 'execute'}.school`
									])
					]
				} satisfies Schema['CurrentUserResponse']);
			if (path === '/api/academic/context/options')
				return reply(route, {
					activeAcademicYearId: id(1),
					activeAcademicTermId: null,
					terms: [],
					years: [current.sourceYear, current.targetYear].map((year) => ({
						id: year.academicYearId,
						year: year.year,
						name: year.name,
						startDate: year.startDate,
						endDate: year.endDate,
						status: year.status
					}))
				});
			if (path === '/api/academic/lifecycle/promotion-policies/options')
				return reply(route, {
					grades: [
						{ id: id(10), levelType: 'secondary', year: 1, isActive: true },
						{ id: id(11), levelType: 'secondary', year: 2, isActive: true }
					],
					programs: [
						{
							id: id(12),
							code: 'SCI',
							name: 'วิทย์–คณิต',
							curriculumId: id(13),
							curriculumName: 'หลักสูตรสถานศึกษา',
							versionName: '2569',
							status: 'published'
						}
					],
					progressionSet: { rowVersion: 1, progressions: [] }
				});
			if (path === '/api/lookup/homerooms')
				return reply(route, [
					{ id: id(14), name: 'ม.2/1', gradeLevelId: id(11), gradeLevel: 'ม.2' }
				]);
			if (path === '/api/academic/lifecycle/promotion-policies')
				return reply(route, [
					{
						id: id(6),
						name: 'เกณฑ์ที่ยืนยันแล้ว',
						rules: [],
						progressionRowVersion: 1,
						reviewedBy: id(20),
						reviewedAt: time
					}
				]);
			if (path === root && method === 'GET')
				return reply(route, { runs: [current.run], nextCursor: null });
			if (path === `${root}/${id(3)}` && method === 'GET') return reply(route, current);
			if (path === `${root}/${id(3)}/impacts` && method === 'GET') {
				impactReads.push(url.search);
				return reply(route, {
					runId: id(3),
					sourceYearId: id(1),
					targetYearId: id(2),
					totalCount: impactScenario === 'empty' ? 0 : impactScenario === 'changed-page' ? 2 : 1,
					pendingCount:
						impactScenario === 'empty' || impactResolution
							? 0
							: impactScenario === 'changed-page'
								? 2
								: 1,
					nextCursor:
						impactScenario === 'changed-page' && !url.searchParams.has('afterId') ? id(60) : null,
					sourceChecksum: (url.searchParams.has('afterId') ? 'd' : 'c').repeat(64),
					impacts:
						impactScenario === 'empty'
							? []
							: [
									{
										id: id(url.searchParams.has('afterId') ? 61 : 60),
										itemId: id(5),
										studentAcademicYearId: id(7),
										resolution: impactResolution,
										evidence: {
											annualRevisionId: id(8),
											academicTermId: id(9),
											termName: 'ภาคเรียนที่ 2',
											resultId: id(62),
											sourceEffectiveVersion: 1,
											offeringCode: 'ค21101',
											offeringName: 'คณิตศาสตร์พื้นฐาน',
											criterionName: null,
											evaluationDomain: null,
											correction: {
												id: id(63),
												expectedEffectiveVersion: 1,
												previous: { kind: 'course', outcome: 'numeric', numericGrade: '0' },
												corrected: { kind: 'course', outcome: 'numeric', numericGrade: '4' },
												correctedBy: id(20),
												correctedAt: time
											}
										}
									}
								]
				});
			}
			if (path === `${root}/${id(3)}/impacts/${id(60)}/resolve` && method === 'POST') {
				const body: Schema['ResolvePromotionImpactInput'] = route.request().postDataJSON();
				writes.push({ path, body });
				if (impactResolveConflict) {
					impactResolveConflict = false;
					return reply(route, 'ผลแก้ไขเปลี่ยนแล้ว กรุณาโหลดข้อมูลล่าสุด', 409);
				}
				impactResolution = {
					id: id(70),
					requestId: body.requestId,
					runId: id(3),
					itemId: id(5),
					correctionId: id(63),
					impactId: id(60),
					resolutionKind: body.resolutionKind,
					replacementDecision: body.replacementDecision ?? null,
					reason: body.reason,
					sourceChecksum: body.sourceChecksum,
					outcome: {
						adjusted: body.resolutionKind === 'replace_decision',
						targetStudentYearId: id(40),
						targetPlacementId: id(41),
						sourceRowVersion: 2
					},
					resolvedBy: id(20),
					resolvedAt: time
				};
				return reply(route, impactResolution);
			}
			if (path === root && method === 'POST') {
				const body: Schema['CreatePromotionRunInput'] = route.request().postDataJSON();
				writes.push({ path, body });
				return reply(route, { ...current.run, status: 'draft', rowVersion: 1 });
			}
			if (path === `${root}/${id(3)}/calculate`) {
				const body: Schema['CalculatePromotionRunInput'] = route.request().postDataJSON();
				writes.push({ path, body });
				if (calculateConflict) {
					calculateConflict = false;
					current.run.rowVersion++;
					return reply(route, 'มีการแก้ไขรอบ กรุณาโหลดข้อมูลล่าสุด', 409);
				}
				if (body.rowVersion !== current.run.rowVersion)
					return reply(route, 'รุ่นข้อมูลเปลี่ยนแล้ว', 409);
				current.run.rowVersion++;
				return reply(route, { run: current.run, items: current.students.map((row) => row.item) });
			}
			if (path === `${root}/${id(3)}/items/${id(5)}` && method === 'PUT') {
				const body: Schema['ReviewPromotionItemInput'] = route.request().postDataJSON();
				writes.push({ path, body });
				if (conflict) return reply(route, 'ข้อมูลเปลี่ยนแล้ว กรุณาตรวจรายการล่าสุด', 409);
				current.run.status = 'reviewed';
				current.run.rowVersion++;
				const row = current.students[0].item;
				row.decision = body.decision;
				row.status = 'reviewed';
				row.rowVersion++;
				row.reviewedBy = id(20);
				row.reviewedAt = time;
				return reply(route, { run: current.run, item: row });
			}
			if (path === `${root}/${id(3)}/approve`) {
				const body: Schema['ApprovePromotionRunInput'] = route.request().postDataJSON();
				writes.push({ path, body });
				current.run.status = 'approved';
				current.run.rowVersion++;
				current.run.approvalId = body.requestId;
				current.run.approvedBy = id(20);
				current.run.approvedAt = time;
				return reply(route, current.run);
			}
			if (path === `${root}/${id(3)}/execute`) {
				const body: Schema['ExecutePromotionRunInput'] = route.request().postDataJSON();
				writes.push({ path, body });
				if (completedCommand) {
					if (JSON.stringify(body) !== JSON.stringify(completedCommand.input))
						return reply(route, 'คำขอเปลี่ยนแล้ว', 409);
					return reply(route, completedCommand.result);
				}
				current.run.status = 'completed';
				current.run.rowVersion++;
				current.run.executedBy = id(20);
				current.run.executedAt = time;
				const receipt: Schema['PromotionExecutionReceipt'] = {
					itemId: id(5),
					runId: id(3),
					requestId: body.requestId,
					approvalId: current.run.approvalId!,
					targetStudentYearId: id(40),
					targetPlacementId: null,
					sourceRowVersion: 1,
					executedBy: id(20),
					executedAt: time
				};
				current.students[0].receipt = receipt;
				current.students[0].item.status = 'executed';
				const result: Schema['PromotionExecutionResult'] = {
					run: current.run,
					receipts: [receipt],
					failures: [],
					remainingCount: 0,
					holdCount: 0
				};
				completedCommand = { input: body, result: structuredClone(result) };
				if (loseExecutionResponse) return route.abort('failed');
				return reply(route, result);
			}
			if (path === '/api/notifications/stream')
				return route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
			if (path === '/api/school/public')
				return reply(route, { schoolName: 'โรงเรียนทดสอบ', logoFileId: null });
			if (path === '/api/menu/user') return reply(route, { groups: [] });
			if (path === '/api/notifications') return reply(route, { items: [], unread_count: 0 });
			if (path === '/api/me/work-items/counts')
				return reply(route, { open: 0, dueSoon: 0, overdue: 0, submitted: 0, closed: 0, total: 0 });
			return reply(route, 'ไม่เปิดใช้ในชุดทดสอบนี้', 403);
		}
	);
	return { current, writes, impactReads };
}

test('executed promotion corrections are lazy read-only evidence and persist on reload', async ({
	page
}, testInfo) => {
	const observed = await mock(page, 'reader', false, false, false, 'changed');
	await page.goto(pageUrl);
	const inspect = page.getByRole('button', { name: 'ตรวจผลแก้ไขหลังเลื่อนชั้น', exact: true });
	await expect(inspect).toBeVisible();
	expect(observed.impactReads).toHaveLength(0);
	await inspect.click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true })).toBeVisible();
	await expect(dialog.getByText('นักเรียน ทดสอบ', { exact: true })).toBeVisible();
	await expect(dialog.getByLabel('ผลก่อนแก้')).toHaveText('0');
	await expect(dialog.getByLabel('ผลหลังแก้')).toHaveText('4');
	await expect(dialog.getByText(/การเปิดดูหน้านี้ไม่เปลี่ยนชั้นหรือห้อง/)).toBeVisible();
	await expect(dialog.getByRole('button', { name: 'ยืนยันแก้ไข' })).toHaveCount(0);
	await page.reload();
	await inspect.click();
	await expect(dialog.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true })).toBeVisible();
	expect(observed.writes).toHaveLength(0);
	expect(observed.impactReads).toHaveLength(2);
	await page.screenshot({
		path: testInfo.outputPath('promotion-impacts-desktop.png'),
		fullPage: true,
		animations: 'disabled'
	});
});

test('empty promotion impacts can be dismissed on mobile', async ({ page }, testInfo) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await mock(page, 'reader', false, false, false, 'empty');
	await page.goto(pageUrl);
	await page.getByRole('button', { name: 'ตรวจผลแก้ไขหลังเลื่อนชั้น', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByText('ยังไม่พบผลแก้ไขหลังเลื่อนชั้น', { exact: true })).toBeVisible();
	await expect(dialog.getByRole('button', { name: 'Close', exact: true })).toBeVisible();
	await expect(
		dialog.getByRole('button', { name: 'กลับไปรอบเลื่อนชั้น', exact: true })
	).toBeVisible();
	await page.screenshot({
		path: testInfo.outputPath('promotion-impacts-mobile.png'),
		fullPage: true,
		animations: 'disabled'
	});
	await dialog.getByRole('button', { name: 'Close', exact: true }).click();
	await expect(dialog).toHaveCount(0);
});

test('promotion impact pages never merge different evidence revisions', async ({ page }) => {
	await mock(page, 'reader', false, false, false, 'changed-page');
	await page.goto(pageUrl);
	await page.getByRole('button', { name: 'ตรวจผลแก้ไขหลังเลื่อนชั้น', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('button', { name: 'แสดงเพิ่มเติม', exact: true }).click();
	await expect(dialog.getByRole('alert')).toContainText('ข้อมูลเปลี่ยนระหว่างตรวจสอบ');
	await expect(dialog.getByText('ค21101 · คณิตศาสตร์พื้นฐาน', { exact: true })).toHaveCount(1);
	await expect(dialog.getByRole('button', { name: 'แสดงเพิ่มเติม', exact: true })).toBeDisabled();
	await dialog.getByRole('button', { name: 'โหลดข้อมูลล่าสุด', exact: true }).click();
	await expect(dialog.getByRole('alert')).toHaveCount(0);
});

test('authorized corrector explicitly retains an executed decision and sees its history', async ({
	page
}) => {
	const observed = await mock(page, 'corrector', false, false, false, 'changed');
	await page.goto(pageUrl);
	await page.getByRole('button', { name: 'ตรวจผลแก้ไขหลังเลื่อนชั้น', exact: true }).click();
	const evidence = page.getByRole('dialog', { name: 'ผลแก้ไขหลังเลื่อนชั้น' });
	await expect(evidence.getByText('รอจัดการ 1 รายการ', { exact: true })).toBeVisible();
	await evidence.getByRole('button', { name: 'จัดการผลกระทบ', exact: true }).click();
	const resolution = page.getByRole('dialog', { name: 'จัดการผลกระทบหลังแก้ผล' });
	await resolution
		.getByLabel('เหตุผลการจัดการ', { exact: true })
		.fill('ตรวจแล้วไม่กระทบผลเลื่อนชั้นเดิม');
	await resolution.getByRole('button', { name: 'ยืนยันการจัดการ', exact: true }).click();
	await expect(resolution).toHaveCount(0);
	await expect(evidence.getByText('จัดการแล้ว · คงผลเดิม', { exact: true })).toBeVisible();
	await expect(evidence.getByText('รอจัดการ 0 รายการ', { exact: true })).toBeVisible();
	expect(observed.writes).toHaveLength(1);
	expect(observed.writes[0].body).toMatchObject({
		resolutionKind: 'keep_existing',
		reason: 'ตรวจแล้วไม่กระทบผลเลื่อนชั้นเดิม',
		replacementDecision: null,
		sourceChecksum: 'c'.repeat(64)
	});
});

test('corrector refreshes stale evidence then replaces the decision with explicit destination data', async ({
	page
}) => {
	const observed = await mock(page, 'corrector', false, false, false, 'changed', true);
	await page.goto(pageUrl);
	await page.getByRole('button', { name: 'ตรวจผลแก้ไขหลังเลื่อนชั้น', exact: true }).click();
	await page.getByRole('button', { name: 'จัดการผลกระทบ', exact: true }).click();
	const resolution = page.getByRole('dialog', { name: 'จัดการผลกระทบหลังแก้ผล' });
	await resolution.getByLabel('วิธีจัดการ', { exact: true }).click();
	await page.getByRole('option', { name: 'ปรับผลเลื่อนชั้นใหม่', exact: true }).click();
	await resolution.getByLabel('เหตุผลการจัดการ', { exact: true }).fill('ปรับตามผลรายปีล่าสุด');
	await resolution.getByRole('button', { name: 'ยืนยันการจัดการ', exact: true }).click();
	await expect(resolution.getByRole('alert')).toContainText('ผลแก้ไขเปลี่ยนแล้ว');
	await resolution.getByRole('button', { name: 'โหลดหลักฐานล่าสุด', exact: true }).click();
	await expect(resolution.getByRole('alert')).toHaveCount(0);
	await resolution.getByRole('button', { name: 'ยืนยันการจัดการ', exact: true }).click();
	await expect(resolution).toHaveCount(0);
	expect(observed.writes).toHaveLength(2);
	expect(observed.writes[1].body).toMatchObject({
		resolutionKind: 'replace_decision',
		reason: 'ปรับตามผลรายปีล่าสุด',
		replacementDecision: {
			outcome: 'promote',
			targetGradeLevelId: id(11),
			targetStudyProgramId: id(12),
			targetHomeroomId: id(14),
			reason: 'ปรับตามผลรายปีล่าสุด'
		}
	});
});

test('promotion reader sees source and destination ledger without mutation controls', async ({
	page
}, testInfo) => {
	const observed = await mock(page);
	await page.goto(pageUrl);
	await expect(page.getByRole('heading', { name: 'ตรวจรอบเลื่อนชั้น', exact: true })).toBeVisible();
	await expect(page.getByText('นักเรียน ทดสอบ', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'พิจารณา', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'อนุมัติรอบ', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'ดำเนินการเลื่อนชั้น', exact: true })).toHaveCount(
		0
	);
	expect(observed.writes).toHaveLength(0);
	await page.screenshot({
		path: testInfo.outputPath('promotion-ledger.png'),
		fullPage: true,
		animations: 'disabled'
	});
});

test('manager reviews a recommendation explicitly without approving or executing', async ({
	page
}) => {
	const observed = await mock(page, 'manager');
	await page.goto(pageUrl);
	await page.getByRole('button', { name: 'พิจารณา', exact: true }).click();
	await expect(page.getByRole('dialog')).toBeVisible();
	await page.getByRole('button', { name: 'บันทึกผลพิจารณา', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	expect(observed.writes).toHaveLength(1);
	expect(observed.writes[0].body).toMatchObject({
		rowVersion: 1,
		decision: { outcome: 'promote', targetGradeLevelId: id(11), targetStudyProgramId: id(12) }
	});
	await expect(page.getByRole('button', { name: 'อนุมัติรอบ', exact: true })).toHaveCount(0);
});

test('conflicting review preserves the teachers draft for inspection', async ({ page }) => {
	await mock(page, 'manager', true);
	await page.goto(pageUrl);
	await page.getByRole('button', { name: 'พิจารณา', exact: true }).click();
	await page.getByLabel('เหตุผล', { exact: true }).fill('ตรวจทานกับทะเบียนแล้ว');
	await page.getByRole('button', { name: 'บันทึกผลพิจารณา', exact: true }).click();
	await expect(page.getByRole('dialog').getByRole('alert')).toBeVisible();
	await expect(page.getByLabel('เหตุผล', { exact: true })).toHaveValue('ตรวจทานกับทะเบียนแล้ว');
	await page.getByRole('button', { name: 'ปิด', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('approver confirms the whole reviewed checksum without student edits', async ({ page }) => {
	const observed = await mock(page, 'approver');
	await page.goto(pageUrl);
	await page.getByRole('button', { name: 'อนุมัติรอบ', exact: true }).click();
	expect(observed.writes).toHaveLength(0);
	await page.getByRole('dialog').getByRole('button', { name: 'ยืนยัน', exact: true }).click();
	await expect(page.getByText('อนุมัติแล้ว', { exact: true })).toBeVisible();
	expect(observed.writes[0].body).toMatchObject({ rowVersion: 3, sourceChecksum: 'a'.repeat(64) });
	await expect(page.getByRole('button', { name: 'พิจารณา', exact: true })).toHaveCount(0);
});

test('executor uses approved batch then reloads canonical receipts', async ({ page }) => {
	const observed = await mock(page, 'executor');
	await page.goto(pageUrl);
	await page.getByRole('button', { name: 'ดำเนินการเลื่อนชั้น', exact: true }).click();
	expect(observed.writes).toHaveLength(0);
	await page.getByRole('dialog').getByRole('button', { name: 'ยืนยัน', exact: true }).click();
	await expect(page.getByText('ดำเนินการครบแล้ว', { exact: true })).toBeVisible();
	expect(observed.writes[0].body).toMatchObject({ rowVersion: 3, limit: 100 });
	await page.reload();
	await expect(page.getByText('ดำเนินการแล้ว', { exact: true })).toBeVisible();
	await expect(
		page.getByRole('button', { name: 'ดำเนินการเลื่อนชั้น', exact: true })
	).toBeDisabled();
	expect(observed.writes).toHaveLength(1);
});

test('mobile details retain a visible close control', async ({ page }, testInfo) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await mock(page);
	await page.goto(pageUrl);
	await page.getByRole('button', { name: 'รายละเอียด', exact: true }).click();
	await expect(
		page.getByRole('dialog').getByRole('button', { name: 'Close', exact: true })
	).toBeVisible();
	await page.screenshot({
		path: testInfo.outputPath('promotion-mobile-details.png'),
		fullPage: true,
		animations: 'disabled'
	});
	await page.getByRole('dialog').getByRole('button', { name: 'Close', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
});

for (const [outcome, label] of [
	['repeat', 'ซ้ำชั้น'],
	['graduate', 'จบการศึกษา'],
	['transfer_out', 'ย้ายออก'],
	['hold', 'พักรายการ'],
	['conditional', 'มีเงื่อนไข']
] as const) {
	test(`manager records explicit ${outcome} with applicable fields`, async ({ page }) => {
		const observed = await mock(page, 'manager');
		await page.goto(pageUrl);
		await page.getByRole('button', { name: 'พิจารณา', exact: true }).click();
		await page.getByLabel('ผลพิจารณา', { exact: true }).click();
		await page.getByRole('option', { name: label, exact: true }).click();
		await page.getByLabel('เหตุผล', { exact: true }).fill('ผ่านการพิจารณาของฝ่ายวิชาการ');
		if (outcome === 'conditional') {
			await expect(
				page.getByRole('button', { name: 'บันทึกผลพิจารณา', exact: true })
			).toBeDisabled();
			await page
				.getByLabel('เงื่อนไขที่ต้องติดตาม', { exact: true })
				.fill('ส่งผลแก้รายวิชาที่ค้าง');
		}
		await page.getByRole('button', { name: 'บันทึกผลพิจารณา', exact: true }).click();
		await expect(page.getByRole('dialog')).toHaveCount(0);
		expect(observed.writes[0].body).toMatchObject({
			decision: {
				outcome,
				targetGradeLevelId:
					outcome === 'repeat' ? id(10) : outcome === 'conditional' ? id(11) : null,
				targetStudyProgramId: ['repeat', 'conditional'].includes(outcome) ? id(12) : null
			}
		});
	});
}

test('list creates a run only for the chosen source and future planning year', async ({ page }) => {
	const observed = await mock(page, 'manager');
	await page.goto(`/staff/academic/promotion?academicYearId=${id(1)}`);
	await page.getByRole('button', { name: 'สร้างรอบเลื่อนชั้น', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel('ปีการศึกษาปลายทาง', { exact: true }).click();
	await page.getByRole('option', { name: 'ปีการศึกษา 2570', exact: true }).click();
	await dialog.getByRole('button', { name: 'สร้างรอบ', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`/promotion/${id(3)}$`));
	expect(observed.writes[0].body).toMatchObject({
		sourceYearId: id(1),
		targetYearId: id(2),
		policyId: id(6)
	});
});

test('manual refresh lets a manager recalculate with a new version after conflict', async ({
	page
}) => {
	const observed = await mock(page, 'manager', false, true);
	await page.goto(pageUrl);
	await page.getByRole('button', { name: 'คำนวณข้อเสนอ', exact: true }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'ยืนยัน', exact: true }).click();
	await expect(page.getByRole('alert')).toContainText('มีการแก้ไขรอบ');
	await page.getByRole('button', { name: 'โหลดข้อมูลรอบใหม่', exact: true }).click();
	await expect(page.getByRole('button', { name: 'คำนวณข้อเสนอ', exact: true })).toBeEnabled();
	await page.getByRole('button', { name: 'คำนวณข้อเสนอ', exact: true }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'ยืนยัน', exact: true }).click();
	await expect(page.getByRole('button', { name: 'คำนวณข้อเสนอ', exact: true })).toBeEnabled();
	await expect(page.getByRole('alert')).toHaveCount(0);
	expect(observed.writes.map((write) => write.body)).toEqual([
		expect.objectContaining({ rowVersion: 2 }),
		expect.objectContaining({ rowVersion: 3 })
	]);
});

test('uncertain execution retry retains the original request ID and does not repeat writes', async ({
	page
}) => {
	const observed = await mock(page, 'executor', false, false, true);
	await page.goto(pageUrl);
	await page.getByRole('button', { name: 'ดำเนินการเลื่อนชั้น', exact: true }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'ยืนยัน', exact: true }).click();
	await expect(page.getByRole('alert')).toBeVisible();
	await page.getByRole('button', { name: 'ดำเนินการเลื่อนชั้น', exact: true }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'ยืนยัน', exact: true }).click();
	await expect(page.getByText('ดำเนินการครบแล้ว', { exact: true })).toBeVisible();
	expect(observed.writes).toHaveLength(2);
	expect(observed.writes[1].body).toEqual(observed.writes[0].body);
	expect(observed.current.run.rowVersion).toBe(4);
});
