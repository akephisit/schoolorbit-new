<script lang="ts">
	import type { CertificateRenderManifest, CertificateTemplateDetail } from '$lib/api/certificates';
	import {
		certificateManifestExpiresSoon,
		certificateManifestNeedsLayoutGrants,
		cloneCertificateLayout,
		constrainElementToPage,
		moveElement,
		resizeElement,
		rotateElement,
		screenDeltaToElementAxes,
		snapElementToPage,
		type CertificateElement,
		type CertificateLayout,
		type ResizeHandle
	} from '$lib/certificates/editor-state';
	import { interpolateCertificateText } from '$lib/certificates/interpolation';
	import { pointsToMillimetres } from '$lib/certificates/layout';
	import { loadCertificateRenderer } from '$lib/certificates/renderer';
	import { AlertTriangle, Image as ImageIcon, QrCode } from 'lucide-svelte';
	import { untrack } from 'svelte';

	type Interaction = {
		kind: 'move' | 'resize' | 'rotate';
		targetId: string;
		selectedIds: string[];
		startX: number;
		startY: number;
		startLayout: CertificateLayout;
		handle?: ResizeHandle;
		startAngle?: number;
		startRotation?: number;
		centerX?: number;
		centerY?: number;
	};

	let {
		template,
		layout,
		manifest,
		selectedIds,
		zoom,
		safeMarginPoints,
		showSafeArea,
		snapToGuides,
		disabled = false,
		onmanifestrefresh,
		onlayoutchange,
		onselectionchange,
		onduplicate,
		ondelete
	}: {
		template: CertificateTemplateDetail;
		layout: CertificateLayout;
		manifest: CertificateRenderManifest;
		selectedIds: string[];
		zoom: number;
		safeMarginPoints: number;
		showSafeArea: boolean;
		snapToGuides: boolean;
		disabled?: boolean;
		onmanifestrefresh: () => Promise<CertificateRenderManifest>;
		onlayoutchange: (layout: CertificateLayout) => void;
		onselectionchange: (elementIds: string[]) => void;
		onduplicate: () => void;
		ondelete: () => void;
	} = $props();

	let backgroundCanvas = $state<HTMLCanvasElement>();
	let paperElement = $state<HTMLDivElement>();
	let backgroundError = $state('');
	let fontError = $state('');
	let fontAliases = $state.raw<Record<string, string>>({});
	let interaction: Interaction | null = null;

	const geometry = $derived(template.pageGeometry ?? manifest.pageGeometry);
	const pageSize = $derived({
		width: geometry.displayedWidthPoints,
		height: geometry.displayedHeightPoints
	});
	const safeMarginMillimetres = $derived(
		Math.round(pointsToMillimetres(safeMarginPoints) * 10) / 10
	);
	const manifestNeedsLayoutGrants = $derived(
		certificateManifestNeedsLayoutGrants(manifest, layout)
	);
	const fontPreparationKey = $derived(
		layout.elements
			.filter((element) => element.type === 'text')
			.map((element) =>
				[
					element.id,
					element.fontSource.type === 'asset' ? `asset:${element.fontSource.asset_id}` : 'built_in',
					element.fontFamily,
					element.fontWeight,
					element.fontStyle
				].join(':')
			)
			.join('|')
	);
	const resizeHandles: ResizeHandle[] = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'];

	const displayValues = $derived({
		ปีการศึกษา: manifest.campaignValues.academicYear,
		ชื่อกิจกรรมหลัก: manifest.campaignValues.campaignName,
		วันที่จัดกิจกรรม: manifest.campaignValues.eventDate,
		วันที่ออก: manifest.campaignValues.issueDate,
		ชื่อโรงเรียนผู้ออก: manifest.campaignValues.schoolName,
		ชื่อหน่วยงานเจ้าของกิจกรรม: manifest.campaignValues.ownerOrganizationUnitName,
		...manifest.recipientValues,
		เลขเกียรติบัตร: manifest.certificateNumber,
		QR_CODE: manifest.qrPayload
	});

	$effect(() => {
		const canvas = backgroundCanvas;
		const currentManifest = manifest;
		const needsLayoutGrants = manifestNeedsLayoutGrants;
		const currentZoom = zoom;
		if (!canvas) return;
		const controller = new AbortController();
		backgroundError = '';
		void (async () => {
			const effectiveManifest =
				certificateManifestExpiresSoon(currentManifest) || needsLayoutGrants
					? await onmanifestrefresh()
					: currentManifest;
			controller.signal.throwIfAborted();
			const renderer = await loadCertificateRenderer();
			await renderer.renderPreview(
				{
					...effectiveManifest,
					layout: { schemaVersion: 1, elements: [] }
				},
				canvas,
				{ scale: currentZoom, signal: controller.signal }
			);
		})().catch((error: unknown) => {
			if (controller.signal.aborted) return;
			backgroundError = error instanceof Error ? error.message : 'ไม่สามารถแสดง PDF พื้นหลังได้';
		});
		return () => controller.abort();
	});

	$effect(() => {
		const preparationKey = fontPreparationKey;
		const currentManifest = manifest;
		const needsLayoutGrants = manifestNeedsLayoutGrants;
		if (!preparationKey) {
			fontAliases = {};
			fontError = '';
			return;
		}
		const currentLayout = untrack(() => cloneCertificateLayout(layout));
		const controller = new AbortController();
		fontAliases = {};
		fontError = '';
		void (async () => {
			const effectiveManifest =
				certificateManifestExpiresSoon(currentManifest) || needsLayoutGrants
					? await onmanifestrefresh()
					: currentManifest;
			controller.signal.throwIfAborted();
			const renderer = await loadCertificateRenderer();
			const aliases = await renderer.prepareFontAliases(
				effectiveManifest,
				currentLayout,
				controller.signal
			);
			controller.signal.throwIfAborted();
			fontAliases = aliases;
		})().catch((error: unknown) => {
			if (controller.signal.aborted) return;
			fontError = error instanceof Error ? error.message : 'ไม่สามารถโหลดฟอนต์สำหรับ editor ได้';
		});
		return () => controller.abort();
	});

	function textValue(element: Extract<CertificateElement, { type: 'text' }>): string {
		try {
			return interpolateCertificateText(element.content, displayValues);
		} catch {
			return element.content;
		}
	}

	function imageUrl(element: Extract<CertificateElement, { type: 'image' }>): string | null {
		return manifest.imageGrants.find((grant) => grant.assetId === element.assetId)?.url ?? null;
	}

	function selectForPointer(event: PointerEvent, elementId: string): string[] {
		const next = event.shiftKey
			? selectedIds.includes(elementId)
				? selectedIds.filter((id) => id !== elementId)
				: [...selectedIds, elementId]
			: [elementId];
		onselectionchange(next);
		return next.includes(elementId) ? next : [elementId];
	}

	function beginMove(event: PointerEvent, elementId: string) {
		if (disabled || event.button !== 0) return;
		event.preventDefault();
		event.stopPropagation();
		const nextSelection = selectForPointer(event, elementId);
		interaction = {
			kind: 'move',
			targetId: elementId,
			selectedIds: nextSelection,
			startX: event.clientX,
			startY: event.clientY,
			startLayout: cloneCertificateLayout(layout)
		};
	}

	function beginResize(event: PointerEvent, elementId: string, handle: ResizeHandle) {
		if (disabled || event.button !== 0) return;
		event.preventDefault();
		event.stopPropagation();
		onselectionchange([elementId]);
		interaction = {
			kind: 'resize',
			targetId: elementId,
			selectedIds: [elementId],
			startX: event.clientX,
			startY: event.clientY,
			startLayout: cloneCertificateLayout(layout),
			handle
		};
	}

	function beginRotate(event: PointerEvent, element: CertificateElement) {
		if (disabled || event.button !== 0 || !paperElement) return;
		event.preventDefault();
		event.stopPropagation();
		onselectionchange([element.id]);
		const pageRect = paperElement.getBoundingClientRect();
		const centerX = pageRect.left + (element.frame.x + element.frame.width / 2) * zoom;
		const centerY = pageRect.top + (element.frame.y + element.frame.height / 2) * zoom;
		interaction = {
			kind: 'rotate',
			targetId: element.id,
			selectedIds: [element.id],
			startX: event.clientX,
			startY: event.clientY,
			startLayout: cloneCertificateLayout(layout),
			centerX,
			centerY,
			startAngle: Math.atan2(event.clientY - centerY, event.clientX - centerX),
			startRotation: element.rotation
		};
	}

	function replaceElements(
		base: CertificateLayout,
		replacements: Readonly<Record<string, CertificateElement>>
	): CertificateLayout {
		return {
			schemaVersion: 1,
			elements: base.elements.map((element) => replacements[element.id] ?? element)
		};
	}

	function handlePointerMove(event: PointerEvent) {
		if (!interaction) return;
		event.preventDefault();
		const dxPixels = event.clientX - interaction.startX;
		const dyPixels = event.clientY - interaction.startY;
		const target = interaction.startLayout.elements.find(
			(element) => element.id === interaction?.targetId
		);
		if (!target) return;

		if (interaction.kind === 'move') {
			let movedTarget = moveElement(target, { dxPixels, dyPixels }, zoom);
			movedTarget = snapToGuides
				? snapElementToPage(movedTarget, pageSize, { safeMarginPoints })
				: constrainElementToPage(movedTarget, pageSize);
			const dxPoints = movedTarget.frame.x - target.frame.x;
			const dyPoints = movedTarget.frame.y - target.frame.y;
			const replacements: Record<string, CertificateElement> = {};
			for (const element of interaction.startLayout.elements) {
				if (!interaction.selectedIds.includes(element.id)) continue;
				replacements[element.id] = constrainElementToPage(
					moveElement(element, { dxPixels: dxPoints * zoom, dyPixels: dyPoints * zoom }, zoom),
					pageSize
				);
			}
			onlayoutchange(replaceElements(interaction.startLayout, replacements));
			return;
		}

		if (interaction.kind === 'resize' && interaction.handle) {
			const localDelta = screenDeltaToElementAxes({ dxPixels, dyPixels }, target.rotation);
			const resized = constrainElementToPage(
				resizeElement(target, { handle: interaction.handle, ...localDelta }, zoom),
				pageSize
			);
			onlayoutchange(replaceElements(interaction.startLayout, { [target.id]: resized }));
			return;
		}

		if (
			interaction.kind === 'rotate' &&
			interaction.centerX !== undefined &&
			interaction.centerY !== undefined &&
			interaction.startAngle !== undefined &&
			interaction.startRotation !== undefined
		) {
			const angle = Math.atan2(
				event.clientY - interaction.centerY,
				event.clientX - interaction.centerX
			);
			const delta = ((angle - interaction.startAngle) * 180) / Math.PI;
			const rotated = constrainElementToPage(
				rotateElement(target, interaction.startRotation + delta),
				pageSize
			);
			onlayoutchange(replaceElements(interaction.startLayout, { [target.id]: rotated }));
		}
	}

	function finishInteraction() {
		interaction = null;
	}

	function handleKeydown(event: KeyboardEvent) {
		if (disabled || selectedIds.length === 0) return;
		const target = event.target as HTMLElement | null;
		if (target?.matches('input, textarea, select, [contenteditable="true"]')) return;
		if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'd') {
			event.preventDefault();
			onduplicate();
			return;
		}
		if (event.key === 'Delete' || event.key === 'Backspace') {
			event.preventDefault();
			ondelete();
			return;
		}
		const directions: Record<string, { x: number; y: number }> = {
			ArrowLeft: { x: -1, y: 0 },
			ArrowRight: { x: 1, y: 0 },
			ArrowUp: { x: 0, y: -1 },
			ArrowDown: { x: 0, y: 1 }
		};
		const direction = directions[event.key];
		if (!direction) return;
		event.preventDefault();
		const distance = event.shiftKey ? 10 : 1;
		const replacements: Record<string, CertificateElement> = {};
		for (const element of layout.elements) {
			if (!selectedIds.includes(element.id)) continue;
			replacements[element.id] = constrainElementToPage(
				moveElement(
					element,
					{
						dxPixels: direction.x * distance * zoom,
						dyPixels: direction.y * distance * zoom
					},
					zoom
				),
				pageSize
			);
		}
		onlayoutchange(replaceElements(layout, replacements));
	}

	function handlePosition(handle: ResizeHandle): string {
		const positions: Record<ResizeHandle, string> = {
			nw: 'left:0;top:0;cursor:nwse-resize',
			n: 'left:50%;top:0;cursor:ns-resize',
			ne: 'left:100%;top:0;cursor:nesw-resize',
			e: 'left:100%;top:50%;cursor:ew-resize',
			se: 'left:100%;top:100%;cursor:nwse-resize',
			s: 'left:50%;top:100%;cursor:ns-resize',
			sw: 'left:0;top:100%;cursor:nesw-resize',
			w: 'left:0;top:50%;cursor:ew-resize'
		};
		return `${positions[handle]};transform:translate(-50%,-50%)`;
	}
