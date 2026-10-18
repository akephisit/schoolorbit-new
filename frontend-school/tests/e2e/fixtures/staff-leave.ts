import type { Page, Route } from '@playwright/test';
import { mockStaffHome, actor, firstStaff, id } from './staff-home-route-data';
import type {
	LeaveRequestDetail,
	LeavePolicyVersion,
	LeaveType,
	LeavePeriodInput,
	LeaveActionRequest,
	SaveLeavePolicyRequest
} from '../../../src/lib/api/staff-leave';
import { PERMISSIONS } from '../../../src/lib/permissions/registry.generated';

export const leaveId = id(700),
	secondLeaveId = id(701),
	leaveTypeId = id(710),
	exitTypeId = id(711),
	leavePolicyId = id(720);
export const leaveDetailPath = `/staff/leave/requests/${leaveId}`;
const base = '/api/staff-leave';
const now = '2026-10-18T03:00:00Z';
export const leavePermissions = [
	PERMISSIONS.STAFF_LEAVE_READ_OWN,
	PERMISSIONS.STAFF_LEAVE_CREATE_OWN,
	PERMISSIONS.STAFF_LEAVE_UPDATE_ASSIGNED,
	PERMISSIONS.STAFF_LEAVE_MANAGE_SCHOOL,
	PERMISSIONS.STAFF_LEAVE_DOWNLOAD_OWN
];

