import type { Page, Route } from '@playwright/test';
import { mockStaffHome, id, actor, firstStaff } from './staff-home-route-data';
import { PERMISSIONS } from '../../../src/lib/permissions/registry.generated';
import type {
	BudgetProjectWorkspace,
	BudgetSpendingRequestDetail,
	BudgetCatalogs,
	BudgetPayment,
	BudgetAdjustmentDetail,
	BudgetSettings,
	BudgetDashboard,
	BudgetHistoryPage
} from '../../../src/lib/api/budget';

export const budgetProjectId = id(810),
	budgetRequestId = id(811),
	budgetYearId = id(812),
	budgetOrgId = id(813),
	budgetFundId = id(814),
	learnerFundId = id(815),
	materialsId = id(816),
	equipmentId = id(817);
export const budgetPermissions = [
	PERMISSIONS.BUDGET_READ_SCHOOL,
	PERMISSIONS.BUDGET_CREATE_OWN,
	PERMISSIONS.BUDGET_UPDATE_OWN,
	PERMISSIONS.BUDGET_APPROVE_ASSIGNED,
	PERMISSIONS.BUDGET_MANAGE_SCHOOL,
	PERMISSIONS.BUDGET_PAYMENT_EXECUTE_SCHOOL,
	PERMISSIONS.BUDGET_ADJUSTMENT_CREATE_SCHOOL,
	PERMISSIONS.BUDGET_DOWNLOAD_SCHOOL,
	PERMISSIONS.SUPPLY_CATALOG_READ_SCHOOL
];
export const budgetCatalogs: BudgetCatalogs = {
	fiscalYears: [
		{ id: budgetYearId, name: '2570', startDate: '2026-10-01', endDate: '2027-09-30', active: true }
	],
	funds: [
		{ id: budgetFundId, name: 'เงินอุดหนุน', active: true },
		{ id: learnerFundId, name: 'เงินพัฒนาผู้เรียน', active: true }
	],
	expenseCategories: [
		{ id: materialsId, name: 'วัสดุ', active: true },
		{ id: equipmentId, name: 'ครุภัณฑ์', active: true }
	]
};
export function budgetWorkspace(approved = true): BudgetProjectWorkspace {
	const activities = [
		['กิจกรรมที่หนึ่ง', '358800', budgetFundId, materialsId],
		['กิจกรรมที่สอง', '304000', budgetFundId, equipmentId],
		['กิจกรรมที่สาม', '330000', learnerFundId, materialsId]
	].map(([title, total, fundId, expenseCategoryId], index) => ({
		id: id(820 + index),
		title,
		defaultFundId: fundId,
		defaultExpenseCategoryId: expenseCategoryId,
		totalSatang: total,
		lines: [
			{
				id: id(830 + index),
				description: `รายการค่าใช้จ่าย ${index + 1}`,
				quantity: '1',
				unit: 'รายการ',
				unitPriceSatang: total,
				totalSatang: total,
				fundId,
				expenseCategoryId,
				supplyItemId: null,
				supplyVersionId: null,
				supplyVersionRevision: null,
				supplyNameSnapshot: null
			}
		]
	}));
	const project = {
		id: budgetProjectId,
		revision: 1,
		name: 'โครงการคณิตศาสตร์',
		fiscalYearId: budgetYearId,
		organizationUnitId: budgetOrgId,
		organizationUnitName: 'กลุ่มสาระคณิตศาสตร์',
		startDate: '2026-10-01',
		endDate: '2027-09-30',
		ownerId: actor,
		status: approved ? ('approved' as const) : ('draft' as const),
		acceptedVersion: approved ? 1 : null,
		totalSatang: '992800',
		reservedSatang: '0',
		paidSatang: '0',
		availableSatang: approved ? '992800' : '0'
	};
	return {
		project,
		activities,
		acceptedPlan: approved
			? {
					id: id(840),
					version: 1,
					project: {
						name: project.name,
						fiscalYearId: budgetYearId,
						organizationUnitId: budgetOrgId,
						startDate: project.startDate,
						endDate: project.endDate
					},
					activities: structuredClone(activities),
					totalSatang: '992800',
					acceptedAt: '2026-10-20T01:00:00Z',
					acceptedBy: actor
				}
			: null,
		balances: [],
		capabilities: {
			canEdit: !approved,
			canSubmit: !approved,
			canAct: false,
			canAmend: approved,
			canCreateRequest: approved,
			canPay: false,
			canClose: false,
			canAdjust: true,
			canManageSettings: true,
			canExport: true
		}
	};
}
export async function mockBudget(
	page: Page,
	options: {
		permissions?: string[];
		approved?: boolean;
		hold?: 'dashboard' | 'projects' | 'requests' | 'payments';
		fail?: 'dashboard' | 'projects' | 'requests' | 'payments';
		failMutation?: boolean;
		withRequest?: boolean;
		empty?: boolean;
		holdMutation?: boolean;
		stalePlan?: boolean;
		stalePayment?: boolean;
		exportFiltered?: boolean;
		reviewer?: boolean;
	} = {}
) {
	await mockStaffHome(page, { permissions: options.permissions ?? budgetPermissions });
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
	let workspace = budgetWorkspace(options.approved ?? true),
		request: BudgetSpendingRequestDetail | null = null,
		settings: BudgetSettings = {
			...structuredClone(budgetCatalogs),
			revision: 1,
			planSteps: [],
			requestSteps: [],
			adjustmentSteps: []
		};
	const retainedRows: BudgetDashboard['rows'] = [];
	const payments: BudgetPayment[] = [],
		adjustments: BudgetAdjustmentDetail[] = [],
		reads: string[] = [],
		writes: { path: string; payload: Record<string, unknown> }[] = [];
	const history: BudgetHistoryPage['items'] = [];
	if (options.withRequest) {
		const activity = workspace.acceptedPlan!.activities[0],
			line = activity.lines[0];
		request = {
			request: {
				id: budgetRequestId,
				projectId: budgetProjectId,
				activityId: activity.id,
				ownerId: actor,
				title: 'คำขอทดสอบ',
				revision: 1,
				status: 'approved',
				totalSatang: '1000',
				reservedSatang: '1000',
				paidSatang: '0',
				remainingSatang: '1000',
				createdAt: '2026-10-20T01:00:00Z'
			},
			lines: [
				{
					planLineId: line.id,
					description: line.description,
					unit: line.unit,
					fundId: line.fundId,
					fundName: 'เงินอุดหนุน',
					expenseCategoryId: line.expenseCategoryId,
					expenseCategoryName: 'วัสดุ',
					amountSatang: '1000',
					reservedSatang: '1000',
					paidSatang: '0'
				}
			],
			evidenceFileIds: [],
			capabilities: {
				...workspace.capabilities,
				canEdit: false,
				canSubmit: false,
				canPay: true,
				canClose: true
			}
		};
	}
	let role: 'requester' | 'reviewer' = options.reviewer ? 'reviewer' : 'requester';
	const effectivePermissions = () =>
		role === 'reviewer'
			? [PERMISSIONS.BUDGET_READ_ASSIGNED, PERMISSIONS.BUDGET_APPROVE_ASSIGNED]
			: (options.permissions ?? budgetPermissions);
	function syncCapabilities() {
		const requester = role === 'requester',
			permissions = effectivePermissions();
		workspace.capabilities = {
			...workspace.capabilities,
			canEdit:
				requester &&
				['draft', 'returned'].includes(workspace.project.status) &&
				permissions.includes(PERMISSIONS.BUDGET_UPDATE_OWN),
			canSubmit:
				requester &&
				['draft', 'returned'].includes(workspace.project.status) &&
				permissions.includes(PERMISSIONS.BUDGET_UPDATE_OWN),
			canAct: !requester && workspace.project.status === 'submitted',
			canAmend:
				requester &&
				workspace.project.status === 'approved' &&
				permissions.includes(PERMISSIONS.BUDGET_UPDATE_OWN),
			canCreateRequest:
				requester && !!workspace.acceptedPlan && permissions.includes(PERMISSIONS.BUDGET_CREATE_OWN)
		};
		if (request)
			request.capabilities = {
				...request.capabilities,
				canEdit: requester && ['draft', 'returned'].includes(request.request.status),
				canSubmit: requester && ['draft', 'returned'].includes(request.request.status),
				canAct: !requester && request.request.status === 'submitted',
				canPay: requester && request.request.status === 'approved',
				canClose: requester && request.request.status === 'approved'
			};
		for (const adjustment of adjustments)
			adjustment.capabilities = {
				...adjustment.capabilities,
				canEdit: requester && ['draft', 'returned'].includes(adjustment.status),
				canSubmit: requester && ['draft', 'returned'].includes(adjustment.status),
				canAct: !requester && adjustment.status === 'submitted'
			};
	}
	let mutationFailed = false,
		stalePlanConsumed = false,
		stalePaymentConsumed = false,
		initialFailed = false,
		release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	const reply = (route: Route, data: unknown, status = 200) => {
		syncCapabilities();
		return route.fulfill({
			status,
			contentType: 'application/json',
			headers: { 'X-CSRF-Token': 'synthetic-csrf' },
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	};
	await page.route('**/api/auth/me', (route) =>
		reply(route, {
			id: role === 'requester' ? actor : firstStaff,
			username: `budget-${role}`,
			firstName: role === 'requester' ? 'ผู้ขอ' : 'ผู้พิจารณา',
			lastName: 'ทดสอบ',
			displayName: role === 'requester' ? 'ผู้ขอ ทดสอบ' : 'ผู้พิจารณา ทดสอบ',
			userType: 'staff',
			status: 'active',
			permissions: effectivePermissions()
		})
	);
	const pageData = <T>(items: T[], url: URL) => {
		const limit = Number(url.searchParams.get('limit') ?? 20),
			offset = Number(url.searchParams.get('offset') ?? 0);
		return { items: items.slice(offset, offset + limit), total: items.length, limit, offset };
	};
	function process(resource: 'project' | 'request' | 'adjustment', resourceId: string) {
		const status =
			resource === 'project'
				? workspace.project.status
				: resource === 'request'
					? request?.request.status
					: adjustments.find((item) => item.id === resourceId)?.status;
		if (status !== 'submitted') return { process: null };
		return {
			process: {
				id: id(850),
				revision: 1,
				title: 'พิจารณางบประมาณ',
				source: { moduleCode: 'budget', resourceId, resourceType: resource },
				status: 'active',
				currentStep: 0,
				steps: [
					{
						stepIndex: 0,
						definition: {
							title: 'อนุมัติ',
							kind: 'approve',
							completionPolicy: 'all',
							recipients: [{ kind: 'user', userId: firstStaff }],
							requiresComment: false
						},
						recipients: [{ userId: firstStaff, displayName: 'ผู้พิจารณาทดสอบ' }]
					}
				],
				createdAt: '2026-10-20T01:00:00Z',
				createdBy: actor,
				actionPath: `/staff/budget/${resource === 'project' ? 'projects' : resource === 'request' ? 'requests' : 'adjustments'}/${resourceId}`
			}
		};
	}
	function dashboard(): BudgetDashboard {
		const rows = (workspace.acceptedPlan?.activities ?? []).flatMap((activity) =>
			activity.lines.map((line) => ({
				projectId: budgetProjectId,
				projectName: workspace.project.name,
				activityId: activity.id,
				activityTitle: activity.title,
				fundId: line.fundId,
				fundName: budgetCatalogs.funds.find((item) => item.id === line.fundId)!.name,
				expenseCategoryId: line.expenseCategoryId,
				expenseCategoryName: budgetCatalogs.expenseCategories.find(
					(item) => item.id === line.expenseCategoryId
				)!.name,
				plannedSatang: line.totalSatang,
				adjustedSatang: '0',
				currentSatang: line.totalSatang,
				reservedSatang: '0',
				paidSatang: '0',
				availableSatang: line.totalSatang,
				loanOutstandingSatang: '0'
			}))
		);
		return {
			rows: [...rows, ...retainedRows],
			totals: {
				plannedSatang: '992800',
				adjustedSatang: '0',
				currentSatang: '992800',
				reservedSatang: '0',
				paidSatang: '0',
				availableSatang: '992800',
				loanOutstandingSatang: '0'
			}
		};
	}
	await page.route('**/api/lookup/organization-units*', (route) =>
		reply(route, [{ id: budgetOrgId, name: 'กลุ่มสาระคณิตศาสตร์', code: 'MATH' }])
	);
	await page.route('**/api/lookup/staff*', (route) =>
		reply(route, [{ id: actor, name: 'ผู้พิจารณาทดสอบ' }])
	);
	let fileNumber = 880;
	await page.route('**/api/files', (route) =>
		reply(route, {
			id: id(fileNumber++),
			byteSize: 16,
			currentVersion: 1,
			detectedMimeType: 'application/pdf',
			displayFilename: 'หลักฐานทดสอบ.pdf',
			lifecycleStatus: 'ready',
			publicContentUrl: null,
			purpose: 'budget_evidence'
		})
	);
	await page.route('**/api/budget/**', async (route) => {
		const req = route.request(),
			url = new URL(req.url()),
			path = url.pathname,
			method = req.method();
		const kind = path.endsWith('/dashboard')
			? 'dashboard'
			: path.endsWith('/projects')
				? 'projects'
				: path.endsWith('/payments')
					? 'payments'
					: path.endsWith('/requests')
						? 'requests'
						: '';
		if (method === 'GET') {
			reads.push(path + url.search);
			if (options.hold === kind) await held;
			if (options.fail === kind && !initialFailed) {
				initialFailed = true;
				return reply(route, `บริการ ${kind} ไม่พร้อม`, 503);
			}
		}
		const payload = method === 'GET' ? {} : req.postDataJSON();
		if (method === 'POST' && path.endsWith('/actions') && role === 'requester')
			return reply(route, 'ไม่สามารถพิจารณารายการของตนเอง', 403);
		if (method !== 'GET') {
			writes.push({ path, payload });
			if (options.holdMutation) await held;
			if (options.failMutation && !mutationFailed) {
				mutationFailed = true;
				return reply(route, 'บันทึกไม่สำเร็จชั่วคราว', 503);
			}
			if (
				options.stalePlan &&
				!stalePlanConsumed &&
				method === 'PUT' &&
				path === `/api/budget/projects/${budgetProjectId}`
			) {
				stalePlanConsumed = true;
				workspace.project.revision++;
				workspace.project.name = 'ชื่อโครงการจากอีกคน';
				workspace.activities[0].title = 'กิจกรรมจากอีกคน';
				return reply(route, 'แผนเปลี่ยนแปลงแล้ว กรุณาตรวจสอบฉบับล่าสุด', 409);
			}
			if (
				options.stalePayment &&
				!stalePaymentConsumed &&
				path === `/api/budget/requests/${budgetRequestId}/payments`
			) {
				stalePaymentConsumed = true;
				request!.request.revision++;
				request!.request.reservedSatang = '500';
				request!.request.paidSatang = '500';
				request!.request.remainingSatang = '500';
				request!.lines[0].reservedSatang = '500';
				request!.lines[0].paidSatang = '500';
				payments.push({
					id: id(860),
					requestId: budgetRequestId,
					paidOn: '2026-10-20',
					voucherNumber: 'PV-OTHER',
					note: null,
					totalSatang: '500',
					allocations: [{ planLineId: request!.lines[0].planLineId, amountSatang: '500' }],
					evidence: [],
					createdBy: firstStaff,
					createdAt: '2026-10-20T01:00:00Z',
					refunds: []
				});
				return reply(route, 'มีการจ่ายเงินจากอีกคน กรุณาตรวจสอบยอดล่าสุด', 409);
			}
			const action =
				method === 'PUT'
					? 'saved'
					: path.endsWith('/amendments')
						? 'amendment_started'
						: path.endsWith('/payments')
							? 'payment_recorded'
							: path.endsWith('/refunds')
								? 'refund_recorded'
								: path.endsWith('/close')
									? 'closed'
									: path.endsWith('/submit')
										? 'submitted'
										: path.endsWith('/actions')
											? payload.action === 'return_for_revision'
												? 'returned'
												: payload.action === 'reject'
													? 'rejected'
													: 'approved'
											: null;
			if (action) {
				const resourceKind = path.includes('/requests/')
					? 'request'
					: path.includes('/adjustments/')
						? 'adjustment'
						: 'project';
				history.unshift({
					id: id(1000 + history.length),
					resourceKind,
					resourceId:
						resourceKind === 'request'
							? budgetRequestId
							: resourceKind === 'project'
								? budgetProjectId
								: path.split('/')[4],
					revision: Number(payload.expectedRevision ?? 0) + 1,
					action,
					actorId: role === 'requester' ? actor : firstStaff,
					comment: payload.reason ?? payload.comment ?? null,
					createdAt: '2026-10-20T01:00:00Z'
				});
			}
		}
		if (path.endsWith('/catalogs')) return reply(route, budgetCatalogs);
		if (path.endsWith('/settings')) {
			if (method === 'PUT')
				settings = {
					...payload,
					revision: settings.revision + 1,
					fiscalYears: payload.fiscalYears.map((item: object, index: number) => ({
						id: id(900 + index),
						...item
					})),
					funds: payload.funds.map((item: object, index: number) => ({
						id: id(920 + index),
						...item
					})),
					expenseCategories: payload.expenseCategories.map((item: object, index: number) => ({
						id: id(940 + index),
						...item
					}))
				};
			return reply(route, settings);
		}
		if (path.endsWith('/dashboard')) {
			const result = dashboard();
			if (options.empty) {
				result.rows = [];
				result.totals = Object.fromEntries(
					Object.keys(result.totals).map((key) => [key, '0'])
				) as BudgetDashboard['totals'];
			}
			if (options.exportFiltered && url.searchParams.get('exportScope') === 'true') {
				result.rows = result.rows.slice(0, 1);
				result.totals = {
					...result.totals,
					plannedSatang: '358800',
					currentSatang: '358800',
					availableSatang: '358800'
				};
			}
			return reply(route, result);
		}
		if (path.endsWith('/reports/export'))
			return route.fulfill({
				contentType: 'text/csv',
				body: options.exportFiltered
					? 'โครงการ,กิจกรรม,เงินอุดหนุน/วัสดุ,รวม\nโครงการคณิตศาสตร์,กิจกรรมที่หนึ่ง,3588.00,3588.00'
					: 'โครงการ,กิจกรรม,เงินอุดหนุน/วัสดุ,เงินอุดหนุน/ครุภัณฑ์,เงินพัฒนาผู้เรียน/วัสดุ,รวม\nโครงการคณิตศาสตร์,รวม,3588.00,3040.00,3300.00,9928.00'
			});
		if (path.endsWith('/history'))
			return reply(
				route,
				pageData(
					history.filter((item) => item.resourceId === path.split('/').at(-2)),
					url
				)
			);
		if (path.endsWith('/process')) {
			const resource = path.includes('/projects/')
				? 'project'
				: path.includes('/requests/')
					? 'request'
					: 'adjustment';
			return reply(route, process(resource, path.split('/').at(-2)!));
		}
		if (path === '/api/budget/projects' && method === 'GET')
			return reply(route, pageData(options.empty ? [] : [workspace.project], url));
		if (path === '/api/budget/projects' && method === 'POST') {
			workspace = {
				...workspace,
				project: {
					...workspace.project,
					...payload,
					status: 'draft',
					revision: 1,
					totalSatang: '0'
				},
				activities: [],
				acceptedPlan: null,
				capabilities: {
					...workspace.capabilities,
					canEdit: true,
					canSubmit: true,
					canAmend: false,
					canCreateRequest: false
				}
			};
			return reply(route, workspace);
		}
		if (path === `/api/budget/projects/${budgetProjectId}`) {
			if (method === 'PUT') {
				workspace.activities = payload.activities.map(
					(activity: BudgetProjectWorkspace['activities'][number], index: number) => ({
						...activity,
						id: activity.id ?? id(820 + index),
						lines: activity.lines.map((line, index: number) => ({
							...line,
							id: line.id ?? id(830 + index),
							totalSatang: (
								(BigInt(line.unitPriceSatang) * BigInt(Math.round(Number(line.quantity) * 1000)) +
									500n) /
								1000n
							).toString()
						})),
						totalSatang: '0'
					})
				);
				for (const activity of workspace.activities)
					activity.totalSatang = activity.lines
						.reduce((sum, line) => sum + BigInt(line.totalSatang), 0n)
						.toString();
				workspace.project = {
					...workspace.project,
					...payload,
					revision: workspace.project.revision + 1,
					totalSatang: workspace.activities
						.reduce((sum, activity) => sum + BigInt(activity.totalSatang), 0n)
						.toString()
				};
			}
			return reply(route, workspace);
		}
		if (path === `/api/budget/projects/${budgetProjectId}/submit`) {
			workspace.project.status = 'submitted';
			workspace.project.revision++;
			workspace.capabilities = {
				...workspace.capabilities,
				canEdit: false,
				canSubmit: false,
				canAct: true
			};
			return reply(route, workspace);
		}
		if (path === `/api/budget/projects/${budgetProjectId}/actions`) {
			workspace.project.status =
				payload.action === 'return_for_revision'
					? 'returned'
					: payload.action === 'reject'
						? 'rejected'
						: 'approved';
			workspace.project.revision++;
			workspace.capabilities = {
				...workspace.capabilities,
				canEdit: workspace.project.status === 'returned',
				canSubmit: workspace.project.status === 'returned',
				canAct: false,
				canCreateRequest: workspace.project.status === 'approved'
			};
			if (workspace.project.status === 'approved')
				workspace.acceptedPlan = {
					id: id(840),
					version: 1,
					project: workspace.project,
					activities: structuredClone(workspace.activities),
					totalSatang: workspace.project.totalSatang,
					acceptedAt: '2026-10-20T01:00:00Z',
					acceptedBy: actor
				};
			return reply(route, workspace);
		}
		if (path.endsWith('/amendments')) {
			workspace.project.status = 'draft';
			workspace.project.revision++;
			workspace.capabilities = {
				...workspace.capabilities,
				canEdit: true,
				canSubmit: true,
				canAmend: false
			};
			return reply(route, workspace);
		}
		if (path === '/api/budget/requests' && method === 'GET')
			return reply(route, pageData(request ? [request.request] : [], url));
		if (path === `/api/budget/projects/${budgetProjectId}/requests` && method === 'GET')
			return reply(route, pageData(request ? [request.request] : [], url));
		if (
			(path === `/api/budget/projects/${budgetProjectId}/requests` && method === 'POST') ||
			(path === `/api/budget/requests/${budgetRequestId}` && method === 'PUT')
		) {
			const activity = workspace.acceptedPlan!.activities.find(
				(item) => item.id === payload.activityId
			)!;
			const lines = payload.lines.map((input: { planLineId: string; amountSatang: string }) => {
				const line = activity.lines.find((item) => item.id === input.planLineId)!;
				return {
					...input,
					description: line.description,
					unit: line.unit,
					fundId: line.fundId,
					fundName: budgetCatalogs.funds.find((item) => item.id === line.fundId)!.name,
					expenseCategoryId: line.expenseCategoryId,
					expenseCategoryName: budgetCatalogs.expenseCategories.find(
						(item) => item.id === line.expenseCategoryId
					)!.name,
					reservedSatang: '0',
					paidSatang: '0'
				};
			});
			const total = lines
				.reduce(
					(sum: bigint, line: { amountSatang: string }) => sum + BigInt(line.amountSatang),
					0n
				)
				.toString();
			request = {
				request: {
					id: budgetRequestId,
					projectId: budgetProjectId,
					activityId: payload.activityId,
					ownerId: actor,
					title: payload.title,
					revision: (request?.request.revision ?? 0) + 1,
					status: 'draft',
					totalSatang: total,
					reservedSatang: '0',
					paidSatang: '0',
					remainingSatang: total,
					createdAt: '2026-10-20T01:00:00Z'
				},
				lines,
				evidenceFileIds: payload.evidenceFileIds,
				capabilities: {
					...workspace.capabilities,
					canEdit: true,
					canSubmit: true,
					canAct: false,
					canPay: false,
					canClose: false
				}
			};
			return reply(route, request);
		}
		if (path === `/api/budget/requests/${budgetRequestId}`) return reply(route, request);
		if (path.endsWith(`/requests/${budgetRequestId}/submit`)) {
			request!.request.status = 'submitted';
			request!.request.revision++;
			request!.capabilities = {
				...request!.capabilities,
				canEdit: false,
				canSubmit: false,
				canAct: true
			};
			return reply(route, request);
		}
		if (path.endsWith(`/requests/${budgetRequestId}/actions`)) {
			request!.request.status = 'approved';
			request!.request.revision++;
			request!.request.reservedSatang = request!.request.totalSatang;
			for (const line of request!.lines) line.reservedSatang = line.amountSatang;
			request!.capabilities = {
				...request!.capabilities,
				canAct: false,
				canPay: true,
				canClose: true
			};
			return reply(route, request);
		}
		if (path.endsWith(`/requests/${budgetRequestId}/payments`)) {
			if (method === 'GET') return reply(route, pageData(payments, url));
			const total = payload.allocations
				.reduce(
					(sum: bigint, line: { amountSatang: string }) => sum + BigInt(line.amountSatang),
					0n
				)
				.toString();
			payments.push({
				id: id(860 + payments.length),
				requestId: budgetRequestId,
				paidOn: payload.paidOn,
				voucherNumber: payload.voucherNumber,
				note: payload.note,
				totalSatang: total,
				allocations: payload.allocations,
				evidence: [],
				createdBy: actor,
				createdAt: '2026-10-20T01:00:00Z',
				refunds: []
			});
			for (const allocation of payload.allocations) {
				const line = request!.lines.find((line) => line.planLineId === allocation.planLineId)!;
				line.reservedSatang = (
					BigInt(line.reservedSatang) - BigInt(allocation.amountSatang)
				).toString();
				line.paidSatang = (BigInt(line.paidSatang) + BigInt(allocation.amountSatang)).toString();
			}
			request!.request.paidSatang = (
				BigInt(request!.request.paidSatang) + BigInt(total)
			).toString();
			request!.request.reservedSatang = (
				BigInt(request!.request.reservedSatang) - BigInt(total)
			).toString();
			request!.request.remainingSatang = request!.request.reservedSatang;
			request!.request.revision++;
			return reply(route, request);
		}
		if (path.endsWith(`/requests/${budgetRequestId}/close`)) {
			request!.request.status = 'closed';
			request!.request.reservedSatang = '0';
			for (const line of request!.lines) line.reservedSatang = '0';
			request!.request.revision++;
			request!.capabilities = { ...request!.capabilities, canPay: false, canClose: false };
			return reply(route, request);
		}
		if (path.endsWith('/refunds')) {
			const payment = payments.find((item) => item.id === path.split('/').at(-2))!;
			const total = payload.allocations
				.reduce(
					(sum: bigint, line: { amountSatang: string }) => sum + BigInt(line.amountSatang),
					0n
				)
				.toString();
			payment.refunds.push({
				id: id(890 + payment.refunds.length),
				paymentId: payment.id,
				refundedOn: payload.refundedOn,
				reason: payload.reason,
				totalSatang: total,
				allocations: payload.allocations,
				evidence: [],
				createdBy: actor,
				createdAt: '2026-10-20T01:00:00Z'
			});
			for (const allocation of payload.allocations) {
				const line = request!.lines.find((line) => line.planLineId === allocation.planLineId)!;
				line.paidSatang = (BigInt(line.paidSatang) - BigInt(allocation.amountSatang)).toString();
				if (request!.request.status === 'approved')
					line.reservedSatang = (
						BigInt(line.reservedSatang) + BigInt(allocation.amountSatang)
					).toString();
			}
			request!.request.paidSatang = (
				BigInt(request!.request.paidSatang) - BigInt(total)
			).toString();
			request!.request.remainingSatang = (
				BigInt(request!.request.totalSatang) - BigInt(request!.request.paidSatang)
			).toString();
			request!.request.revision++;
			return reply(route, request);
		}
		if (path === '/api/budget/adjustments') {
			if (method === 'GET') return reply(route, pageData(adjustments, url));
			const value: BudgetAdjustmentDetail = {
				...payload,
				id: id(870 + adjustments.length),
				revision: 1,
				ownerId: actor,
				status: 'draft',
				loanOutstandingSatang: '0',
				createdAt: '2026-10-20T01:00:00Z',
				capabilities: { ...workspace.capabilities, canEdit: true, canSubmit: true, canAct: false }
			};
			adjustments.push(value);
			return reply(route, value);
		}
		const adjustment = adjustments.find((item) => path.includes(item.id));
		if (adjustment) {
			if (path.endsWith('/submit')) {
				adjustment.status = 'submitted';
				adjustment.capabilities.canAct = true;
				adjustment.capabilities.canSubmit = false;
			} else if (path.endsWith('/actions')) {
				adjustment.status = 'approved';
				adjustment.capabilities.canAct = false;
				adjustment.loanOutstandingSatang =
					adjustment.kind === 'loan' ? adjustment.amountSatang : '0';
			}
			return reply(route, adjustment);
		}
		return reply(route, 'ไม่มี fixture สำหรับ ' + path, 404);
	});
	return {
		reads,
		writes,
		release,
		get workspace() {
			return workspace;
		},
		get request() {
			return request;
		},
		payments,
		retainedRows,
		adjustments,
		history,
		async asReviewer() {
			role = 'reviewer';
			syncCapabilities();
			await page.reload();
		},
		async asRequester() {
			role = 'requester';
			syncCapabilities();
			await page.reload();
		}
	};
}
