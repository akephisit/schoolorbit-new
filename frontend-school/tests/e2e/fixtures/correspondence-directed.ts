import type { Page } from '@playwright/test';
import { mockCorrespondence, correspondenceId, detailPath, stampTemplates } from './correspondence';
import { actor, firstStaff, id } from './staff-home-route-data';
import type {
	CorrespondenceDetail,
	CorrespondenceOptions,
	CorrespondenceRoutingSettings,
	DirectedSendCorrespondenceRequest,
	ProcessActionRequest,
	SaveCorrespondenceRoutingSettingsRequest
} from '../../../src/lib/api/correspondence';
const now = '2026-10-18T08:00:00Z';
export const parentAssignmentId = id(906),
	assignmentId = id(901),
	childAssignmentId = id(902);
export async function mockDirectedCorrespondence(
	page: Page,
	config: {
		scenario?: 'draft' | 'decide' | 'execute' | 'copy' | 'review' | 'branches' | 'recovery';
		inheritedReview?: boolean;
		failAction?: boolean;
		abortSend?: boolean;
		recommended?: boolean;
		recoverReviewer?: boolean;
		terminalCopy?: 'completed' | 'closed';
		bookStatus?: 'rejected' | 'revision_requested';
		historicalContext?: boolean;
		permissions?: string[];
	} = {}
) {
	const base = await mockCorrespondence(page, {
		mode: config.scenario === 'draft' ? 'draft' : 'acknowledge',
		permissions: config.permissions
	});
	const snapshots = [
		{
			userId: actor,
			displayName: 'ครูเจ้าของ ทดสอบ',
			completedAt: null,
			readAt: null,
			reassignedFrom: null
		},
		{
			userId: firstStaff,
			displayName: 'ครูผู้รับคนที่สอง',
			completedAt: null,
			readAt: null,
			reassignedFrom: null
		}
	];
	const backupReviewer = { ...snapshots[0], userId: id(908), displayName: 'ครูสำรอง ทดสอบ' };
	const settings: CorrespondenceRoutingSettings[] = (
		['incoming', 'outgoing', 'internal'] as const
	).map((register) => ({
		register,
		revision: 1,
		eligibleDecisionMakers: [],
		defaultFirstRecipient: config.recommended ? { kind: 'user', userId: firstStaff } : null,
		defaultPurpose: 'execute',
		defaultRequireReview: false,
		defaultReviewer: null,
		defaultAllowOnwardAssignment: false,
		numbering: { format: '{number}/{year}', minimumDigits: 1 },
		textStampTemplates: structuredClone(stampTemplates)
	}));
	const options: CorrespondenceOptions = {
		users: [
			{ id: id(908), displayName: 'ครูสำรอง ทดสอบ' },
			...snapshots.map((item) => ({ id: item.userId, displayName: item.displayName })),
			{ id: id(907), displayName: 'บุคลากรอ่านอย่างเดียว' }
		],
		organizations: [{ id: id(806), name: 'งานบริหารทั่วไป' }],
		positions: [{ organizationUnitId: id(806), code: 'HEAD', name: 'หัวหน้างาน' }],
		decisionMakers: [{ id: firstStaff, displayName: snapshots[1].displayName }],
		reviewers: [
			...snapshots.map((item) => ({ id: item.userId, displayName: item.displayName })),
			{ id: id(908), displayName: 'ครูสำรอง ทดสอบ' }
		],
		templates: [],
		routingSettings: settings,
		canManageRoutingSettings: true,
		stampContext: {
			schoolName: 'โรงเรียนตัวอย่าง',
			actorDisplayName: 'ครูเจ้าของ ทดสอบ',
			signerTitle: 'ครู'
		}
	};
	let detail = structuredClone(base.detail());
	function makeDirected(
		input: CorrespondenceDetail,
		purpose: 'decide' | 'execute' | 'acknowledge' = 'decide'
	): CorrespondenceDetail {
		const copied = config.scenario === 'copy';
		return {
			...input,
			record: { ...input.record, status: 'in_progress' },
			canManage: false,
			canAct: true,
			availableActions: copied
				? ['acknowledge']
				: purpose === 'decide'
					? ['route', 'review']
					: config.scenario === 'review'
						? ['review']
						: ['execute'],
			assignmentActions: [
				{
					assignmentId,
					actions: copied
						? ['acknowledge']
						: purpose === 'decide'
							? ['route', 'review']
							: config.scenario === 'review'
								? ['review']
								: ['execute']
				}
			],
			instance: {
				id: id(904),
				source: {
					moduleCode: 'correspondence',
					resourceType: 'correspondence',
					resourceId: correspondenceId
				},
				title: input.record.title,
				actionPath: detailPath,
				templateId: null,
				templateVersion: null,
				revision: input.record.revision,
				status: 'active',
				currentStep: 0,
				steps: [],
				createdBy: actor,
				createdAt: now,
				dueAt: null,
				directed: {
					assignments: [
						{
							id: assignmentId,
							parentId: null,
							selector: { kind: 'user', userId: actor },
							recipient: snapshots[0],
							purpose,
							status: config.scenario === 'review' ? 'waiting_review' : 'pending',
							required: !copied,
							allowOnwardAssignment: !copied,
							reviewer: config.inheritedReview
								? backupReviewer
								: config.scenario === 'review'
									? snapshots[1]
									: null,
							dueAt: null,
							workItemId: null,
							reviewWorkItemId: null,
							completedAt: null,
							assignedBy: firstStaff,
							createdAt: now
						}
					]
				}
			}
		};
	}
	if (config.scenario !== 'draft')
		detail = makeDirected(
			detail,
			['execute', 'review'].includes(config.scenario ?? '')
				? 'execute'
				: config.scenario === 'copy'
					? 'acknowledge'
					: 'decide'
		);
	if (config.inheritedReview && detail.instance?.directed) {
		const current = detail.instance.directed.assignments[0];
		const ancestor = {
			...current,
			id: parentAssignmentId,
			parentId: null,
			recipient: { ...snapshots[0], userId: id(909), displayName: 'ผู้สั่งการก่อนหน้า' },
			purpose: 'decide' as const,
			status: 'waiting_children' as const,
			reviewer: backupReviewer
		};
		detail = {
			...detail,
			availableActions: ['route', 'execute'],
			assignmentActions: [{ assignmentId, actions: ['route', 'execute'] }],
			instance: {
				...detail.instance,
				directed: {
					assignments: [
						ancestor,
						{ ...current, parentId: parentAssignmentId, purpose: 'execute', reviewer: null }
					]
				}
			}
		};
	}
	if (config.scenario === 'review' && detail.instance?.directed) {
		const current = detail.instance.directed.assignments[0];
		detail = {
			...detail,
			availableActions: ['review'],
			assignmentActions: [{ assignmentId, actions: ['review'] }],
			instance: {
				...detail.instance,
				directed: {
					assignments: [
						{
							...current,
							recipient: snapshots[1],
							purpose: 'decide',
							status: 'waiting_review',
							reviewer: snapshots[0]
						},
						{
							...current,
							id: childAssignmentId,
							parentId: assignmentId,
							recipient: { ...snapshots[0], userId: id(909), displayName: 'ผู้ดำเนินการ' },
							purpose: 'execute',
							status: 'completed',
							allowOnwardAssignment: false,
							reviewer: null,
							completedAt: now
						}
					]
				}
			}
		};
	}
	if (config.historicalContext) {
		detail = {
			...detail,
			record: { ...detail.record, revision: 5 },
			history: [
				{
					id: id(999),
					assignmentId: id(998),
					assignmentRecipientName: 'หัวหน้างานเดิม ณ วันสั่งการ',
					routedRecipients: [{ id: firstStaff, displayName: 'ผู้รับคำสั่งเดิม ณ วันส่ง' }],
					actorId: firstStaff,
					actorName: 'ผู้สั่งการเดิม',
					action: 'route',
					comment: 'คำสั่งก่อนส่งกลับแก้ไข',
					documentRevision: 1,
					annotations: [],
					evidenceFileIds: [],
					revision: 2,
					createdAt: now
				}
			]
		};
	}
	if (config.terminalCopy && detail.instance)
		detail = {
			...detail,
			record: { ...detail.record, status: config.terminalCopy },
			availableActions: ['read', 'acknowledge'],
			assignmentActions: [{ assignmentId, actions: ['read', 'acknowledge'] }],
			instance: { ...detail.instance, status: config.terminalCopy }
		};
	if (config.bookStatus && detail.instance)
		detail = {
			...detail,
			record: { ...detail.record, status: config.bookStatus },
			availableActions: [],
			assignmentActions: [],
			instance: { ...detail.instance, status: config.bookStatus }
		};
	if (config.scenario === 'recovery' && detail.instance?.directed) {
		detail = {
			...detail,
			canManage: true,
			availableActions: ['reassign'],
			assignmentActions: [{ assignmentId, actions: ['reassign'] }],
			instance: {
				...detail.instance,
				directed: {
					assignments: detail.instance.directed.assignments.map((item) => ({
						...item,
						purpose: 'execute',
						recipient: snapshots[1],
						status: config.recoverReviewer ? 'waiting_review' : 'pending',
						reviewer: config.recoverReviewer ? snapshots[0] : item.reviewer
					}))
				}
			}
		};
	}
	if (config.scenario === 'branches' && detail.instance?.directed) {
		const root = detail.instance.directed.assignments[0];
		detail = {
			...detail,
			availableActions: ['execute'],
			assignmentActions: [{ assignmentId: childAssignmentId, actions: ['execute'] }],
			instance: {
				...detail.instance,
				directed: {
					assignments: [
						{ ...root, status: 'waiting_children' },
						{
							...root,
							id: childAssignmentId,
							parentId: root.id,
							purpose: 'execute',
							allowOnwardAssignment: false
						},
						{
							...root,
							id: id(903),
							parentId: root.id,
							purpose: 'execute',
							recipient: snapshots[1],
							allowOnwardAssignment: false
						},
						{
							...root,
							id: id(905),
							parentId: root.id,
							purpose: 'acknowledge',
							recipient: snapshots[1],
							required: false,
							allowOnwardAssignment: false
						}
					]
				}
			}
		};
	}
	const sends: DirectedSendCorrespondenceRequest[] = [],
		actions: ProcessActionRequest[] = [],
		settingsWrites: SaveCorrespondenceRoutingSettingsRequest[] = [];
	let sendAccepted: CorrespondenceDetail | undefined;
	let acceptedSendPayload: string | undefined;
	await page.route(
		(url) =>
			(url.pathname === '/api/correspondence' && url.searchParams.has('limit')) ||
			url.pathname === '/api/correspondence/options' ||
			url.pathname === '/api/correspondence/routing-settings' ||
			url.pathname === `${detailPath.replace('/staff', '/api')}` ||
			url.pathname.endsWith('/directed-send') ||
			url.pathname.endsWith('/read') ||
			url.pathname === `/api/correspondence/${correspondenceId}/actions`,
		async (route) => {
			const req = route.request(),
				path = new URL(req.url()).pathname;
			const reply = (data: unknown, status = 200) =>
				route.fulfill({
					status,
					contentType: 'application/json',
					headers: { 'X-CSRF-Token': 'synthetic-csrf' },
					body: JSON.stringify(
						status < 400 ? { success: true, data } : { success: false, error: data }
					)
				});
			if (path === '/api/correspondence' && req.method() === 'GET')
				return reply({
					items: [detail.record],
					counts: { actionRequired: 1, sent: 1, drafts: 0, all: 1 },
					nextCursor: null,
					progress: [
						{
							correspondenceId,
							currentStepIndex: null,
							stepCount: detail.instance?.directed?.assignments.length ?? 0,
							currentStepTitle: 'กำลังดำเนินการตามการสั่งการ',
							pendingRecipients: [{ id: actor, displayName: snapshots[0].displayName }],
							dueAt: null,
							myAction: 'execute',
							canRevise: false
						}
					]
				});
			if (path.endsWith('/read')) {
				const payload = req.postDataJSON();
				base.readRequests.push(payload);
				const receipt = base.recordRead(payload.documentRevision);
				detail = { ...detail, myReadReceipt: receipt };
				return reply(receipt);
			}
			if (path === '/api/correspondence/options') return reply(options);
			if (path === '/api/correspondence/routing-settings') {
				if (req.method() === 'GET') return reply(settings);
				const payload = req.postDataJSON() as SaveCorrespondenceRoutingSettingsRequest;
				settingsWrites.push(payload);
				const previous = settings.find((item) => item.register === payload.register);
				const row: CorrespondenceRoutingSettings = {
					...payload,
					numbering: payload.numbering ??
						previous?.numbering ?? { format: '{number}/{year}', minimumDigits: 1 },
					textStampTemplates:
						payload.textStampTemplates ??
						previous?.textStampTemplates ??
						structuredClone(stampTemplates),
					revision: payload.expectedRevision + 1,
					defaultRequireReview:
						payload.defaultRequireReview ?? previous?.defaultRequireReview ?? false,
					defaultAllowOnwardAssignment:
						payload.defaultAllowOnwardAssignment ?? previous?.defaultAllowOnwardAssignment ?? true
				};
				settings.splice(
					settings.findIndex((item) => item.register === row.register),
					1,
					row
				);
				return reply(row);
			}
			if (req.method() === 'GET') return reply(detail);
			if (path.endsWith('/directed-send')) {
				const payload = req.postDataJSON() as DirectedSendCorrespondenceRequest;
				sends.push(payload);
				if (acceptedSendPayload && JSON.stringify(payload) !== acceptedSendPayload)
					return reply('คำขอเดิมต้องใช้ข้อมูลเดิม', 409);
				if (!sendAccepted) {
					detail = makeDirected(base.detail());
					sendAccepted = structuredClone(detail);
					acceptedSendPayload = JSON.stringify(payload);
				}
				if (config.abortSend && sends.length === 1) return route.abort('connectionreset');
				return reply(sendAccepted);
			}
			const action = req.postDataJSON() as ProcessActionRequest;
			actions.push(action);
			if (config.failAction && actions.length === 1)
				return reply('บริการไม่พร้อม ลองอีกครั้ง', 503);
			detail = {
				...detail,
				record: {
					...detail.record,
					revision: detail.record.revision + 1,
					status:
						config.terminalCopy ??
						(action.action === 'route' ||
						action.action === 'reassign' ||
						config.scenario === 'branches'
							? 'in_progress'
							: 'completed')
				},
				availableActions: [],
				assignmentActions: [],
				history: [
					...detail.history,
					{
						id: id(930 + actions.length),
						assignmentId: action.assignmentId,
						assignmentRecipientName:
							action.action === 'reassign' ? 'ครูเดิม ณ วันมอบหมาย' : snapshots[0].displayName,
						routedRecipients:
							action.action === 'route'
								? [{ id: firstStaff, displayName: snapshots[1].displayName }]
								: [],
						actorId: actor,
						actorName: snapshots[0].displayName,
						action: action.action,
						reassignedFromUserId:
							action.action === 'reassign' ? (config.recoverReviewer ? actor : firstStaff) : null,
						reassignedToUserId: action.reassignUserId ?? null,
						reassignedFromName: action.action === 'reassign' ? 'ครูเดิม ณ วันมอบหมาย' : null,
						reassignedToName: action.action === 'reassign' ? 'ครูสำรอง ณ วันมอบหมาย' : null,
						comment: action.comment,
						documentRevision: detail.document.revision,
						annotations: action.annotations ?? [],
						evidenceFileIds: action.evidenceFileIds ?? [],
						revision: detail.record.revision + 1,
						createdAt: now
					}
				]
			};
			if (detail.instance?.directed)
				detail.instance.directed.assignments = detail.instance.directed.assignments.map((item) => ({
					...item,
					status:
						config.scenario === 'branches'
							? item.id === action.assignmentId
								? 'completed'
								: item.status
							: action.action === 'reassign'
								? 'pending'
								: action.action === 'route'
									? 'waiting_children'
									: 'completed'
				}));
			return reply(detail);
		}
	);
	return { ...base, detail: () => detail, sends, actions, settingsWrites, options };
}
