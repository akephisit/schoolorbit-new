import type { Page, Route } from '@playwright/test';
import { PDFDocument, degrees } from 'pdf-lib';
import { mockStaffHome, actor, firstStaff, id } from './staff-home-route-data';
import type {
	CorrespondenceDetail,
	CorrespondenceList,
	ProcessTemplate,
	ProcessActionRequest,
	CreateCorrespondenceRequest,
	SendCorrespondenceRequest,
	ReviseCorrespondenceRequest,
	SaveProcessTemplateRequest
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
		holdSend?: boolean;
		narrowSearch?: boolean;
		secondTemplate?: boolean;
		holdActionResponse?: boolean;
		abortAcceptedAction?: boolean;
		acceptedAnnotationColor?: string;
		nextStepAfterAccept?: boolean;
		holdListRefresh?: boolean;
		failCreate?: boolean;
		recovery?: boolean;
		rotation?: number;
		failUploadAt?: number;
		failSend?: number;
		failRevision?: number;
		abortAcceptedCreate?: boolean;
		abortAcceptedRevision?: boolean;
		abortAcceptedSend?: boolean;
		listCounts?: CorrespondenceList['counts'];
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
		assignmentActions: [],
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
	// Nonpersonal transparent black scribble, large enough to inspect actual private-image placement.
	const png = Buffer.from(
		'iVBORw0KGgoAAAANSUhEUgAAAPAAAABQCAYAAAAnSfh8AAADeklEQVR4nO3aUW7lIAyF4W5h3mYHs/8VzqhSK42qBAM+xgb+T8pTEwPBJ7m97ccHAAAAAAAAAAAAAAAAMOPX7z9/v4/suQAYRICBTf0fXgIMbOZngAkxsBECDGyMAAObegovAQY2QYCBjRFgYGNvASbEQHGt8BLgXOwDTE+BpXFqYB9gemoSGicfn4RgemsSGicfv87ARIDrIsAwvTUHjZOPAMNEgGviLwIwWc1B4+QhwDAR4LoIMExWY9A4OfjHmot4NjYrwDRjGwG+hGdje66NaBya0caf9S7heToT4LoI8AW8H696r1E2Dh8JbW/3hvt1GAJ8JgJ8Ae8XHCPXqBon8kuZ6s09Mr+3c3d56EXu8xFaN2imSVYE2Dvf0dqeekqjc2udW3WNn3p6UtG3R3hatCfAM+Op5zxT8612hWaYbVTrvCrr+6YK7jUB731Cz9ZQnN97fcSDocKGexrROid7bT/n0LtGAv4lI8AjtUfGUz8YsjfVmsPOAY6+11cE3JpM72RnFjR7E0bm7KnbejBEb1xvw4yEs0qAqwRj+1D3DFwtwKNz7q37VHtmbIWRxlDsX/aaljd+p+0C3HOep85o3dmxouo+nafcrJmGUDzMMtZTNbQjUtc0MuCKAPdcp2pq6/yR+6HYMG8jvJ2v3OMRpwe3BE/Dtn4eNYeR81eEUdWQigavEGBCu5A6OJ4NUj1EVtVtXTsbOm+Tv10fcb9610BogyjeOK16nvko56x+U6vmFtHoqwNMcBMp3hiKelZd7xjW+ZEBGhlT1ew/a3keeE/nE9oCPDdb8ZS36irn3LouOkCtexTV8BEBJrTFqAOs2MiooI3UnZl3z5grm14x1sj8Ce5i6rBF1FQHredtODPn3jFXNv6qAEfMHQbVRrQCrJrf25zV880IU8RYynEJbVGqDYl6Mq8I8OrGzAiAd40Et6CooEUGuPoDp6pb1nkV9YZGB1hZ+6bwfrpprVeI2MyIJol8U97W1Det9Xi7Bzhivoqald201uNFbOaqoEXUVdSr7rb1YgJvSmBjK97sqrq4g/VFaNQXpVuKDrCqJvJ4A7X6yL5fQFN2QKof2fuDzWQ37KlH9r4iSXbjnXpk7yuSZDfeqUf2viJJduOdemTvKzaT3bDVj+z9AZqyA0KgAAcCBQAAAAAAAAAAAAAAAAAAAAAAZvwDYRb41aAiqn4AAAAASUVORK5CYII=',
		'base64'
	);
	const uploads: string[] = [],
		assetWrites: { label: string; fileId: string }[] = [];
	const templateWrites: SaveProcessTemplateRequest[] = [];
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
	const acceptedCommands = new Map<string, { payload: string; detail: CorrespondenceDetail }>();
	let revisionAttempts = 0;
	let sendAttempts = 0,
		uploadFailed = false;
	function remember(payload: { requestId: string }, result: CorrespondenceDetail) {
		acceptedCommands.set(payload.requestId, {
			payload: JSON.stringify(payload),
			detail: structuredClone(result)
		});
	}
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
				if (settings.failUploadAt === uploads.length && !uploadFailed) {
					uploadFailed = true;
					return reply(route, 'อัปโหลดไฟล์ไม่สำเร็จ ลองอีกครั้ง', 503);
				}
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
					organizations:
						settings.narrowSearch && url.searchParams.get('search')
							? []
							: [{ id: id(806), name: 'งานบริหารทั่วไป' }],
					positions:
						settings.narrowSearch && url.searchParams.get('search')
							? []
							: [{ organizationUnitId: id(806), code: 'HEAD', name: 'หัวหน้างาน' }],
					decisionMakers: [
						{ id: actor, displayName: 'ครูเจ้าของ ทดสอบ' },
						{ id: firstStaff, displayName: 'ครูผู้รับคนที่สอง' }
					],
					reviewers: [
						{ id: actor, displayName: 'ครูเจ้าของ ทดสอบ' },
						{ id: firstStaff, displayName: 'ครูผู้รับคนที่สอง' }
					],
					routingSettings: [],
					canManageRoutingSettings: true,
					templates: settings.secondTemplate
						? [
								template,
								{ ...template, id: id(855), code: 'alternate_fixture', title: 'แม่แบบทางเลือก' }
							]
						: [template]
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
				const matches =
					(!url.searchParams.get('search') ||
						detail.record.title.includes(url.searchParams.get('search')!)) &&
					(!url.searchParams.get('status') ||
						detail.record.status === url.searchParams.get('status')) &&
					(!url.searchParams.get('register') ||
						detail.record.register === url.searchParams.get('register'));
				const canRevise =
					detail.canManage && ['draft', 'revision_requested'].includes(detail.record.status);
				const action =
					detail.availableActions.find(
						(action) =>
							action === detail.instance?.steps[detail.instance.currentStep]?.definition.kind
					) ??
					detail.availableActions.find((action) => action === 'close') ??
					null;
				const counts = matches
					? {
							actionRequired: Number(
								!!action || (canRevise && detail.record.status === 'revision_requested')
							),
							sent: Number(detail.record.createdBy === actor && detail.record.status !== 'draft'),
							drafts: Number(detail.record.createdBy === actor && detail.record.status === 'draft'),
							all: 1
						}
					: { actionRequired: 0, sent: 0, drafts: 0, all: 0 };
				const view = url.searchParams.get('view') ?? 'all';
				const visible =
					matches &&
					(view === 'all' ||
						(view === 'action_required'
							? counts.actionRequired > 0
							: view === 'sent'
								? counts.sent > 0
								: counts.drafts > 0));
				const current =
					detail.instance?.status === 'active'
						? detail.instance.steps[detail.instance.currentStep]
						: undefined;
				return reply(route, {
					items: visible ? [detail.record] : [],
					nextCursor: null,
					counts: settings.listCounts ?? counts,
					progress: visible
						? [
								{
									correspondenceId,
									currentStepIndex: current?.stepIndex ?? null,
									stepCount: detail.instance?.steps.length ?? 0,
									currentStepTitle: current?.definition.title ?? null,
									pendingRecipients:
										current?.recipients
											.filter((item) => !item.completedAt)
											.map((item) => ({ id: item.userId, displayName: item.displayName })) ?? [],
									dueAt: current?.dueAt ?? null,
									myAction: action,
									canRevise
								}
							]
						: []
				});
			}
			if (path === `/api/correspondence/${correspondenceId}` && method === 'GET') {
				if (settings.holdDetail) await held;
				return reply(route, detail);
			}
			const payload = request.postDataJSON();
			writes.push({ path, payload });
			if (!path.endsWith('/actions')) {
				const accepted = acceptedCommands.get(payload.requestId);
				if (accepted)
					return JSON.stringify(payload) === accepted.payload
						? reply(route, accepted.detail)
						: reply(route, 'คำขอเดิมต้องใช้ข้อมูลเดิม', 409);
			}
			if (path === '/api/correspondence') {
				createAttempts += 1;
				if (settings.failCreate && createAttempts === 1)
					return reply(route, 'บันทึกไม่สำเร็จ กรุณาลองอีกครั้ง', 503);
				detail = {
					...detail,
					record: {
						...detail.record,
						...payload,
						documentNumber: payload.documentNumber ?? null,
						documentDate: payload.documentDate ?? null,
						organizationUnitId: payload.organizationUnitId ?? null,
						id: correspondenceId,
						status: 'draft'
					},
					document: { ...document, content: payload.content },
					canManage: true,
					canAct: false,
					availableActions: [],
					instance: null
				};
				remember(payload, detail);
				if (settings.abortAcceptedCreate && createAttempts === 1)
					return route.abort('connectionreset');
				return reply(route, detail);
			}
			if (path.endsWith('/revisions')) {
				if (
					new Set((payload.annotations ?? []).map((item: { id: string }) => item.id)).size !==
					(payload.annotations ?? []).length
				)
					return reply(route, 'หมายเหตุซ้ำในฉบับแก้ไข', 400);
				revisionAttempts += 1;
				if (settings.failRevision && revisionAttempts === 1) {
					if (settings.failRevision === 409) advanceDocument();
					return reply(route, 'ฉบับร่างมีการเปลี่ยนแปลง กรุณาตรวจสอบล่าสุด', settings.failRevision);
				}
				detail = {
					...detail,
					record: { ...detail.record, ...payload.metadata, revision: detail.record.revision + 1 },
					document: { ...detail.document, ...payload, revision: detail.document.revision + 1 }
				};
				remember(payload, detail);
				if (settings.abortAcceptedRevision) {
					settings.abortAcceptedRevision = false;
					return route.abort('connectionreset');
				}
				return reply(route, detail);
			}
			if (path.endsWith('/send')) {
				if (settings.holdSend) await held;
				sendAttempts += 1;
				if (settings.failSend && sendAttempts === 1) {
					if (settings.failSend === 409) advanceDocument();
					return reply(route, 'ส่งไม่สำเร็จ ลองอีกครั้ง', settings.failSend);
				}
				detail = {
					...detail,
					record: { ...detail.record, revision: detail.record.revision + 1, status: 'in_progress' },
					canManage: false,
					canAct: true,
					availableActions: ['acknowledge']
				};
				remember(payload, detail);
				if (settings.abortAcceptedSend && sendAttempts === 1) return route.abort('connectionreset');
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
