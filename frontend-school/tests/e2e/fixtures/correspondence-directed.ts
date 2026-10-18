import type { Page } from '@playwright/test';
import { mockCorrespondence, correspondenceId, detailPath } from './correspondence';
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
export const assignmentId = id(901),
	childAssignmentId = id(902);
export async function mockDirectedCorrespondence(
	page: Page,
	config: {
		scenario?: 'draft' | 'decide' | 'execute' | 'copy' | 'review';
		inheritedReview?: boolean;
		failAction?: boolean;
		abortSend?: boolean;
		recommended?: boolean;
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
		defaultAllowOnwardAssignment: false
	}));
	const options: CorrespondenceOptions = {
		users: snapshots.map((item) => ({ id: item.userId, displayName: item.displayName })),
		organizations: [{ id: id(806), name: 'งานบริหารทั่วไป' }],
		positions: [{ organizationUnitId: id(806), code: 'HEAD', name: 'หัวหน้างาน' }],
		decisionMakers: [{ id: firstStaff, displayName: snapshots[1].displayName }],
		reviewers: snapshots.map((item) => ({ id: item.userId, displayName: item.displayName })),
		templates: [],
		routingSettings: settings,
		canManageRoutingSettings: true
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
							reviewer:
								config.inheritedReview || config.scenario === 'review' ? snapshots[1] : null,
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
	const sends: DirectedSendCorrespondenceRequest[] = [],
		actions: ProcessActionRequest[] = [],
		settingsWrites: SaveCorrespondenceRoutingSettingsRequest[] = [];
	let sendAccepted: CorrespondenceDetail | undefined;
	await page.route(
		(url) =>
			url.pathname === '/api/correspondence/options' ||
			url.pathname === '/api/correspondence/routing-settings' ||
			url.pathname === `${detailPath.replace('/staff', '/api')}` ||
			url.pathname.endsWith('/directed-send') ||
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
			if (path === '/api/correspondence/options') return reply(options);
			if (path === '/api/correspondence/routing-settings') {
				if (req.method() === 'GET') return reply(settings);
				const payload = req.postDataJSON() as SaveCorrespondenceRoutingSettingsRequest;
				settingsWrites.push(payload);
				const row: CorrespondenceRoutingSettings = {
					...payload,
					revision: payload.expectedRevision + 1,
					defaultRequireReview: payload.defaultRequireReview ?? false,
					defaultAllowOnwardAssignment: payload.defaultAllowOnwardAssignment ?? false
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
				if (!sendAccepted) {
					detail = makeDirected(base.detail());
					sendAccepted = structuredClone(detail);
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
					status: action.action === 'route' ? 'in_progress' : 'completed'
				},
				availableActions: [],
				assignmentActions: [],
				history: [
					...detail.history,
					{
						id: id(930 + actions.length),
						assignmentId: action.assignmentId,
						actorId: actor,
						actorName: snapshots[0].displayName,
						action: action.action,
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
					status: action.action === 'route' ? 'waiting_children' : 'completed'
				}));
			return reply(detail);
		}
	);
	return { ...base, detail: () => detail, sends, actions, settingsWrites, options };
}