</script>

<svelte:window
	onpointermove={handlePointerMove}
	onpointerup={finishInteraction}
	onpointercancel={finishInteraction}
	onkeydown={handleKeydown}
/>

<div
	class="relative flex min-h-[34rem] min-w-0 flex-1 items-center justify-center overflow-auto bg-[radial-gradient(circle_at_1px_1px,rgba(148,163,184,0.24)_1px,transparent_0)] bg-[size:20px_20px] p-12"
	data-testid="certificate-canvas-workspace"
>
	<div
		bind:this={paperElement}
		class="certificate-paper relative shrink-0 overflow-visible bg-white shadow-[0_18px_55px_rgba(15,23,42,0.28)] ring-1 ring-slate-950/10"
		style:width={`${pageSize.width * zoom}px`}
		style:height={`${pageSize.height * zoom}px`}
	>
		<canvas
			bind:this={backgroundCanvas}
			class="pointer-events-none absolute inset-0 size-full bg-white"
			aria-label="PDF พื้นหลังที่ล็อกไว้"
		></canvas>
		<button
			type="button"
			class="absolute inset-0 z-[1] size-full bg-transparent p-0 outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-indigo-500"
			onclick={() => onselectionchange([])}
			aria-label="ยกเลิกการเลือกองค์ประกอบ"
		></button>

		{#if backgroundError || fontError}
			<div
				class="absolute inset-0 z-10 grid place-items-center bg-white/95 p-8 text-center text-sm text-destructive"
			>
				<div><AlertTriangle class="mx-auto mb-2 size-6" />{backgroundError || fontError}</div>
			</div>
		{/if}

		{#if showSafeArea}
			<div
				class="pointer-events-none absolute z-10 border border-dashed border-amber-500/90"
				style:inset={`${safeMarginPoints * zoom}px`}
			>
				<span
					class="absolute -top-5 left-0 rounded-t bg-amber-500 px-1.5 py-0.5 text-[9px] font-semibold text-white"
				>
					พื้นที่ปลอดภัย {safeMarginMillimetres} มม.
				</span>
			</div>
		{/if}

		{#each layout.elements as element (element.id)}
			{@const selected = selectedIds.includes(element.id)}
			<div
				class="absolute z-20"
				style:left={`${element.frame.x * zoom}px`}
				style:top={`${element.frame.y * zoom}px`}
				style:width={`${element.frame.width * zoom}px`}
				style:height={`${element.frame.height * zoom}px`}
				style:transform={`rotate(${element.rotation}deg)`}
			>
				<button
					type="button"
					class={[
						'absolute inset-0 bg-transparent p-0 outline-none',
						element.type === 'text' ? 'overflow-visible' : 'overflow-hidden',
						selected
							? 'ring-2 ring-indigo-500 ring-offset-1 ring-offset-white/70'
							: 'hover:ring-1 hover:ring-indigo-400/60',
						disabled ? 'cursor-default' : 'cursor-move'
					]}
					{disabled}
					onpointerdown={(event) => beginMove(event, element.id)}
					onclick={(event) => event.stopPropagation()}
					aria-label={`เลือกองค์ประกอบ ${element.type}`}
					aria-pressed={selected}
				>
					{#if element.type === 'text'}
						{@const fontAlias = fontAliases[element.id]}
						{#if fontAlias}
							{@const textSafetyInset = Math.max(2, element.fontSize * 0.12) * zoom}
							<span
								class="block w-full whitespace-pre-wrap break-words"
								style:height={`calc(100% + ${textSafetyInset * 2}px)`}
								style:margin-top={`-${textSafetyInset}px`}
								style:padding-top={`${textSafetyInset}px`}
								style:padding-bottom={`${textSafetyInset}px`}
								style:font-family={fontAlias}
								style:font-size={`${element.fontSize * zoom}px`}
								style:font-weight={element.fontWeight}
								style:font-style={element.fontStyle}
								style:line-height={element.lineHeight}
								style:color={element.color}
								style:text-align={element.alignment}
								style:text-shadow={element.shadow
									? `${element.shadow.offsetX * zoom}px ${element.shadow.offsetY * zoom}px ${element.shadow.blur * zoom}px ${element.shadow.color}`
									: 'none'}
							>
								{textValue(element)}
							</span>
						{/if}
					{:else if element.type === 'image'}
						{@const url = imageUrl(element)}
						{#if url}
							<img
								src={url}
								alt=""
								class="size-full object-fill"
								crossorigin="anonymous"
								referrerpolicy="no-referrer"
							/>
						{:else}
							<span
								class="grid size-full place-items-center border bg-muted/70 text-muted-foreground"
							>
								<ImageIcon class="size-1/3" />
							</span>
						{/if}
					{:else}
						<span class="grid size-full place-items-center border bg-white text-slate-950">
							<QrCode class="size-[82%]" strokeWidth={1.7} />
						</span>
					{/if}
				</button>

				{#if selected && selectedIds.length === 1 && !disabled}
					{#each resizeHandles as handle (handle)}
						<button
							type="button"
							class="absolute z-30 size-2.5 rounded-full border-2 border-white bg-indigo-600 shadow-sm"
							style={handlePosition(handle)}
							onpointerdown={(event) => beginResize(event, element.id, handle)}
							aria-label={`ปรับขนาดด้าน ${handle}`}
						></button>
					{/each}
					<div
						class="pointer-events-none absolute bottom-full left-1/2 h-6 w-px -translate-x-1/2 bg-indigo-500"
					></div>
					<button
						type="button"
						class="absolute bottom-[calc(100%+1.35rem)] left-1/2 z-30 size-3 -translate-x-1/2 rounded-full border-2 border-white bg-indigo-600 shadow-sm cursor-grab"
						onpointerdown={(event) => beginRotate(event, element)}
						aria-label="หมุนองค์ประกอบ"
					></button>
				{/if}
			</div>
		{/each}
	</div>
</div>
