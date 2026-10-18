import { expect, test } from '@playwright/test';
import { PERMISSIONS } from '../../src/lib/permissions/registry';
import {
	mockStudentCare,
	carePath,
	careAssignmentId,
	careVersionId,
	careRoundId,
	careCaseId,
	careVisitId,
	careDefinition
} from './fixtures/student-care-route-data';
import { actor } from './fixtures/staff-home-route-data';

test('routine reader sees independent assessment region when overview fails and retries only the failed region', async ({
	page
}) => {
	const api = await mockStudentCare(page, {
		permissions: [PERMISSIONS.STUDENT_CARE_READ_SCHOOL],
		failOverview: true
	});
	await page.goto(carePath());
	await expect(page.getByText('แบบทดสอบสมมติ', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'ลองใหม่: ภาพรวม' })).toBeVisible();
	expect(api.reads.filter((path) => path.endsWith('/assignments'))).toHaveLength(1);
	await page.getByRole('button', { name: 'ลองใหม่: ภาพรวม' }).click();
	await expect(page.getByText('รอส่งแบบประเมิน', { exact: true })).toBeVisible();
	expect(api.reads.filter((path) => path.endsWith('/assignments'))).toHaveLength(1);
	await expect(page.getByRole('link', { name: 'การดูแลรายกรณี', exact: true })).toHaveCount(0);
	expect(
		api.reads.some(
			(path) => path.endsWith('/staff') || path.endsWith('/students') || path.endsWith('/cases')
		)
	).toBe(false);
	await page.screenshot({ path: '/tmp/schoolorbit-student-care-overview.png', fullPage: true });
});
test('sensitive-only counselor enters overview and server-filtered due queue without routine lookups', async ({
	page
}) => {
	const api = await mockStudentCare(page, {
		permissions: [PERMISSIONS.STUDENT_CARE_SENSITIVE_READ_SCHOOL]
	});
	await page.goto(carePath());
	await expect(
		page.getByRole('heading', { name: 'ดูแลช่วยเหลือนักเรียน', exact: true })
	).toBeVisible();
	await page.getByRole('link', { name: 'งานติดตามถึงกำหนด' }).click();
	await expect(page.getByText('การดูแลสมมติ', { exact: true })).toBeVisible();
	expect(api.queries.some((query) => new URLSearchParams(query).has('followUpBefore'))).toBe(true);
	expect(api.reads.some((path) => path.endsWith('/staff') || path.endsWith('/students'))).toBe(
		false
	);
});
test('library import validates typed content and creates an immutable version without status controls', async ({
	page
}) => {
	const api = await mockStudentCare(page);
	await page.goto(carePath('/library/new'));
	await page.getByText('นำเข้า / ส่งออกการตั้งค่า', { exact: true }).click();
	await page
		.getByLabel('ข้อมูลการตั้งค่า JSON', { exact: true })
		.fill(JSON.stringify({ ...careDefinition, status: 'ready' }));
	await page.getByRole('button', { name: 'นำเข้าการตั้งค่า', exact: true }).click();
	await expect(page.getByRole('alert').filter({ hasText: 'บันทึกแบบประเมินไม่ได้' })).toBeVisible();
	await page
		.getByLabel('ข้อมูลการตั้งค่า JSON', { exact: true })
		.fill(JSON.stringify(careDefinition));
	await page.getByRole('button', { name: 'นำเข้าการตั้งค่า', exact: true }).click();
	await expect(page.getByLabel('ชื่อแบบประเมิน *', { exact: true })).toHaveValue(
		careDefinition.name
	);
	await page.getByRole('button', { name: 'บันทึกรุ่นใหม่', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`/library/${careVersionId}`));
	expect(api.versions).toHaveLength(1);
	expect(api.versions[0].definition).toEqual(careDefinition);
	expect('status' in api.versions[0].definition).toBe(false);
	await page.screenshot({ path: '/tmp/schoolorbit-student-care-library.png', fullPage: true });
});
test('round creation pins selected version and assignment names reviewers explicitly', async ({
	page
}) => {
	const api = await mockStudentCare(page);
	await page.goto(carePath('/rounds/new'));
	await page.getByLabel('ชื่อรอบ *', { exact: true }).fill('รอบใหม่สมมติ');
	await page
		.getByLabel('วัตถุประสงค์และการใช้ข้อมูล *', { exact: true })
		.fill('วัตถุประสงค์สมมติสำหรับการทดสอบ');
	await page.getByRole('checkbox', { name: /แบบทดสอบสมมติ/ }).check();
	await page.getByRole('button', { name: 'สร้างรอบประเมิน', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`/rounds/${careRoundId}`));
	expect(api.rounds[0].instrumentVersionIds).toEqual([careVersionId]);
	await page.getByRole('link', { name: 'มอบหมายผู้ตอบ', exact: true }).click();
	await page.getByRole('button', { name: 'รุ่นแบบประเมิน', exact: true }).click();
	await page.getByRole('option', { name: /แบบทดสอบสมมติ/ }).click();
	await page.getByRole('checkbox', { name: /นักเรียนทดสอบ หนึ่ง/ }).check();
	await page.getByRole('checkbox', { name: 'ครูผู้รับผิดชอบ', exact: true }).check();
	await page.getByRole('button', { name: 'มอบหมายแบบประเมิน', exact: true }).click();
	await expect(page.getByText('มอบหมายแล้ว 1 รายการ', { exact: true })).toBeVisible();
	expect(api.assignments[0].reviewerUserIds).toEqual([actor]);
	expect(api.assignments[0].targets).toEqual([{ studentId: actor, respondentId: actor }]);
});
test('server draft failure retains answers and reuses receipt identity; reopen and submit freezes response', async ({
	page
}) => {
	const api = await mockStudentCare(page, {
		failDraft: true,
		userType: 'student',
		permissions: [PERMISSIONS.STUDENT_CARE_READ_OWN]
	});
	await page.goto(`/student-care/assignments/${careAssignmentId}`);
	await page.getByRole('checkbox', { name: 'อ่านและรับทราบวัตถุประสงค์การประเมินแล้ว' }).check();
	await page.getByRole('radio', { name: 'ตัวเลือกสมมติ ข', exact: true }).check();
	await page.getByRole('button', { name: 'บันทึกร่าง', exact: true }).click();
	await expect(
		page.getByText('บันทึกชั่วคราวไม่ได้ กรุณาลองอีกครั้ง', { exact: true })
	).toBeVisible();
	await expect(page.getByRole('radio', { name: 'ตัวเลือกสมมติ ข', exact: true })).toBeChecked();
	await page.getByRole('button', { name: 'บันทึกร่าง', exact: true }).click();
	await expect(page.getByText('บันทึกร่างบนเซิร์ฟเวอร์แล้ว', { exact: true })).toBeVisible();
	expect(api.responses[0].requestId).toBe(api.responses[1].requestId);
	expect(api.responses[0].answers).toHaveLength(1);
	await page.reload();
	await expect(page.getByRole('radio', { name: 'ตัวเลือกสมมติ ข', exact: true })).toBeChecked();
	await page.getByRole('radio', { name: 'คำตอบสมมติ หนึ่ง', exact: true }).check();
	await page.getByRole('button', { name: 'ตรวจและส่งแบบประเมิน', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันส่งแบบประเมิน', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'คำตอบที่ส่งแล้ว', exact: true })).toBeVisible();
	await expect(page.getByText('ตัวเลือกสมมติ ข', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'บันทึกร่าง', exact: true })).toHaveCount(0);
	expect(api.responses.at(-1)?.acknowledgePurpose).toBe(true);
});
test('own inbox requests current respondent and paper assessments cannot be self-scored by guardians', async ({
	page
}) => {
	const api = await mockStudentCare(page, {
		manual: true,
		userType: 'parent',
		permissions: [PERMISSIONS.STUDENT_CARE_READ_OWN]
	});
	await page.goto('/parent/student-care');
	await page.getByRole('link', { name: 'เปิดแบบประเมิน', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'แบบประเมินกระดาษ', exact: true })).toBeVisible();
	await expect(page.getByLabel('มาตรสมมติ', { exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'บันทึกร่าง', exact: true })).toHaveCount(0);
	expect(
		api.queries.some((query) => new URLSearchParams(query).get('respondentId') === actor)
	).toBe(true);
	expect(api.responses).toHaveLength(0);
});
test('named sensitive staff records paper provenance and retains explicit zero score', async ({
	page
}) => {
	const api = await mockStudentCare(page, { manual: true });
	await page.goto(`/student-care/assignments/${careAssignmentId}`);
	await page.getByRole('checkbox', { name: 'อ่านและรับทราบวัตถุประสงค์การประเมินแล้ว' }).check();
	await page.getByLabel('มาตรสมมติ', { exact: true }).fill('0');
	await page
		.getByLabel('ที่มา / เอกสารอ้างอิงคะแนน *', { exact: true })
		.fill('แบบกระดาษสมมติหมายเลข 1');
	await page.getByRole('button', { name: 'ตรวจและส่งแบบประเมิน', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันส่งแบบประเมิน', exact: true }).click();
	await expect(page.getByText('แบบกระดาษสมมติหมายเลข 1', { exact: true })).toBeVisible();
	expect(api.responses[0].manualScores).toEqual([{ scaleKey: 's', score: 0 }]);
	expect(api.responses[0].answers).toEqual([]);
});
for (const theme of ['light', 'dark'])
	test(`mobile visit planning and factual record are usable in ${theme}`, async ({ page }) => {
		await page.setViewportSize({ width: 375, height: 812 });
		await page.addInitScript(
			(value) => localStorage.setItem('ui-preferences', JSON.stringify({ theme: value })),
			theme
		);
		const api = await mockStudentCare(page);
		await page.goto(carePath('/visits/new'));
		await expect(page.locator('html')).toHaveClass(theme === 'dark' ? /dark/ : /^(?!.*\bdark\b)/);
		await page.getByRole('checkbox', { name: /นักเรียนทดสอบ หนึ่ง/ }).check();
		await page.getByRole('checkbox', { name: 'ครูผู้รับผิดชอบ', exact: true }).check();
		await page.getByLabel(/^ข้อเท็จจริงที่สังเกต(?: \*)?$/).fill('ข้อมูลสมมติจากการเยี่ยมบ้าน');
		await page.getByLabel(/^แนวทางที่ตกลงร่วมกัน(?: \*)?$/).fill('ข้อตกลงสมมติ');
		await page.screenshot({
			path: `/tmp/schoolorbit-student-care-visit-form-${theme}.png`,
			fullPage: true
		});
		await page.getByRole('button', { name: 'บันทึกเยี่ยมบ้าน', exact: true }).click();
		await expect(page).toHaveURL(new RegExp(`/visits/${careVisitId}`));
		await expect(page.getByText('ข้อมูลสมมติจากการเยี่ยมบ้าน', { exact: true })).toBeVisible();
		expect(api.visits[0].visitorIds).toEqual([actor]);
		expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
			true
		);
		await page.screenshot({
			path: `/tmp/schoolorbit-student-care-visit-${theme}.png`,
			fullPage: true
		});
	});
test('case event history loads older pages and preserves latest note, referral and closure', async ({
	page
}) => {
	const api = await mockStudentCare(page, { manyEvents: true });
	await page.goto(carePath(`/cases/${careCaseId}`));
	await page.getByRole('button', { name: 'โหลดประวัติก่อนหน้า', exact: true }).click();
	await expect(page.getByText('บันทึกสมมติ 0', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ประเภทการดำเนินการ', exact: true }).click();
	await page.getByRole('option', { name: 'ส่งต่อ', exact: true }).click();
	await page
		.getByLabel('รายละเอียดการดำเนินการ *', { exact: true })
		.fill('ส่งต่อสมมติให้ผู้รับผิดชอบ');
	await page.getByRole('button', { name: 'บันทึกการดำเนินการ', exact: true }).click();
	await expect(page.getByText('ส่งต่อสมมติให้ผู้รับผิดชอบ', { exact: true })).toBeVisible();
	await expect(page.getByText('บันทึกสมมติ 0', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ประเภทการดำเนินการ', exact: true }).click();
	await page.getByRole('option', { name: 'ปิดการดูแล', exact: true }).click();
	await page
		.getByLabel('รายละเอียดการดำเนินการ *', { exact: true })
		.fill('ปิดการดูแลสมมติพร้อมเหตุผล');
	await page.getByRole('button', { name: 'บันทึกการดำเนินการ', exact: true }).click();
	await expect(page.getByText('ปิดการดูแลสมมติพร้อมเหตุผล', { exact: true })).toBeVisible();
	expect(api.events.map((event) => event.kind)).toEqual(['referral', 'closure']);
	expect(api.events[1].status).toBe('closed');
});

test('routine manager cancels delivery metadata without fetching confidential answers', async ({
	page
}) => {
	const api = await mockStudentCare(page, {
		permissions: [PERMISSIONS.STUDENT_CARE_READ_SCHOOL, PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL]
	});
	await page.goto(carePath(`/rounds/${careRoundId}`));
	await expect(page.getByRole('link', { name: 'เปิดแบบประเมิน', exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: 'ยกเลิกการมอบหมาย', exact: true }).click();
	await page.getByLabel('เหตุผลการยกเลิก *', { exact: true }).fill('มอบหมายซ้ำในข้อมูลสมมติ');
	await page.getByRole('button', { name: 'ยืนยันยกเลิกการมอบหมาย', exact: true }).click();
	await expect(page.getByText('ยกเลิก', { exact: true })).toBeVisible();
	expect(
		api.reads.some((path) => path === `/api/student-care/assignments/${careAssignmentId}`)
	).toBe(false);
});
test('school read combined with assigned manage does not authorize unassigned resource controls', async ({
	page
}) => {
	await mockStudentCare(page, {
		unassigned: true,
		permissions: [
			PERMISSIONS.STUDENT_CARE_READ_SCHOOL,
			PERMISSIONS.STUDENT_CARE_MANAGE_ASSIGNED,
			PERMISSIONS.STUDENT_CARE_SENSITIVE_READ_SCHOOL,
			PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_ASSIGNED
		]
	});
	await page.goto(carePath(`/visits/${careVisitId}`));
	await expect(page.getByRole('link', { name: 'แก้ไขบันทึก', exact: true })).toHaveCount(0);
	await page.goto(carePath(`/cases/${careCaseId}`));
	await expect(page.getByRole('link', { name: 'แก้ผู้รับผิดชอบและแผน', exact: true })).toHaveCount(
		0
	);
	await expect(page.getByRole('button', { name: 'บันทึกการดำเนินการ', exact: true })).toHaveCount(
		0
	);
	await page.goto(`/student-care/assignments/${careAssignmentId}`);
	await expect(page.getByRole('button', { name: 'บันทึกร่าง', exact: true })).toHaveCount(0);
});
test('case creation preserves its assessment source and updates named responsibility', async ({
	page
}) => {
	const api = await mockStudentCare(page, { submitted: true });
	await page.goto(`/student-care/assignments/${careAssignmentId}`);
	await expect(page.getByRole('heading', { name: 'คำตอบที่ส่งแล้ว', exact: true })).toBeVisible();
	await page.getByRole('link', { name: 'เปิดการดูแลจากแบบประเมิน', exact: true }).click();
	await page.getByLabel('เรื่องที่ดูแล *', { exact: true }).fill('ติดตามสมมติจากแบบประเมิน');
	await page
		.getByLabel('รายละเอียด / เหตุผล *', { exact: true })
		.fill('เปิดการดูแลสมมติโดยผู้รับผิดชอบ');
	await page.getByRole('checkbox', { name: 'ครูผู้รับผิดชอบ', exact: true }).check();
	await page.getByRole('button', { name: 'เปิดการดูแล', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`/cases/${careCaseId}`));
	expect(api.cases[0].sourceAssignmentId).toBe(careAssignmentId);
	await page.getByRole('link', { name: 'แก้ผู้รับผิดชอบและแผน', exact: true }).click();
	await page.getByRole('checkbox', { name: 'ครูผู้ติดตาม', exact: true }).check();
	await page.getByLabel('รายละเอียด / เหตุผล *', { exact: true }).fill('เพิ่มผู้ติดตามสมมติ');
	await page.getByRole('button', { name: 'บันทึกผู้รับผิดชอบและแผน', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`/cases/${careCaseId}`));
	expect(api.cases[1].assignedUserIds).toHaveLength(2);
	await page.goto(carePath());
	await expect(
		page.getByRole('heading', { name: 'ดูแลช่วยเหลือนักเรียน', exact: true })
	).toBeVisible();
	await page.screenshot({
		path: '/tmp/schoolorbit-student-care-overview-full-authority.png',
		fullPage: true
	});
});
test('visit completion requires factual record and retains the completed mobile record', async ({
	page
}) => {
	const api = await mockStudentCare(page);
	await page.goto(carePath(`/visits/${careVisitId}/edit`));
	await page.getByRole('button', { name: 'สถานะการเยี่ยมบ้าน', exact: true }).click();
	await page.getByRole('option', { name: 'เยี่ยมแล้ว', exact: true }).click();
	await page.getByRole('button', { name: 'วันที่เยี่ยมจริง', exact: true }).click();
	await page.locator('[data-bits-day][data-today]').click();
	await page.getByLabel(/^ผู้ให้ข้อมูล \/ ความสัมพันธ์(?: \*)?$/).fill('ผู้ปกครองสมมติ');
	await page.getByLabel(/^ข้อเท็จจริงที่สังเกต(?: \*)?$/).fill('ข้อเท็จจริงสมมติ');
	await page.getByRole('button', { name: 'บันทึกเยี่ยมบ้าน', exact: true }).click();
	await expect(page.getByRole('alert').filter({ hasText: 'แนวทางที่ตกลง' })).toBeVisible();
	expect(api.visits).toHaveLength(0);
	await page.getByLabel(/^แนวทางที่ตกลงร่วมกัน(?: \*)?$/).fill('นัดติดตามสมมติ');
	await page.getByRole('button', { name: 'บันทึกเยี่ยมบ้าน', exact: true }).click();
	await expect(page.getByText('บันทึกเยี่ยมบ้านแล้ว', { exact: true })).toBeVisible();
	expect(api.visits[0].status).toBe('completed');
	expect(api.visits[0].visitedDate).toBeTruthy();
});
test('late draft completion cannot update a disposed response page', async ({ page }) => {
	const api = await mockStudentCare(page, {
		holdDraft: true,
		userType: 'student',
		permissions: [PERMISSIONS.STUDENT_CARE_READ_OWN]
	});
	await page.goto(`/student-care/assignments/${careAssignmentId}`);
	await page.getByRole('checkbox', { name: 'อ่านและรับทราบวัตถุประสงค์การประเมินแล้ว' }).check();
	const answer = page.getByRole('radio', { name: 'ตัวเลือกสมมติ ก', exact: true });
	await answer.focus();
	await page.keyboard.press('Space');
	await expect(answer).toBeChecked();
	await page.getByRole('button', { name: 'บันทึกร่าง', exact: true }).click();
	await expect.poll(() => api.responses.length).toBe(1);
	await page.getByRole('link', { name: 'กลับแบบประเมินของฉัน', exact: true }).click();
	await expect(page).toHaveURL(/\/student\/student-care/);
	api.release();
	await expect(page.getByText('บันทึกร่างบนเซิร์ฟเวอร์แล้ว', { exact: true })).toHaveCount(0);
});

test('school manager hands off reviewers through a lazy metadata dialog without reading responses', async ({
	page
}) => {
	const api = await mockStudentCare(page, {
		submitted: true,
		permissions: [PERMISSIONS.STUDENT_CARE_READ_SCHOOL, PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL]
	});
	await page.goto(carePath(`/rounds/${careRoundId}`));
	expect(api.reads.some((path) => path.endsWith('/staff'))).toBe(false);
	await page.getByRole('button', { name: 'เปลี่ยนผู้รับผิดชอบติดตาม', exact: true }).click();
	await page.getByRole('checkbox', { name: 'ครูผู้ติดตาม', exact: true }).check();
	await page.getByLabel('เหตุผลการเปลี่ยนผู้รับผิดชอบ *', { exact: true }).fill('ส่งต่องานสมมติ');
	await page.getByRole('button', { name: 'บันทึกผู้รับผิดชอบติดตาม', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	expect(api.reviewerChanges[0].reviewerUserIds).toHaveLength(2);
	expect(
		api.reads.some((path) => path === `/api/student-care/assignments/${careAssignmentId}`)
	).toBe(false);
});

test('school manager can assign a subject teacher who is not the dated advisor', async ({
	page
}) => {
	const api = await mockStudentCare(page, { respondent: 'teacher' });
	await page.goto(carePath(`/rounds/${careRoundId}/assign`));
	await page.getByRole('button', { name: 'รุ่นแบบประเมิน', exact: true }).click();
	await page.getByRole('option', { name: /แบบทดสอบสมมติ/ }).click();
	await page.getByRole('checkbox', { name: /นักเรียนทดสอบ หนึ่ง/ }).check();
	await page
		.getByRole('group', { name: 'ครูผู้ตอบ', exact: true })
		.getByRole('checkbox', { name: 'ครูผู้ติดตาม', exact: true })
		.check();
	await page
		.getByRole('group', { name: 'ผู้รับผิดชอบติดตามผล', exact: true })
		.getByRole('checkbox', { name: 'ครูผู้รับผิดชอบ', exact: true })
		.check();
	await page.getByRole('button', { name: 'มอบหมายแบบประเมิน', exact: true }).click();
	await expect(page.getByText('มอบหมายแล้ว 1 รายการ', { exact: true })).toBeVisible();
	expect(api.assignments[0].targets[0].respondentId).not.toBe(actor);
});
test('final validation stays visible before confirmation and submit failure stays inside the modal', async ({
	page
}) => {
	const api = await mockStudentCare(page, {
		failDraft: true,
		userType: 'student',
		permissions: [PERMISSIONS.STUDENT_CARE_READ_OWN]
	});
	await page.goto(`/student-care/assignments/${careAssignmentId}`);
	await page.getByRole('button', { name: 'ตรวจและส่งแบบประเมิน', exact: true }).click();
	await expect(page.getByRole('alert').filter({ hasText: 'วัตถุประสงค์' })).toBeVisible();
	await expect(page.getByRole('alertdialog')).toHaveCount(0);
	await page.getByRole('checkbox', { name: 'อ่านและรับทราบวัตถุประสงค์การประเมินแล้ว' }).check();
	await page.getByRole('button', { name: 'ตรวจและส่งแบบประเมิน', exact: true }).click();
	await expect(page.getByRole('alert').filter({ hasText: 'ตอบให้ครบทุกข้อ' })).toBeVisible();
	await expect(page.getByRole('alertdialog')).toHaveCount(0);
	await page.getByRole('radio', { name: 'ตัวเลือกสมมติ ก', exact: true }).check();
	await page.getByRole('radio', { name: 'คำตอบสมมติ สอง', exact: true }).check();
	await page.getByRole('button', { name: 'ตรวจและส่งแบบประเมิน', exact: true }).click();
	const dialog = page.getByRole('alertdialog');
	await dialog.getByRole('button', { name: 'ยืนยันส่งแบบประเมิน', exact: true }).click();
	await expect(dialog.getByRole('alert').filter({ hasText: 'กรุณาลองอีกครั้ง' })).toBeVisible();
	await dialog.getByRole('button', { name: 'ยืนยันส่งแบบประเมิน', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'คำตอบที่ส่งแล้ว', exact: true })).toBeVisible();
	expect(api.responses[0].requestId).toBe(api.responses[1].requestId);
});
test('round transitions offer only close then cancel and leave cancelled rounds terminal', async ({
	page
}) => {
	const api = await mockStudentCare(page);
	await page.goto(carePath(`/rounds/${careRoundId}`));
	await page.getByRole('button', { name: 'เปลี่ยนสถานะรอบเป็น', exact: true }).click();
	await expect(page.getByRole('option', { name: 'เปิดรับคำตอบ', exact: true })).toHaveCount(0);
	await page.getByRole('option', { name: 'ปิดรับคำตอบ', exact: true }).click();
	await page.getByRole('button', { name: 'เปลี่ยนสถานะรอบ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันเปลี่ยนสถานะรอบ', exact: true }).click();
	await expect(page.getByRole('link', { name: 'มอบหมายผู้ตอบ', exact: true })).toHaveCount(0);
	await page.getByRole('button', { name: 'เปลี่ยนสถานะรอบเป็น', exact: true }).click();
	await expect(page.getByRole('option', { name: 'เปิดรับคำตอบ', exact: true })).toHaveCount(0);
	await expect(page.getByRole('option', { name: 'ปิดรับคำตอบ', exact: true })).toHaveCount(0);
	await page.getByRole('option', { name: 'ยกเลิก', exact: true }).click();
	await page.getByRole('button', { name: 'เปลี่ยนสถานะรอบ', exact: true }).click();
	await page.getByRole('button', { name: 'ยืนยันเปลี่ยนสถานะรอบ', exact: true }).click();
	await expect(page.getByRole('button', { name: 'เปลี่ยนสถานะรอบเป็น', exact: true })).toHaveCount(
		0
	);
	expect(api.roundChanges.map((change) => change.status)).toEqual(['closed', 'cancelled']);
});
test('student preview links to complete paginated assessment history beyond fifty entries', async ({
	page
}) => {
	await mockStudentCare(page, { manyAssignments: true });
	await page.goto(carePath(`/students/${actor}`));
	await page.getByRole('link', { name: 'ดูประวัติแบบประเมินทั้งหมด', exact: true }).click();
	await expect(page).toHaveURL(new RegExp(`/students/${actor}/assessments`));
	await expect(page.getByText('แบบประเมินประวัติ 51', { exact: true })).toHaveCount(0);
	await page.getByRole('link', { name: 'ถัดไป', exact: true }).click();
	await expect(page.getByText('แบบประเมินประวัติ 51', { exact: true })).toBeVisible();
});
