import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });

const subjectId = '10000000-0000-4000-8000-000000000101';
const questionId = '20000000-0000-4000-8000-000000000101';
const userId = '30000000-0000-4000-8000-000000000101';
const questionBankUrl = '/staff/academic/question-bank';

function content(text: string) {
	return {
		schemaVersion: 1,
		document: { type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text }] }] }
	};
}

function question(text = 'โจทย์เริ่มต้น') {
	return {
		id: questionId,
		subjectId,
		subjectCode: 'ค21101',
		subjectNameTh: 'คณิตศาสตร์',
		subjectNameEn: null,
		subjectGroupId: null,
		subjectGroupName: null,
		ownerUserId: userId,
		questionType: 'short_answer',
		difficulty: 'medium',
		points: 1,
		status: 'ready',
		stemContent: content(text),
		explanationContent: null,
		rubricContent: null,
		tags: [],
		choiceCount: 0,
		correctChoiceCount: 0,
		canManage: true,
		createdAt: '2026-09-01T00:00:00Z',
		updatedAt: '2026-09-01T00:00:00Z'
	};
}

function questionPage(text = 'โจทย์เริ่มต้น') {
	return {
		items: [question(text)],
		page: 1,
		pageSize: 20,
		total: 1,
		totalPages: 1,
		summary: { total: 1, choice: 0, written: 1, ready: 1 }
	};
}

function deferred() {
	let resolve!: () => void;
	const promise = new Promise<void>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

async function fulfill(route: Route, data: unknown) {
	await route.fulfill({
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data })
	});
}

async function mockApis(
	page: Page,
	options: {
		optionsWait?: Promise<void>;
		listWait?: Promise<void>;
		failOptionsOnce?: boolean;
		failListOnce?: boolean;
		onDetailRead?: () => void;
		onOptionsRead?: () => void;
		onListRead?: (search: string) => void;
		searchWait?: Record<string, Promise<void>>;
	} = {}
) {
	let optionsReads = 0;
	let listReads = 0;
	let deleted = false;
	await page.route('**/api/**', async (route) => {
		const url = new URL(route.request().url());
		if (url.pathname === '/api/auth/me') {
			await fulfill(route, {
				id: userId,
				username: 'question-bank-admin',
				firstName: 'ทดสอบ',
				lastName: 'ระบบ',
				userType: 'staff',
				status: 'ACTIVE',
				permissions: ['*']
			});
			return;
		}
		if (url.pathname === '/api/academic/question-bank/options') {
			optionsReads += 1;
			options.onOptionsRead?.();
			await options.optionsWait;
			if (options.failOptionsOnce && optionsReads === 1) {
				await route.fulfill({
					status: 500,
					contentType: 'application/json',
					body: '{"success":false,"error":"options failed"}'
				});
				return;
			}
			await fulfill(route, {
				subjects: [{ id: subjectId, code: 'ค21101', nameTh: 'คณิตศาสตร์', canCreate: true }]
			});
			return;
		}
		if (url.pathname === '/api/academic/question-bank/questions') {
			listReads += 1;
			const search = url.searchParams.get('search') ?? '';
			options.onListRead?.(search);
			await (options.searchWait?.[search] ?? options.listWait);
			if (options.failListOnce && listReads === 1) {
				await route.fulfill({
					status: 500,
					contentType: 'application/json',
					body: '{"success":false,"error":"list failed"}'
				});
				return;
			}
			try {
				await fulfill(
					route,
					deleted
						? {
								items: [],
								page: 1,
								pageSize: 20,
								total: 0,
								totalPages: 1,
								summary: { total: 0, choice: 0, written: 0, ready: 0 }
							}
						: questionPage(search ? `โจทย์ ${search}` : undefined)
				);
			} catch {
				/* Superseded request may already be aborted. */
			}
			return;
		}
		if (url.pathname === `/api/academic/question-bank/questions/${questionId}`) {
			if (route.request().method() === 'DELETE') {
				deleted = true;
				await fulfill(route, {});
				return;
			}
			options.onDetailRead?.();
			await fulfill(route, { ...question(), choices: [], files: [] });
			return;
		}
		if (url.pathname === '/api/menu/user') {
			await fulfill(route, { groups: [] });
			return;
		}
		if (url.pathname === '/api/me/work-items/counts') {
			await fulfill(route, { open: 0, dueSoon: 0, overdue: 0, submitted: 0, closed: 0, total: 0 });
			return;
		}
		if (url.pathname === '/api/notifications') {
			await fulfill(route, { items: [], unread_count: 0 });
			return;
		}
		if (url.pathname === '/api/notifications/stream') {
			await route.fulfill({ contentType: 'text/event-stream', body: '' });
			return;
		}
		await fulfill(route, {});
	});
}

