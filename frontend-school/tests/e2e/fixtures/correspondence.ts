import type { Page, Route } from '@playwright/test';
import { PDFDocument, degrees } from 'pdf-lib';
import { mockStaffHome, actor, firstStaff, id } from './staff-home-route-data';
import type {
	CorrespondenceDetail,
	ProcessTemplate,
	ProcessActionRequest,
	CreateCorrespondenceRequest,
	SendCorrespondenceRequest,
	ReviseCorrespondenceRequest
} from '../../../src/lib/api/correspondence';

export const correspondenceId = id(800),
	templateId = id(801),
	signatureId = id(802),
	fileId = id(803);
export const detailPath = `/staff/correspondence/${correspondenceId}`;
const now = '2026-10-18T08:00:00Z';
export async function mockCorrespondence(
	page: Page,
	settings: {
		mode?: 'draft' | 'sign' | 'acknowledge';
		permissions?: string[];
		failAction?: number;
		holdDetail?: boolean;
		holdAction?: boolean;
		holdActionResponse?: boolean;
		abortAcceptedAction?: boolean;
		acceptedAnnotationColor?: string;
		nextStepAfterAccept?: boolean;
		holdListRefresh?: boolean;
		failCreate?: boolean;
		recovery?: boolean;
		rotation?: number;
	} = {}
) {
	const permissions = settings.permissions ?? [
		'correspondence.read.own',
		'correspondence.read.assigned',
		'correspondence.create.own',
		'correspondence.update.assigned',
		'correspondence.sign.own',
		'correspondence.download.assigned',
		'process.manage.school'
	];
	const base = await mockStaffHome(page, { permissions });
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
	const kind = settings.mode === 'acknowledge' ? 'acknowledge' : 'sign';
	let template: ProcessTemplate = {
		id: templateId,
		code: 'internal_fixture_code',
		title: 'เวียนหนังสือมาตรฐาน',
		version: 1,
		active: true,
		createdAt: now,
		steps: [
			{
				title: kind === 'sign' ? 'ลงนามหนังสือ' : 'รับทราบหนังสือ',
				kind,
				completionPolicy: 'all',
				recipients: [],
				requiresSignature: kind === 'sign',
				requiresComment: false,
				requiresEvidence: false,
				requiresDelegatedAuthority: false
			}
		]
	};
	const document = {
		revision: 1,
		content: 'เอกสารตัวอย่างสำหรับทดสอบ',
		originalFileId: null,
		attachmentFileIds: [],
		annotations: [],
		contentHash: 'a'.repeat(64),
		fileSnapshots: [],
		createdBy: actor,
		createdAt: now
	};
	let detail: CorrespondenceDetail = {
		record: {
			id: correspondenceId,
			register: 'incoming',
			registerNumber: 18,
			registerYear: 2569,
			title: 'หนังสือทดสอบการดำเนินงาน',
			sender: 'โรงเรียนตัวอย่าง',
			documentNumber: 'ศธ 001/2569',
			documentDate: '2026-10-18',
			urgency: 'normal',
			organizationUnitId: null,
			revision: 1,
			status: settings.mode === 'draft' ? 'draft' : 'in_progress',
			createdBy: actor,
			createdAt: now
		},
		sourceLinks: [],
		document,
		revisions: [document],
		instance:
			settings.mode === 'draft'
				? null
				: {
						id: id(804),
						source: {
							moduleCode: 'correspondence',
							resourceType: 'correspondence',
							resourceId: correspondenceId
						},
						title: 'หนังสือทดสอบการดำเนินงาน',
						actionPath: detailPath,
						templateId,
						templateVersion: 1,
						revision: 1,
						status: 'active',
						currentStep: 0,
						steps: [
							{
								stepIndex: 0,
								definition: template.steps[0],
								recipients: [
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
								],
								dueAt: null,
								workItemId: id(805),
								completedAt: null
							}
						],
						createdBy: actor,
						createdAt: now,
						dueAt: null
					},
		history: [],
		signatures: [],
		canManage: settings.mode === 'draft',
		canAct: settings.mode !== 'draft',
		availableActions: settings.mode === 'draft' ? [] : [kind, 'read']
	};
	if (settings.nextStepAfterAccept && detail.instance) {
		const first = detail.instance.steps[0];
		const nextDefinition = { ...first.definition, title: 'ลงนามขั้นถัดไป' };
		detail.instance.steps = [
			{ ...first, definition: { ...first.definition, completionPolicy: 'any' } },
			{
				...first,
				stepIndex: 1,
				definition: nextDefinition,
				recipients: [first.recipients[0]],
				workItemId: null
			}
		];
	}

	if (settings.recovery && detail.instance) {
		detail = {
			...detail,
			availableActions: ['reassign'],
			instance: {
				...detail.instance,
				steps: detail.instance.steps.map((step) => ({
					...step,
					recipients: step.recipients.map((recipient) =>
						recipient.userId === actor
							? { ...recipient, userId: id(21), displayName: 'ครูเดิมที่ย้ายงาน' }
							: recipient
					)
				}))
			}
		};
	}
	function applyReassignmentRoundTrip() {
		if (!detail.instance) throw new Error('Expected process instance');
		const first = detail.instance.steps[0].recipients[0];
		const second = detail.instance.steps[0].recipients[1];
		detail = {
			...detail,
			record: { ...detail.record, revision: 3 },
			sourceLinks: [
				{ label: 'แหล่งอ้างอิงแรก', url: 'https://school.example/source' },
				{ label: 'แหล่งอ้างอิงอีกชื่อ', url: 'https://school.example/source' }
			],
			document: { ...detail.document, attachmentFileIds: [id(870), id(870)] },
			availableActions: ['acknowledge', 'reassign'],
			instance: {
				...detail.instance,
				revision: 3,
				steps: detail.instance.steps.map((step) => ({
					...step,
					recipients: [
						{ ...first, completedAt: now },
						{ ...second, completedAt: now, reassignedFrom: actor },
						{ ...first, completedAt: null, reassignedFrom: firstStaff }
					]
				}))
			},
			history: [
				[actor, firstStaff],
				[firstStaff, actor]
			].map(([from, to], index) => ({
				id: id(860 + index),
				actorId: from,
				actorName: from === actor ? first.displayName : second.displayName,
				action: 'reassign',
				reassignedFromUserId: from,
				reassignedToUserId: to,
				documentRevision: detail.document.revision,
				annotations: [],
				evidenceFileIds: index === 0 ? [id(870), id(870)] : [],
				comment: 'ส่งต่อและรับคืนหน้าที่',
				createdAt: now,
				revision: index + 2
			}))
		};
	}

	function addCommittedAnnotation(text: string) {
		detail.history = [
			...detail.history,
			{
				id: id(850),
				actorId: actor,
				actorName: 'ครูเจ้าของ ทดสอบ',
				action: 'review',
				documentRevision: detail.document.revision,
				annotations: [
					{
						id: id(851),
						kind: 'text',
						page: 1,
						x: 0.1,
						y: 0.25,
						width: 0.6,
						height: 0.1,
						text,
						color: '#111827',
						strokes: []
					}
				],
				evidenceFileIds: [],
				createdAt: now,
				revision: detail.record.revision
			}
		];
	}

	function advanceDocument() {
		const current = {
			...detail.document,
			revision: detail.document.revision + 1,
			contentHash: 'c'.repeat(64),
			content: 'เอกสารฉบับใหม่จากผู้ส่ง'
		};
		detail = {
			...detail,
			record: { ...detail.record, revision: detail.record.revision + 1 },
			document: current,
			revisions: [...detail.revisions, current]
		};
	}

	const pdf = await PDFDocument.create();
	const pdfPage = pdf.addPage([400, 600]);
	pdfPage.drawText('Synthetic correspondence source', { x: 30, y: 550, size: 12 });
	if (settings.rotation) pdfPage.setRotation(degrees(settings.rotation));
	const pdfBytes = Buffer.from(await pdf.save());
	const png = Buffer.from(
		'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jZ3kAAAAASUVORK5CYII=',
		'base64'
	);
	const uploads: string[] = [],
		assetWrites: { label: string; fileId: string }[] = [];
	const templateWrites: { code: string }[] = [];
	let createAttempts = 0;
	const reads = new Map<string, number>(),
		writes: {
			path: string;
			payload:
				| ProcessActionRequest
				| CreateCorrespondenceRequest
				| SendCorrespondenceRequest
				| ReviseCorrespondenceRequest;
		}[] = [];
	let actionAttempts = 0,
		release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			contentType: 'application/json',
			headers: { 'X-CSRF-Token': 'synthetic-csrf' },
			body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
		});
	await page.route(
		(url) =>
			url.pathname.startsWith('/api/correspondence') ||
			url.pathname.startsWith('/api/process/') ||
			url.pathname.startsWith('/api/files') ||
			url.pathname === '/fixture-signature.png',
		async (route) => {
			const request = route.request(),
				url = new URL(request.url()),
				path = url.pathname,
				method = request.method();
			if (method === 'GET') reads.set(path, (reads.get(path) ?? 0) + 1);
			if (path === '/fixture-signature.png')
				return route.fulfill({ contentType: 'image/png', body: png });
			if (path.endsWith('/preview') || path.endsWith('/export'))
				return route.fulfill({ contentType: 'application/pdf', body: pdfBytes });
			if (path === '/api/files' && method === 'POST') {
				uploads.push(request.postData() ?? '');
				return reply(route, {
					id: id(840 + uploads.length),
					originalFilename: 'signature.png',
					mimeType: 'image/png',
					lifecycleStatus: 'ready'
				});
			}
			if (path.includes('/download'))
				return reply(route, { url: new URL('/fixture-signature.png', page.url()).href });
			if (path === '/api/correspondence/signatures') {
				if (method === 'GET')
					return reply(route, [
						{ id: signatureId, label: 'ลายมือชื่อส่วนตัว', fileId, createdAt: now }
					]);
				const payload = request.postDataJSON();
				assetWrites.push(payload);
				return reply(route, {
					id: id(845),
					label: payload.label,
					fileId: payload.fileId,
					createdAt: now
				});
			}
			if (path === '/api/correspondence/options')
				return reply(route, {
					users: [
						{ id: actor, displayName: 'ครูเจ้าของ ทดสอบ' },
						{ id: firstStaff, displayName: 'ครูผู้รับคนที่สอง' }
					],
					organizations: [{ id: id(806), name: 'งานบริหารทั่วไป' }],
					positions: [{ organizationUnitId: id(806), code: 'HEAD', name: 'หัวหน้างาน' }],
					templates: [template]
				});
			if (path === '/api/process/templates' && method === 'GET') return reply(route, [template]);
			if (path.startsWith('/api/process/templates') && method !== 'GET') {
				const payload = request.postDataJSON();
				templateWrites.push(payload);
				if (!/^[a-z0-9_]{1,100}$/.test(payload.code))
					return reply(route, 'ชื่อหรือรหัสแม่แบบไม่ถูกต้อง', 400);
				template = {
					...template,
					...payload,
					version: method === 'POST' ? 1 : template.version + 1
				};
				return reply(route, template);
			}
			if (path === '/api/correspondence' && method === 'GET') {
				if (settings.holdListRefresh && (reads.get(path) ?? 0) > 1) await held;
				return reply(route, {
					items:
						!url.searchParams.get('search') ||
						detail.record.title.includes(url.searchParams.get('search')!)
							? [detail.record]
							: [],
					nextCursor: null
				});
			}
			if (path === `/api/correspondence/${correspondenceId}` && method === 'GET') {
				if (settings.holdDetail) await held;
				return reply(route, detail);
			}
			const payload = request.postDataJSON();
			writes.push({ path, payload });
			if (path === '/api/correspondence') {
				createAttempts += 1;
				if (settings.failCreate && createAttempts === 1)
					return reply(route, 'บันทึกไม่สำเร็จ กรุณาลองอีกครั้ง', 503);
				detail = {
					...detail,
					record: { ...detail.record, ...payload, id: correspondenceId, status: 'draft' },
					document: { ...document, content: payload.content },
					canManage: true,
					canAct: false,
					availableActions: [],
					instance: null
				};
				return reply(route, detail);
			}
			if (path.endsWith('/revisions')) {
				detail = {
					...detail,
					record: { ...detail.record, revision: detail.record.revision + 1 },
					document: { ...detail.document, ...payload, revision: detail.document.revision + 1 }
				};
				return reply(route, detail);
			}
			if (path.endsWith('/send')) {
				detail = {
					...detail,
					record: { ...detail.record, revision: detail.record.revision + 1, status: 'in_progress' },
					canManage: false,
					canAct: true,
					availableActions: ['acknowledge']
				};
				return reply(route, detail);
			}
			if (path.endsWith('/actions')) {
				if (settings.holdAction) await held;
				actionAttempts += 1;
				if (settings.failAction && actionAttempts === 1) {
					if (settings.failAction === 409) advanceDocument();
					return reply(
						route,
						settings.failAction === 409
							? 'เอกสารมีการเปลี่ยนแปลง กรุณาตรวจสอบฉบับล่าสุด'
							: 'บริการไม่พร้อม ลองอีกครั้ง',
						settings.failAction
					);
				}
				const action = payload as ProcessActionRequest;
				const committedIds = new Set(
					[
						...detail.document.annotations,
						...detail.history
							.filter((entry) => entry.documentRevision === detail.document.revision)
							.flatMap((entry) => entry.annotations)
					].map((annotation) => annotation.id)
				);
				if ((action.annotations ?? []).some((annotation) => committedIds.has(annotation.id)))
					return reply(route, 'หมายเหตุนี้ถูกบันทึกในฉบับเอกสารแล้ว', 400);
				detail = {
					...detail,
					record: { ...detail.record, revision: detail.record.revision + 1 },
					availableActions: action.action === 'read' ? [kind] : [],
					history: [
						...detail.history,
						{
							id: id(820 + actionAttempts),
							actorId: actor,
							actorName: 'ครูเจ้าของ ทดสอบ',
							action: action.action,
							reassignedFromUserId: action.reassignFromUserId,
							reassignedToUserId: action.reassignUserId,
							documentRevision: detail.document.revision,
							annotations: (action.annotations ?? []).map((annotation) => ({
								...annotation,
								color: settings.acceptedAnnotationColor ?? annotation.color
							})),
							comment: action.comment ?? null,
							evidenceFileIds: action.evidenceFileIds ?? [],
							createdAt: now,
							revision: detail.record.revision + 1
						}
					],
					signatures: action.signature
						? [
								...detail.signatures,
								{
									id: id(830 + actionAttempts),
									signerId: actor,
									signerName: 'ครูเจ้าของ ทดสอบ',
									signerRole: 'ครู',
									signedAt: now,
									signedContentHash: 'b'.repeat(64),
									signature: action.signature,
									fileId: action.signature.method === 'image' ? fileId : null,
									actionId: id(820 + actionAttempts)
								}
							]
						: detail.signatures
				};
				if (detail.instance && action.action !== 'read')
					detail = {
						...detail,
						instance: {
							...detail.instance,
							steps: detail.instance.steps.map((step) => ({
								...step,
								recipients: step.recipients.map((recipient) =>
									step.stepIndex === detail.instance?.currentStep && recipient.userId === actor
										? { ...recipient, completedAt: now }
										: recipient
								)
							}))
						}
					};
				if (settings.nextStepAfterAccept && actionAttempts === 1 && detail.instance)
					detail = {
						...detail,
						availableActions: [kind],
						instance: {
							...detail.instance,
							currentStep: 1,
							steps: detail.instance.steps.map((step) =>
								step.stepIndex === 0
									? { ...step, completedAt: now }
									: { ...step, workItemId: id(807) }
							)
						}
					};
				const accepted = structuredClone(detail);
				if (settings.holdActionResponse) await held;
				if (settings.abortAcceptedAction && actionAttempts === 1)
					return route.abort('connectionreset');
				return reply(route, accepted);
			}
			return reply(route, 'Unexpected synthetic request', 400);
		}
	);
	return {
		...base,
		reads,
		uploads,
		assetWrites,
		templateWrites,
		writes,
		release,
		advanceDocument,
		applyReassignmentRoundTrip,
		addCommittedAnnotation,
		detail: () => detail,
		count: (path: string) => reads.get(path) ?? 0
	};
}
