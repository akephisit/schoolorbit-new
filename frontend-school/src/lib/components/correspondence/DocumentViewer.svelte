<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import { previewCorrespondence } from '#lib/api/correspondence.js';
	import type {
		DocumentAnnotation,
		CorrespondenceDetail,
		SignatureInput
	} from '#lib/api/correspondence.js';
	import { downloadFile } from '#lib/api/files.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import type { PdfPreview } from '#lib/files/document-preview.js';
	import {
		pointerPoint,
		pathData,
		type PagePoint,
		type PageRotation
	} from '#lib/correspondence/geometry.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';
	import SignaturePlacement from './SignaturePlacement.svelte';
	import type { InkPath } from '#lib/correspondence/geometry.js';
	import AnnotationText from './AnnotationText.svelte';
	import TextAnnotationPlacement from './TextAnnotationPlacement.svelte';
	import TextStampPicker from './TextStampPicker.svelte';
	import FieldError from './FieldError.svelte';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { correspondenceUuidV7 } from '#lib/correspondence/uuid-v7.js';
	import { focusCorrespondenceField } from '#lib/correspondence/validation.js';
	import {
		fitAnnotationBox,
		loadAnnotationTextMeasure,
		type TextMeasure
	} from '#lib/correspondence/text-layout.js';
	let {
		detail,
		draft = $bindable([]),
		editable = false,
		disabled = false,
		placement,
		onplacement,
		onplacementchange,
		placementPaths = [],
		placementFileId,
		onrendered,
		readContext = ''
	}: {
		detail: CorrespondenceDetail;
		draft?: DocumentAnnotation[];
		editable?: boolean;
		disabled?: boolean;
		placement?: SignatureInput;
		onplacement?: (point: PagePoint, page: number) => void;
		onplacementchange?: (placement: SignatureInput) => void;
		placementPaths?: InkPath[];
		placementFileId?: string;
		onrendered?: (document: { correspondenceId: string; documentRevision: number } | null) => void;
		readContext?: string;
	} = $props();
	let canvas = $state<HTMLCanvasElement>();
	let overlay: SVGSVGElement;
	let pdf = $state<PdfPreview | null>(null),
		loading = $state(false),
		error = $state(''),
		pageNumber = $state(1),
		aspect = $state(0.707),
		opened = $state(false),
		fullscreen = $state(false);
	let pageWidth = $state(595),
		pageHeight = $state(842);
	let measure = $state<TextMeasure>((text, size) => text.length * size * 0.5);
	let rotation = $state<PageRotation>(0);
	let selectedTextId = $state(''),
		annotationError = $state(''),
		editTextOpen = $state(false),
		editText = $state(''),
		editBorder = $state(false);
	const selectedText = $derived(
		draft.find(
			(annotation) =>
				annotation.id === selectedTextId &&
				annotation.kind === 'text' &&
				annotation.page === pageNumber
		)
	);
	let toolsOpen = $state(false);
	let tool = $state('view'),
		text = $state(''),
		drawing = $state<PagePoint[]>([]),
		start = $state<PagePoint | null>(null),
		active = $state<number | null>(null);
	$effect.pre(() => {
		const enabled = !!placement;
		untrack(() => {
			if (enabled) {
				toolsOpen = true;
				if (tool === 'view') tool = 'placement';
			}
		});
	});
	const request = new LatestRequest();
	const layers = $derived.by(() => {
		const accepted = [
			...detail.document.annotations,
			...detail.history
				.filter((entry) => entry.documentRevision === detail.document.revision)
				.flatMap((entry) => entry.annotations)
		];
		const unique = new SvelteMap<string, DocumentAnnotation>();
		// A refresh may observe acceptance before the action response reaches this browser.
		// Reconcile the visible layer only; local drafts and signature consent keep their owners.
		for (const annotation of [...accepted, ...draft]) {
			if (!unique.has(annotation.id)) unique.set(annotation.id, annotation);
		}
		return [...unique.values()].filter((annotation) => annotation.page === pageNumber);
	});
	const pageCount = $derived(pdf?.pageCount ?? 1);
	function cleanup() {
		renderedDocument = '';
		onrendered?.(null);
		request.abort();
		opened = false;
		if (pdf) {
			void pdf.dispose();
			pdf = null;
		}
	}
	onDestroy(cleanup);
	let viewedDocument = '',
		renderedDocument = '';
	$effect.pre(() => {
		const key = `${detail.record.id}:${detail.document.revision}`;
		untrack(() => {
			if (viewedDocument !== key) {
				cleanup();
				viewedDocument = key;
				opened = false;
				pageNumber = 1;
				void openDocument();
			}
		});
	});
	function renderPage(next: number) {
		pageNumber = next;
	}
	$effect(() => {
		const target = canvas,
			current = pdf,
			selected = pageNumber,
			documentKey = viewedDocument,
			readKey = `${viewedDocument}:${readContext}`,
			documentRevision = detail.document.revision,
			correspondenceId = detail.record.id;
		if (!target || !current) return;
		const controller = new AbortController();
		void Promise.all([
			current.renderPage(selected, target, controller.signal),
			current.pageSize(selected)
		])
			.then(([, size]) => {
				if (!controller.signal.aborted) {
					pageWidth = size.width;
					pageHeight = size.height;
					aspect = size.width / size.height;
					if (selected === 1 && documentKey === viewedDocument && renderedDocument !== readKey) {
						renderedDocument = readKey;
						onrendered?.({ correspondenceId, documentRevision });
					}
				}
			})
			.catch((cause) => {
				if (!controller.signal.aborted && !isAbortError(cause))
					error = cause instanceof Error ? cause.message : 'แสดงหน้าเอกสารไม่สำเร็จ';
			});
		return () => controller.abort();
	});

	async function openDocument() {
		cleanup();
		const ticket = request.begin();
		loading = true;
		error = '';
		try {
			const [blob, fontMeasure] = await Promise.all([
				previewCorrespondence(detail.record.id, detail.document.revision, {
					signal: ticket.signal
				}),
				loadAnnotationTextMeasure()
			]);
			if (!request.isCurrent(ticket.revision)) return;
			measure = fontMeasure;
			if (blob.type.includes('pdf')) {
				const { loadPdfPreview } = await import('#lib/files/document-preview.js');
				const loaded = await loadPdfPreview(blob, ticket.signal);
				if (!request.isCurrent(ticket.revision)) {
					await loaded.dispose();
					return;
				}
				pdf = loaded;
				opened = true;
				pageNumber = 1;
			} else throw new Error('แสดงเอกสารไม่สำเร็จ');
		} catch (cause) {
			if (request.isCurrent(ticket.revision) && !isAbortError(cause))
				error = cause instanceof Error ? cause.message : 'เปิดเอกสารไม่สำเร็จ';
		} finally {
			if (request.isCurrent(ticket.revision)) loading = false;
		}
	}
	function textBoxError(annotation: DocumentAnnotation): string | null {
		if (!(annotation.text ?? '').trim() || [...(annotation.text ?? '')].length > 5000)
			return 'ระบุข้อความไม่เกิน 5,000 ตัวอักษร';
		return fitAnnotationBox(
			annotation.text ?? '',
			annotation.width * pageWidth,
			annotation.height * pageHeight,
			measure,
			annotation.border ?? false
		).overflow
			? 'ข้อความยาวเกินกรอบ กรุณาขยายกรอบหรือลดข้อความ'
			: null;
	}
	function changeTextBox(annotation: DocumentAnnotation) {
		if (disabled || !opened) return;
		const invalid = textBoxError(annotation);
		annotationError = invalid ?? '';
		if (!invalid) draft = draft.map((item) => (item.id === annotation.id ? annotation : item));
	}
	function insertTextBox(
		value: string,
		border: boolean,
		point: PagePoint,
		width: number,
		height: number
	): string | null {
		if (disabled || !opened) return 'รอเปิดเอกสารก่อนวางข้อความ';
		const annotation: DocumentAnnotation = {
			id: correspondenceUuidV7(),
			kind: 'text',
			textLayoutVersion: 1,
			page: pageNumber,
			x: point.x,
			y: point.y,
			width: Math.min(width, 1 - point.x),
			height: Math.min(height, 1 - point.y),
			text: value,
			color: '#111827',
			strokes: [],
			...(border ? { border: true } : {})
		};
		const invalid = textBoxError(annotation);
		if (invalid) return invalid;
		draft = [...draft, annotation];
		selectedTextId = annotation.id;
		annotationError = '';
		return null;
	}
	function addText(point: PagePoint = { x: 0.1, y: 0.1 }) {
		if (!text.trim()) return;
		annotationError = insertTextBox(text.trim(), false, point, 0.6, 0.08) ?? '';
		if (!annotationError) text = '';
	}
	function saveText(event: SubmitEvent) {
		event.preventDefault();
		if (!selectedText || disabled) return;
		const annotation = {
			...selectedText,
			textLayoutVersion: 1,
			text: editText.trim(),
			border: editBorder ? true : undefined
		};
		annotationError = textBoxError(annotation) ?? '';
		if (annotationError) {
			void focusCorrespondenceField('edit-annotation-text');
			return;
		}
		changeTextBox(annotation);
		editTextOpen = false;
	}
	function begin(event: PointerEvent) {
		if (
			!editable ||
			disabled ||
			!opened ||
			tool === 'view' ||
			active !== null ||
			(event.pointerType === 'mouse' && event.button !== 0)
		)
			return;
		const point = pointerPoint(
			event.clientX,
			event.clientY,
			overlay.getBoundingClientRect(),
			rotation
		);
		if (tool === 'placement') {
			onplacement?.(point, pageNumber);
			return;
		}
		if (tool === 'text') {
			addText(point);
			return;
		}
		event.preventDefault();
		overlay.setPointerCapture(event.pointerId);
		active = event.pointerId;
		start = point;
		drawing = [point];
	}
	function move(event: PointerEvent) {
		if (active === event.pointerId)
			drawing = [
				...drawing,
				pointerPoint(event.clientX, event.clientY, overlay.getBoundingClientRect(), rotation)
			];
	}
	function finish(event: PointerEvent) {
		if (active !== event.pointerId || !start) return;
		move(event);
		const last = drawing.at(-1)!;
		const x = Math.min(start.x, last.x),
			y = Math.min(start.y, last.y);
		if (tool === 'ink' && drawing.length > 1)
			draft = [
				...draft,
				{
					id: correspondenceUuidV7(),
					kind: 'ink',
					page: pageNumber,
					x: 0,
					y: 0,
					width: 1,
					height: 1,
					color: '#111827',
					strokes: [{ points: drawing, width: 0.003, color: '#111827' }]
				}
			];
		else if (Math.abs(last.x - start.x) > 0.003 && Math.abs(last.y - start.y) > 0.003)
			draft = [
				...draft,
				{
					id: correspondenceUuidV7(),
					kind: tool === 'circle' ? 'circle' : 'highlight',
					page: pageNumber,
					x,
					y,
					width: Math.abs(last.x - start.x),
					height: Math.abs(last.y - start.y),
					color: tool === 'circle' ? '#dc2626' : '#facc15',
					strokes: []
				}
			];
		active = null;
		start = null;
		drawing = [];
		if (overlay.hasPointerCapture(event.pointerId)) overlay.releasePointerCapture(event.pointerId);
	}
	async function downloadOriginal() {
		const id = detail.document.originalFileId;
		if (!id) return;
		try {
			const blob = await downloadFile(id, detail.record.id);
			const { saveBlob } = await import('#lib/correspondence/presentation.js');
			saveBlob(blob, 'เอกสารต้นฉบับ');
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'ดาวน์โหลดไม่สำเร็จ';
		}
	}