test('list renders while subject options are pending; detail remains action-only', async ({
	page
}) => {
	const pending = deferred();
	let detailReads = 0;
	await mockApis(page, {
		optionsWait: pending.promise,
		onDetailRead: () => {
			detailReads += 1;
		}
	});
	try {
		await page.goto(questionBankUrl);
		await expect(page.getByText('โจทย์เริ่มต้น')).toBeVisible();
		await expect(page.getByTestId('question-bank-options-loading')).toBeVisible();
		// Opening the list does not fetch full question detail.
		expect(detailReads).toBe(0);
	} finally {
		pending.resolve();
	}
	await expect(page.getByRole('button', { name: 'เพิ่มข้อสอบ' })).toBeVisible();
	await page.getByRole('button', { name: 'ดู', exact: true }).click();
	await expect(page.getByRole('dialog')).toContainText('โจทย์เริ่มต้น');
	await expect.poll(() => detailReads).toBe(1);
});

test('subject options render while list is pending', async ({ page }) => {
	const pending = deferred();
	await mockApis(page, { listWait: pending.promise });
	try {
		await page.goto(questionBankUrl);
		await expect(page.getByTestId('question-bank-list-loading')).toBeVisible();
		await expect(page.getByRole('button', { name: 'เพิ่มข้อสอบ' })).toBeVisible();
		await expect(page.getByTestId('question-bank-options-loading')).toHaveCount(0);
	} finally {
		pending.resolve();
	}
	await expect(page.getByText('โจทย์เริ่มต้น')).toBeVisible();
});

test('failed options retry does not reread or hide the list', async ({ page }) => {
	let listReads = 0;
	await mockApis(page, {
		failOptionsOnce: true,
		onListRead: () => {
			listReads += 1;
		}
	});
	await page.goto(questionBankUrl);
	await expect(page.getByText('โจทย์เริ่มต้น')).toBeVisible();
	await expect(page.getByText('options failed')).toBeVisible();
	await page.getByRole('button', { name: 'ลองโหลดรายวิชาอีกครั้ง' }).click();
	await expect(page.getByRole('button', { name: 'เพิ่มข้อสอบ' })).toBeVisible();
	expect(listReads).toBe(1);
});

test('failed list retry does not reread or hide subject options', async ({ page }) => {
	let optionReads = 0;
	await mockApis(page, {
		failListOnce: true,
		onOptionsRead: () => {
			optionReads += 1;
		}
	});
	await page.goto(questionBankUrl);
	await expect(page.getByRole('button', { name: 'เพิ่มข้อสอบ' })).toBeVisible();
	await expect(page.getByText('list failed')).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText('โจทย์เริ่มต้น')).toBeVisible();
	expect(optionReads).toBe(1);
});

test('newer search result wins over a delayed older search', async ({ page }) => {
	const old = deferred();
	let oldRead = 0;
	await mockApis(page, {
		searchWait: { old: old.promise },
		onListRead: (search) => {
			if (search === 'old') oldRead += 1;
		}
	});
	await page.goto(questionBankUrl);
	await expect(page.getByText('โจทย์เริ่มต้น')).toBeVisible();
	const search = page.getByPlaceholder('ข้อความโจทย์หรือรหัสวิชา');
	await search.fill('old');
	await search.press('Enter');
	await expect.poll(() => oldRead).toBe(1);
	try {
		await search.fill('new');
		await search.press('Enter');
		await expect(page.getByText('โจทย์ new')).toBeVisible();
	} finally {
		old.resolve();
	}
	await expect(page.getByText('โจทย์ old')).toHaveCount(0);
});

test('rich editor JavaScript is loaded only when create is opened', async ({ page }) => {
	const scriptRequests: string[] = [];
	page.on('request', (request) => {
		if (/\/_app\/immutable\/chunks\/[^/]+\.js$/.test(request.url()))
			scriptRequests.push(request.url());
	});
	await mockApis(page);
	await page.goto(questionBankUrl);
	await expect(page.getByText('โจทย์เริ่มต้น')).toBeVisible();
	await expect(page.getByRole('button', { name: 'เพิ่มข้อสอบ' })).toBeVisible();
	// SvelteKit may eagerly link editor CSS from its route manifest; the rich editor JS is action-only.
	const scriptsBeforeCreate = new Set(scriptRequests);
	await page.getByRole('button', { name: 'เพิ่มข้อสอบ' }).click();
	await expect(page.getByRole('dialog')).toContainText('บันทึกข้อสอบ');
	await expect.poll(() => scriptRequests.some((url) => !scriptsBeforeCreate.has(url))).toBe(true);
});

test('deleting rereads only the affected paginated list', async ({ page }) => {
	let optionsReads = 0;
	let listReads = 0;
	await mockApis(page, {
		onOptionsRead: () => {
			optionsReads += 1;
		},
		onListRead: () => {
			listReads += 1;
		}
	});
	await page.goto(questionBankUrl);
	await expect(page.getByText('โจทย์เริ่มต้น')).toBeVisible();
	await page.getByRole('button', { name: 'ลบ', exact: true }).click();
	await expect(page.getByRole('alertdialog')).toContainText('ยืนยันการลบข้อสอบ');
	await page.getByRole('alertdialog').getByRole('button', { name: 'ลบข้อสอบ' }).click();
	await expect(page.getByText('ยังไม่พบข้อสอบ')).toBeVisible();
	await expect.poll(() => listReads).toBe(2);
	expect(optionsReads).toBe(1);
});