export async function mockLeave(
	page: Page,
	options: {
		permissions?: string[];
		role?: 'owner' | 'reviewer';
		status?: LeaveRequestDetail['request']['status'];
		hold?: 'requests' | 'detail' | 'types' | 'usage' | 'mutation';
		fail?: 'requests' | 'detail' | 'types' | 'usage' | 'options';
		empty?: boolean;
		missingPolicy?: boolean;
		requireSignature?: boolean;
		failMutation?: number;
	} = {}
) {
	await mockStaffHome(page, { permissions: options.permissions ?? leavePermissions });
	const types: LeaveType[] = [
		{ id: leaveTypeId, code: 'sick', name: 'ลาป่วย', kind: 'leave', archived: false, revision: 1 },
		{
			id: exitTypeId,
			code: 'exit',
			name: 'ออกนอกโรงเรียน',
			kind: 'exit',
			archived: false,
			revision: 1
		}
	];
	let policy: LeavePolicyVersion = {
		id: leavePolicyId,
		typeId: leaveTypeId,
		version: 1,
		typeRevision: 1,
		effectiveFrom: '2026-01-01',
		sourceReference: 'คำสั่งโรงเรียนทดสอบ 1/2569',
		personnelTypes: ['contract_employee'],
		enabled: true,
		unit: 'half_day',
		countingMode: 'working_days',
		workingWeekdays: [1, 2, 3, 4, 5],
		excludedDates: ['2026-10-23'],
		quotaYearStartMonth: 1,
		maxPerRequest: 10,
		annualLimit: 40,
		evidenceRequired: false,
		evidenceFromAmount: 6,
		minimumNoticeDays: null,
		steps: [
			{
				title: 'หัวหน้าฝ่ายตรวจสอบ',
				kind: 'approve',
				completionPolicy: 'all',
				recipients: [{ kind: 'user', userId: firstStaff }],
				requiresSignature: options.requireSignature ?? false,
				requiresComment: false,
				requiresEvidence: false,
				requiresDelegatedAuthority: false,
				dueAfterDays: null
			}
		],
		templateId: id(721),
		createdAt: now
	};
	let request: LeaveRequestDetail['request'] = {
		id: leaveId,
		typeId: leaveTypeId,
		typeName: 'ลาป่วย',
		kind: 'leave',
		ownerId: options.role === 'reviewer' ? firstStaff : actor,
		ownerName: 'ครูเจ้าของ คำขอทดสอบชื่อยาวเพื่อทดสอบการแสดงผล',
		status: options.status ?? 'draft',
		revision: 1,
		policyVersionId: leavePolicyId,
		personnelType: 'contract_employee',
		startDate: '2026-10-19',
		endDate: '2026-10-19',
		startTime: null,
		endTime: null,
		dayPortion: 'full_day',
		amount: 2,
		unit: 'half_day',
		reason: 'เข้ารับการตรวจรักษา',
		createdAt: now
	};
	let purpose: LeaveRequestDetail['processPurpose'] =
		options.status === 'cancellation_pending' ? 'cancellation' : 'request';
	const history: LeaveRequestDetail['history'] = [
		{
			id: id(730),
			actorId: request.ownerId,
			actorName: request.ownerName,
			action: 'created',
			revision: 1,
			comment: null,
			signature: null,
			contentHash: 'a'.repeat(64),
			createdAt: now
		}
	];
	let evidenceFileIds: string[] = [],
		currentStep = 0;
	const detail = (): LeaveRequestDetail => {
		const canEdit = ['draft', 'returned'].includes(request.status) && options.role !== 'reviewer';
		const active = ['submitted', 'cancellation_pending'].includes(request.status);
		return {
			request: { ...request },
			policy: { ...policy },
			processPurpose: purpose,
			history: structuredClone(history),
			evidenceFileIds: [...evidenceFileIds],
			contentHash: 'a'.repeat(64),
			canEdit,
			canSubmit: canEdit,
			canCancel:
				options.role !== 'reviewer' &&
				['draft', 'returned', 'submitted', 'approved'].includes(request.status),
			canAct: options.role === 'reviewer' && active,
			availableActions:
				options.role === 'reviewer' && active ? ['approve', 'reject', 'return_for_revision'] : [],
			instance:
				active || request.status === 'approved'
					? {
							id: id(740),
							source: {
								moduleCode: 'staff_leave',
								resourceType: 'staff_leave_request',
								resourceId: request.id
							},
							title: 'พิจารณาการลา',
							actionPath: leaveDetailPath,
							templateId: policy.templateId,
							templateVersion: policy.version,
							directed: null,
							revision: request.revision,
							status: active ? 'active' : 'completed',
							currentStep,
							steps: policy.steps.map((step, index) => ({
								stepIndex: index,
								definition: step,
								recipients: [
									{
										userId: firstStaff,
										displayName: 'ครูหัวหน้าฝ่าย ทดสอบ',
										completedAt: request.status === 'approved' ? now : null,
										readAt: null,
										reassignedFrom: null
									}
								],
								dueAt: null,
								workItemId: null,
								completedAt: request.status === 'approved' ? now : null
							})),
							createdBy: actor,
							createdAt: now,
							dueAt: null
						}
					: null
		};
	};
	const counts = new Map<string, number>(),
		writes: Array<{ path: string; method: string; payload: Record<string, unknown> }> = [];
	const uploads: Array<{ purpose: string; resourceId: string }> = [],
		fileReads: Array<{ path: string; resourceId: string | null }> = [];
	let release = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	let failedRead = false,
		failedMutation = false;
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			headers: { 'X-CSRF-Token': 'synthetic-csrf' },
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	function updatePeriod(period: LeavePeriodInput) {
		const type = types.find((type) => type.id === period.typeId)!;
		request = {
			...request,
			...period,
			typeName: type.name,
			kind: type.kind,
			unit: type.kind === 'exit' ? 'minute' : 'half_day',
			amount: type.kind === 'exit' ? 90 : period.dayPortion === 'full_day' ? 2 : 1,
			personnelType: 'contract_employee'
		};
		evidenceFileIds = period.evidenceFileIds ?? [];
		if (type.kind === 'exit')
			policy = {
				...policy,
				typeId: exitTypeId,
				unit: 'minute',
				countingMode: 'calendar_days',
				maxPerRequest: 120,
				annualLimit: null,
				evidenceFromAmount: null
			};
	}
	await page.route(
		(url) => url.pathname.startsWith(base) || url.pathname.startsWith('/api/files'),
		async (route) => {
			const http = route.request(),
				url = new URL(http.url()),
				path = url.pathname,
				method = http.method();
			if (method === 'GET') counts.set(path, (counts.get(path) ?? 0) + 1);
			else if (path.startsWith(base)) writes.push({ path, method, payload: http.postDataJSON() });
			if (path.startsWith('/api/files/') && path.endsWith('/download')) {
				fileReads.push({ path, resourceId: url.searchParams.get('resource_id') });
				return reply(route, {
					url: `${url.origin}/synthetic-leave-signature.png`,
					expiresAt: '2026-10-19T00:00:00Z'
				});
			}
			const region =
				method !== 'GET'
					? 'mutation'
					: path === `${base}/requests`
						? 'requests'
						: path === `${base}/types`
							? 'types'
							: path === `${base}/usage`
								? 'usage'
								: path === `${base}/options`
									? 'options'
									: path.includes('/requests/')
										? 'detail'
										: '';
			if (options.hold === region) await held;
			if (options.fail === region && !failedRead) {
				failedRead = true;
				return reply(route, `บริการ ${region} ไม่พร้อม`, 503);
			}
			if (method !== 'GET' && path.startsWith(base) && options.failMutation && !failedMutation) {
				failedMutation = true;
				if (options.failMutation === 409) request.revision++;
				return reply(
					route,
					options.failMutation === 409
						? 'ข้อมูลมีการเปลี่ยนแปลง กรุณาตรวจสอบข้อมูลล่าสุด'
						: 'บริการไม่พร้อม กรุณาลองอีกครั้ง',
					options.failMutation
				);
			}
			if (method === 'GET') {
				if (path === `${base}/types`) return reply(route, [...types]);
				if (path === `${base}/options`)
					return reply(route, {
						types: types.filter((type) => !type.archived),
						personnelType: 'contract_employee'
					});
				if (path === `${base}/routing-options`)
					return reply(route, {
						users: [{ id: firstStaff, displayName: 'ครูหัวหน้าฝ่าย ทดสอบ' }],
						organizations: [{ id: id(750), name: 'ฝ่ายบุคลากร' }],
						positions: [{ organizationUnitId: id(750), code: 'head', name: 'หัวหน้าฝ่าย' }]
					});
				if (path.endsWith('/effective-policy')) {
					if (options.missingPolicy)
						return reply(route, 'ยังไม่มีนโยบายสำหรับประเภทบุคลากรนี้', 404);
					return reply(
						route,
						path.includes(exitTypeId)
							? {
									...policy,
									typeId: exitTypeId,
									unit: 'minute',
									countingMode: 'calendar_days',
									maxPerRequest: 120,
									annualLimit: null,
									evidenceFromAmount: null
								}
							: policy
					);
				}
				if (path.endsWith('/policies')) return reply(route, [policy]);
				if (path === `${base}/requests`) {
					if (options.empty) return reply(route, { items: [], hasMore: false });
					const kind = url.searchParams.get('kind');
					const status = url.searchParams.get('status');
					const snapshot = {
						...request,
						...(kind === 'exit'
							? {
									typeId: exitTypeId,
									typeName: 'ออกนอกโรงเรียน',
									kind: 'exit',
									unit: 'minute',
									amount: 90,
									startTime: '09:00:00',
									endTime: '10:30:00'
								}
							: {})
					};
					return reply(route, {
						items: status && status !== snapshot.status ? [] : [snapshot],
						hasMore: !url.searchParams.has('offset') || url.searchParams.get('offset') === '0'
					});
				}
				if (path === `${base}/usage`)
					return reply(route, {
						from: url.searchParams.get('from'),
						to: url.searchParams.get('to'),
						items: options.empty
							? []
							: [
									{
										userId: actor,
										displayName: '=ชื่อสูตรทดสอบ',
										typeId: leaveTypeId,
										typeName: 'ลาป่วย',
										unit: 'half_day',
										quotaYearStart: '2026-01-01',
										approvedAmount: 3,
										reservedAmount: 1,
										annualLimit: 40,
										policyVersionId: leavePolicyId
									}
								]
					});
				if (path.endsWith(secondLeaveId))
					return reply(route, {
						...detail(),
						request: { ...request, id: secondLeaveId, typeName: 'คำขอคนละรายการ' }
					});
				return reply(route, detail());
			}
			if (path === '/api/files') {
				const payload = http.postDataBuffer()?.toString() ?? '';
				uploads.push({
					purpose: payload.match(/name="purpose"\r\n\r\n([^\r\n]+)/)?.[1] ?? '',
					resourceId: payload.match(/name="resource_id"\r\n\r\n([^\r\n]+)/)?.[1] ?? ''
				});
				return reply(route, { id: id(760), name: 'หลักฐานทดสอบ.png', status: 'ready' });
			}
			if (path === `${base}/types` || /^\/api\/staff-leave\/types\/[^/]+$/.test(path)) {
				const input = http.postDataJSON();
				const index = types.findIndex((type) => path.endsWith(type.id));
				if (method === 'DELETE') {
					if (index >= 0) types.splice(index, 1);
					return reply(route, {});
				}
				const saved = {
					...input,
					code: index >= 0 ? types[index].code : 'internal-generated-code',
					id: index >= 0 ? types[index].id : id(712),
					revision: index >= 0 ? types[index].revision + 1 : 1
				};
				if (index >= 0) types[index] = saved;
				else types.push(saved);
				return reply(route, saved);
			}
			if (path.endsWith('/policies')) {
				const input = http.postDataJSON() as SaveLeavePolicyRequest;
				const type = types.find((type) => path.includes(type.id))!;
				type.revision++;
				policy = {
					...policy,
					...input,
					id: id(722),
					version: policy.version + 1,
					typeRevision: type.revision
				};
				return reply(route, policy);
			}
			if (path === `${base}/requests` || method === 'PUT') updatePeriod(http.postDataJSON().period);
			if (path.endsWith('/submit')) request.status = 'submitted';
			if (path.endsWith('/cancel')) {
				purpose = request.status === 'approved' ? 'cancellation' : 'request';
				request.status = request.status === 'approved' ? 'cancellation_pending' : 'cancelled';
			}
			if (path.endsWith('/actions')) {
				const input = http.postDataJSON() as LeaveActionRequest;
				request.status =
					purpose === 'cancellation' && ['reject', 'return_for_revision'].includes(input.action)
						? 'approved'
						: input.action === 'reject'
							? 'rejected'
							: input.action === 'return_for_revision'
								? 'returned'
								: purpose === 'cancellation'
									? 'cancelled'
									: 'approved';
				history.push({
					id: id(731 + history.length),
					actorId: actor,
					actorName: 'ครูผู้พิจารณา ทดสอบ',
					action: `${purpose === 'cancellation' ? 'cancellation_' : ''}${input.action}`,
					revision: request.revision,
					comment: input.comment ?? null,
					signature: input.signature ?? null,
					contentHash: 'a'.repeat(64),
					createdAt: now
				});
			}
			request.revision++;
			return reply(route, detail());
		}
	);
	await page.route('**/synthetic-leave-signature.png', (route) =>
		route.fulfill({
			contentType: 'image/png',
			body: Buffer.from(
				'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg==',
				'base64'
			)
		})
	);
	return {
		release,
		writes,
		uploads,
		fileReads,
		count: (path: string) => counts.get(path) ?? 0,
		detail,
		setStatus: (status: LeaveRequestDetail['request']['status']) => (request.status = status),
		setCurrentStep: (index: number) => (currentStep = index)
	};
}