</script>

{#snippet workspace()}
	<div class="space-y-3" data-testid="document-workspace">
		<div class="flex flex-wrap items-center gap-2">
			{#if detail.document.originalFileId}<Button
					type="button"
					variant="ghost"
					onclick={downloadOriginal}>ต้นฉบับ</Button
				>{/if}
			<Button
				type="button"
				variant="outline"
				disabled={pageNumber <= 1}
				onclick={() => renderPage(pageNumber - 1)}>หน้าก่อน</Button
			><span class="text-sm" aria-live="polite">หน้า {pageNumber} / {pageCount}</span><Button
				type="button"
				variant="outline"
				disabled={pageNumber >= pageCount}
				onclick={() => renderPage(pageNumber + 1)}>หน้าถัดไป</Button
			>
			<Button
				type="button"
				variant="ghost"
				onclick={() => {
					rotation = ((rotation + 90) % 360) as PageRotation;
				}}>หมุน 90°</Button
			>
			<Button
				type="button"
				variant="ghost"
				onclick={() => {
					fullscreen = !fullscreen;
				}}>{fullscreen ? 'กลับสู่รายละเอียด' : 'เต็มหน้าจอ'}</Button
			>
		</div>
		{#if editable && opened}<details
				bind:open={toolsOpen}
				class="space-y-3 rounded-xl border bg-card p-3"
				data-testid="annotation-tools"
			>
				<summary class="cursor-pointer text-sm font-medium">เขียนข้อความ / ทำหมายเหตุ</summary>
				<TextStampPicker
					documentRevision={detail.document.revision}
					record={detail.record}
					{disabled}
					oninsert={(value, border) => insertTextBox(value, border, { x: 0.1, y: 0.1 }, 0.55, 0.22)}
				/>
				<ToggleGroup.Root
					type="single"
					value={tool}
					onValueChange={(value) => {
						if (value) tool = value;
					}}
					aria-label="เครื่องมือหมายเหตุ"
					{disabled}
					class="flex flex-wrap gap-1"
				>
					{#each [{ value: 'view', label: 'เลื่อนอ่าน' }, { value: 'text', label: 'ข้อความ' }, { value: 'ink', label: 'ปากกา' }, { value: 'highlight', label: 'เน้นข้อความ' }, { value: 'circle', label: 'วงกลม' }, ...(onplacement ? [{ value: 'placement', label: 'วางลายมือชื่อ' }] : [])] as item (item.value)}<ToggleGroup.Item
							value={item.value}>{item.label}</ToggleGroup.Item
						>{/each}
				</ToggleGroup.Root>
				{#if tool === 'text'}<div class="flex flex-wrap items-end gap-2">
						<div class="min-w-40 flex-1 space-y-2">
							<Label for="annotation-text">ข้อความกำกับ</Label><Input
								id="annotation-text"
								bind:value={text}
								maxlength={5000}
								{disabled}
							/>
						</div>
						<Button
							type="button"
							variant="outline"
							onclick={() => addText()}
							disabled={disabled || !text.trim()}>เพิ่มข้อความ</Button
						>
					</div>
					<p class="text-sm text-muted-foreground">
						คลิกบนหน้าเพื่อวางข้อความ หรือกดเพิ่มข้อความเพื่อวางมุมบน
					</p>{/if}
				{#if selectedText}<div class="flex flex-wrap items-center gap-2">
						<span class="text-sm font-medium">กรอบข้อความที่เลือก</span><Button
							type="button"
							variant="outline"
							size="sm"
							{disabled}
							onclick={() => {
								editText = selectedText.text ?? '';
								editBorder = selectedText.border ?? false;
								annotationError = '';
								editTextOpen = true;
							}}>แก้ไขข้อความ</Button
						><Button
							type="button"
							variant="ghost"
							size="sm"
							{disabled}
							onclick={() => {
								draft = draft.filter((item) => item.id !== selectedTextId);
								selectedTextId = '';
							}}>ลบกรอบข้อความ</Button
						><Button type="button" variant="ghost" size="sm" onclick={() => (selectedTextId = '')}
							>เสร็จสิ้นการจัดวาง</Button
						>
					</div>
					<p class="text-xs text-muted-foreground">
						ลากกรอบเพื่อย้าย หรือลากมุมเพื่อปรับขนาด ใช้ Tab และปุ่มลูกศรได้
					</p>{/if}
				{#if annotationError && !editTextOpen}<p role="alert" class="text-sm text-destructive">
						{annotationError}
					</p>{/if}
				{#if tool === 'placement'}<p class="text-sm text-muted-foreground">
						แตะเพื่อวาง ลากกรอบเพื่อเลื่อน หรือลากมุมเพื่อปรับขนาด ใช้ Tab
						เลือกกรอบแล้วกดปุ่มลูกศรได้
					</p>{/if}
				{#if draft.length}<Button
						type="button"
						variant="outline"
						size="sm"
						{disabled}
						onclick={() => {
							if (!disabled) draft = draft.slice(0, -1);
						}}>เลิกทำหมายเหตุล่าสุด</Button
					>{/if}
			</details>{/if}
		{#if error}<PageState
				variant="error"
				title={error}
				actionLabel="ลองเปิดเอกสารอีกครั้ง"
				onaction={openDocument}
			/>{/if}
		{#if loading}<div role="status" aria-label="กำลังเปิดเอกสาร">
				<PageSkeleton variant="form" rows={4} />
			</div>{/if}
		<div class="overflow-auto rounded-xl border bg-muted/30 p-3 sm:p-5">
			<div
				class="relative mx-auto w-full max-w-3xl bg-white text-black shadow-sm"
				data-testid="document-page"
				style:aspect-ratio={aspect}
				style:transform={`rotate(${rotation}deg)`}
				style:margin-block={rotation === 90 || rotation === 270 ? '25%' : '0'}
			>
				{#if pdf}<canvas
						bind:this={canvas}
						class="block h-full w-full"
						aria-label={`เอกสารหน้า ${pageNumber}`}
					></canvas>{:else}<div class="p-8 text-sm text-muted-foreground">
						{loading ? 'กำลังเตรียมหน้าเอกสาร' : 'ยังแสดงหน้าเอกสารไม่ได้ กรุณาลองเปิดอีกครั้ง'}
					</div>{/if}

				<svg
					bind:this={overlay}
					viewBox={`0 0 ${pageWidth} ${pageHeight}`}
					class="absolute inset-0 h-full w-full"
					class:touch-none={editable && opened && !disabled && tool !== 'view'}
					class:pointer-events-none={!editable || !opened || disabled || tool === 'view'}
					role="img"
					aria-label="ชั้นหมายเหตุบนเอกสาร"
					onpointerdown={begin}
					onpointermove={move}
					onpointerup={finish}
					onpointercancel={() => {
						active = null;
						drawing = [];
						start = null;
					}}
				>
					{#each opened ? layers : [] as annotation (annotation.id)}
						{#if annotation.kind === 'text'}<AnnotationText
								text={annotation.text ?? ''}
								x={annotation.x * pageWidth}
								y={annotation.y * pageHeight}
								width={annotation.width * pageWidth}
								height={annotation.height * pageHeight}
								color={annotation.color}
								border={annotation.border ?? false}
								textLayoutVersion={annotation.textLayoutVersion}
								{measure}
							/>
						{:else if annotation.kind === 'ink'}<g
								transform={`translate(${annotation.x * pageWidth} ${annotation.y * pageHeight})`}
								>{#each annotation.strokes ?? [] as stroke, index (index)}<path
										d={pathData(
											stroke.points,
											0,
											annotation.width * pageWidth,
											annotation.height * pageHeight
										)}
										fill="none"
										stroke={stroke.color}
										stroke-width={Math.max(0.25, stroke.width * annotation.width * pageWidth)}
										stroke-linecap="round"
										stroke-linejoin="round"
									/>{/each}</g
							>
						{:else if annotation.kind === 'circle'}<ellipse
								cx={(annotation.x + annotation.width / 2) * pageWidth}
								cy={(annotation.y + annotation.height / 2) * pageHeight}
								rx={(annotation.width * pageWidth) / 2}
								ry={(annotation.height * pageHeight) / 2}
								fill="none"
								stroke={annotation.color}
								stroke-width="1.2"
							/>
						{:else}<rect
								x={annotation.x * pageWidth}
								y={annotation.y * pageHeight}
								width={annotation.width * pageWidth}
								height={annotation.height * pageHeight}
								fill={annotation.color}
								opacity="0.3"
							/>{/if}
					{/each}
					{#if drawing.length}<path
							d={pathData(drawing, 0, pageWidth, pageHeight)}
							fill="none"
							stroke="currentColor"
							stroke-width={0.003 * pageWidth}
							stroke-linecap="round"
							stroke-linejoin="round"
						/>{/if}
				</svg>
				{#each (opened && editable ? draft : []).filter((annotation) => annotation.kind === 'text' && annotation.page === pageNumber) as annotation (annotation.id)}<TextAnnotationPlacement
						{annotation}
						{rotation}
						{disabled}
						bounds={() => overlay.getBoundingClientRect()}
						selected={selectedTextId === annotation.id}
						onselect={() => (selectedTextId = annotation.id)}
						onchange={changeTextBox}
					/>{/each}
				{#if opened && placement && placement.page === pageNumber && onplacementchange}<SignaturePlacement
						{placement}
						{rotation}
						bounds={() => overlay.getBoundingClientRect()}
						{disabled}
						paths={placementPaths}
						fileId={placementFileId}
						onchange={onplacementchange}
					/>{/if}
				{#each (opened ? detail.signatures : []).filter((signature) => signature.signature.documentRevision === detail.document.revision && signature.signature.page === pageNumber) as signed (signed.id)}
					<div
						class="pointer-events-none absolute"
						style:left={`${signed.signature.x * 100}%`}
						style:top={`${signed.signature.y * 100}%`}
						style:width={`${signed.signature.width * 100}%`}
						style:height={`${signed.signature.height * 100}%`}
					>
						{#if signed.fileId}<PrivateFileImage
								fileId={signed.fileId}
								resourceId={detail.record.id}
								alt={`ลายมือชื่อ ${signed.signerName}`}
								class="h-full w-full object-contain"
							/>{:else}<svg
								viewBox={`0 0 ${signed.signature.width * pageWidth} ${signed.signature.height * pageHeight}`}
								class="h-full w-full"
								aria-label={`ลายมือชื่อ ${signed.signerName}`}
								>{#each signed.signature.strokes ?? [] as stroke, index (index)}<path
										d={pathData(
											stroke.points,
											0,
											signed.signature.width * pageWidth,
											signed.signature.height * pageHeight
										)}
										fill="none"
										stroke={stroke.color}
										stroke-width={Math.max(0.25, stroke.width * signed.signature.width * pageWidth)}
										stroke-linecap="round"
										stroke-linejoin="round"
									/>{/each}</svg
							>{/if}
					</div>
				{/each}
			</div>
		</div>
		{#if layers.some((annotation) => annotation.text)}<details class="text-sm">
				<summary class="cursor-pointer font-medium">ข้อความกำกับบนหน้านี้</summary>
				<ul class="mt-2 space-y-2">
					{#each layers.filter((annotation) => annotation.text) as annotation (annotation.id)}<li
							class="flex justify-between gap-2"
						>
							<span class="whitespace-pre-wrap break-words">{annotation.text}</span
							>{#if draft.some((item) => item.id === annotation.id)}<Button
									type="button"
									variant="ghost"
									size="sm"
									{disabled}
									onclick={() => {
										if (!disabled) draft = draft.filter((item) => item.id !== annotation.id);
									}}>ลบข้อความ</Button
								>{/if}
						</li>{/each}
				</ul>
			</details>{/if}
	</div>
{/snippet}
{#if !fullscreen}{@render workspace()}{/if}
<Dialog.Root bind:open={fullscreen}
	><Dialog.Content class="flex h-[95dvh] w-[96vw] max-w-none flex-col overflow-auto"
		><Dialog.Header
			><Dialog.Title>เอกสาร: {detail.record.title}</Dialog.Title><Dialog.Description
				>อ่านเอกสารและเขียนหมายเหตุ แล้วกลับสู่แผงดำเนินการเพื่อยืนยัน</Dialog.Description
			></Dialog.Header
		>{#if fullscreen}{@render workspace()}{/if}</Dialog.Content
	></Dialog.Root
>

<Dialog.Root bind:open={editTextOpen}
	><Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-xl"
		><Dialog.Header
			><Dialog.Title>แก้ไขข้อความบนเอกสาร</Dialog.Title><Dialog.Description
				>การแก้ไขนี้ยังเป็นฉบับร่างจนกว่าจะยืนยันการดำเนินการ</Dialog.Description
			></Dialog.Header
		>
		<form class="space-y-4" onsubmit={saveText}>
			<div class="space-y-2">
				<Label for="edit-annotation-text">ข้อความ *</Label><Textarea
					id="edit-annotation-text"
					class="field-sizing-fixed max-h-[35dvh] overflow-y-auto"
					bind:value={editText}
					rows={7}
					{disabled}
					aria-invalid={!!annotationError}
					aria-describedby={annotationError ? 'edit-annotation-text-error' : undefined}
				/><FieldError id="edit-annotation-text-error" message={annotationError} />
			</div>
			<div class="flex items-center gap-2">
				<Checkbox id="edit-annotation-border" bind:checked={editBorder} {disabled} /><Label
					for="edit-annotation-border">แสดงกรอบข้อความ</Label
				>
			</div>
			<div class="flex justify-end gap-2">
				<Button type="button" variant="outline" onclick={() => (editTextOpen = false)}
					>ยกเลิก</Button
				><Button type="submit" {disabled}>ใช้ข้อความนี้</Button>
			</div>
		</form></Dialog.Content
	></Dialog.Root
>
