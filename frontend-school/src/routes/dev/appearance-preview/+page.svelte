<script lang="ts">
	import { onMount } from 'svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import {
		Card,
		CardHeader,
		CardContent,
		CardTitle,
		CardDescription
	} from '#lib/components/ui/card/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as RadioGroup from '#lib/components/ui/radio-group/index.js';
	import {
		ArrowUpRight,
		Check,
		CheckCheck,
		ChevronRight,
		CircleAlert,
		Eye,
		GraduationCap,
		Laptop,
		LoaderCircle,
		Monitor,
		Moon,
		Paintbrush,
		Palette,
		RotateCcw,
		Save,
		ShieldCheck,
		Smartphone,
		Sun,
		Type
	} from '@lucide/svelte';
	import {
		APPEARANCE_STORAGE_KEY,
		DEFAULT_APPEARANCE,
		FONTS,
		PALETTES,
		appearanceTokens,
		isDensity,
		isFont,
		isHexColor,
		isPalette,
		palettePrimary,
		readStoredAppearance,
		staffPreviewUrl,
		type Appearance,
		type PreviewMode
	} from '#lib/design-preview/appearance.js';
	import '#lib/design-preview/fonts.css';

	let draft = $state<Appearance>({ ...DEFAULT_APPEARANCE });
	let saved = $state<Appearance>({ ...DEFAULT_APPEARANCE });
	let hasSaved = $state(false);
	let ready = $state(false);
	let storageError = $state('');
	let saveNotice = $state('');
	let colorText = $state(palettePrimary(DEFAULT_APPEARANCE));
	let mode = $state<PreviewMode>('light');
	let device = $state<'desktop' | 'mobile'>('desktop');
	let frameLoading = $state(true);
	let frameArea: HTMLDivElement;
	let availableWidth = $state(800);
	const dirty = $derived(
		JSON.stringify(draft) !== JSON.stringify(saved) ||
			colorText.toLowerCase() !== palettePrimary(draft)
	);
	const colorError = $derived(isHexColor(colorText) ? '' : 'ใช้รหัสสี 6 หลัก เช่น #167166');
	const tokens = $derived(appearanceTokens(draft, mode));
	const font = $derived(FONTS.find((item) => item.value === draft.font) ?? FONTS[0]);
	const previewUrl = $derived(staffPreviewUrl(draft, mode));
	const viewportWidth = $derived(device === 'desktop' ? 1280 : 390);
	const viewportHeight = $derived(device === 'desktop' ? 1160 : 1900);
	const scale = $derived(Math.min(1, availableWidth / viewportWidth));
	const frameKey = $derived(`${previewUrl}:${device}`);

	$effect(() => {
		void frameKey;
		frameLoading = true;
	});

	onMount(() => {
		try {
			const raw = localStorage.getItem(APPEARANCE_STORAGE_KEY);
			const stored = readStoredAppearance(raw);
			if (stored) {
				draft = stored;
				saved = { ...stored };
				colorText = palettePrimary(stored);
				hasSaved = true;
			} else if (raw !== null)
				storageError = 'อ่านตัวอย่างที่บันทึกไว้ไม่ได้ จึงแสดงค่าเริ่มต้น คุณสามารถบันทึกใหม่ได้';
		} catch {
			storageError = 'เบราว์เซอร์ไม่อนุญาตให้อ่านข้อมูลที่บันทึกไว้ คุณยังทดลองปรับรูปแบบได้';
		}
		ready = true;
		const observer = new ResizeObserver(([entry]) => {
			availableWidth = Math.max(1, entry.contentRect.width);
		});
		observer.observe(frameArea);
		return () => observer.disconnect();
	});

	function changePalette(value: string) {
		if (!isPalette(value)) return;
		draft = { ...draft, palette: value, primary: null };
		colorText = palettePrimary(draft);
		saveNotice = '';
	}
	function changePrimary(value: string) {
		colorText = value;
		if (isHexColor(value)) {
			draft = { ...draft, primary: value.toLowerCase() };
		}
		saveNotice = '';
	}
	function resetDraft() {
		draft = { ...DEFAULT_APPEARANCE };
		colorText = palettePrimary(draft);
		saveNotice = '';
	}
	function saveExample(event: SubmitEvent) {
		event.preventDefault();
		if (!ready || colorError) return;
		try {
			localStorage.setItem(APPEARANCE_STORAGE_KEY, JSON.stringify(draft));
			saved = { ...draft };
			hasSaved = true;
			storageError = '';
			saveNotice = 'บันทึกตัวอย่างในเบราว์เซอร์นี้แล้ว';
		} catch {
			saveNotice = '';
			storageError =
				'บันทึกไม่สำเร็จ เบราว์เซอร์ไม่อนุญาตให้เก็บข้อมูล กรุณาลองใหม่ การปรับแต่งของคุณยังอยู่';
		}
	}
</script>

<div class="appearance-editor">
	<header class="editor-topbar">
		<div class="editor-brand">
			<span><GraduationCap class="size-5" /></span><strong
				>SchoolOrbit<span class="text-primary">.</span></strong
			><span class="brand-divider"></span>
			<p>การตั้งค่าโรงเรียน</p>
		</div>
		<Badge variant="outline" class="prototype-badge"
			><Paintbrush class="size-3.5" />พื้นที่ทดลองดีไซน์</Badge
		>
	</header>
	<main>
		<PageShell
			title="รูปแบบและตัวอักษร"
			description="เลือกสี ฟอนต์ และระยะห่างที่เหมาะกับโรงเรียน แล้วดูตัวอย่างได้ทันที"
		>
			{#snippet meta()}<p class="breadcrumb">
					การตั้งค่าโรงเรียน<ChevronRight class="size-3" /><span>การแสดงผล</span>
				</p>{/snippet}
			{#snippet actions()}<Badge variant="secondary" class="local-badge"
					><Laptop class="size-3.5" />เฉพาะเบราว์เซอร์นี้</Badge
				>{/snippet}
			<div class="editor-grid">
				<form class="settings-column" onsubmit={saveExample}>
					<Card class="settings-card gap-0 py-0"
						><CardHeader class="settings-card-header"
							><span class="section-icon"><Palette class="size-5" /></span>
							<div>
								<CardTitle class="text-base">สีประจำโรงเรียน</CardTitle><CardDescription
									class="mt-1">ใช้กับปุ่ม เมนู และจุดเน้นในระบบ</CardDescription
								>
							</div></CardHeader
						><CardContent class="settings-card-content">
							<RadioGroup.Root
								value={draft.palette}
								onValueChange={changePalette}
								aria-label="ชุดสีโรงเรียน"
								class="palette-options"
								>{#each PALETTES as palette (palette.value)}<Label
										for={`palette-${palette.value}`}
										class={`palette-option ${draft.palette === palette.value ? 'selected' : ''}`}
										><span class="palette-swatch" style:background={palette.primary}
											>{#if draft.palette === palette.value}<Check class="size-4" />{/if}</span
										><span>{palette.label}</span><RadioGroup.Item
											id={`palette-${palette.value}`}
											value={palette.value}
											class="sr-only"
										/></Label
									>{/each}</RadioGroup.Root
							>
							<div class="field-block">
								<Label for="custom-primary">ปรับสีหลักเพิ่มเติม</Label>
								<div class="color-controls">
									<Input
										type="color"
										aria-label="เลือกสีหลัก"
										value={palettePrimary(draft)}
										oninput={(event) => changePrimary(event.currentTarget.value)}
										class="color-picker"
									/><Input
										id="custom-primary"
										value={colorText}
										oninput={(event) => changePrimary(event.currentTarget.value)}
										aria-invalid={!!colorError}
										aria-describedby="color-help"
										spellcheck={false}
										maxlength={32}
										class="hex-input"
									/>
								</div>
								<p id="color-help" class:error-text={!!colorError} class="field-help">
									{colorError || 'สีที่เลือกจะปรับให้อ่านง่ายเมื่อจำเป็น'}
								</p>
							</div>
							<div class="contrast-note">
								<ShieldCheck class="size-4 shrink-0" />
								<div>
									<strong
										>{tokens.adjusted
											? 'ปรับความเข้มเพื่อให้อ่านง่าย'
											: 'สีหลักพร้อมใช้งานในพรีวิวนี้'}</strong
									>
									<p>
										สีที่ใช้จริง ({mode === 'light' ? 'สว่าง' : 'มืด'})
										<span class="effective-color" style:background={tokens.primary}></span><code
											>{tokens.primary.toUpperCase()}</code
										>
									</p>
								</div>
							</div>
						</CardContent></Card
					>

					<Card class="settings-card gap-0 py-0"
						><CardHeader class="settings-card-header"
							><span class="section-icon"><Type class="size-5" /></span>
							<div>
								<CardTitle class="text-base">ตัวอักษร</CardTitle><CardDescription class="mt-1"
									>รองรับภาษาไทยและภาษาอังกฤษ</CardDescription
								>
							</div></CardHeader
						><CardContent class="settings-card-content">
							<div class="field-block">
								<Label for="preview-font">ฟอนต์ของโรงเรียน</Label><Select.Root
									type="single"
									value={draft.font}
									onValueChange={(value) => {
										if (isFont(value)) {
											draft = { ...draft, font: value };
											saveNotice = '';
										}
									}}
									><Select.Trigger id="preview-font" aria-label="ฟอนต์ของโรงเรียน" class="w-full"
										>{font.label}</Select.Trigger
									><Select.Content portalProps={{ disabled: true }}
										>{#each FONTS as option (option.value)}<Select.Item value={option.value}
												>{option.label}</Select.Item
											>{/each}</Select.Content
									></Select.Root
								>
							</div>
							<div class="font-sample" style:font-family={`'${font.family}', sans-serif`}>
								<p>พื้นที่ทำงานที่เป็นคุณ</p>
								<span>ก ข ค ง · SchoolOrbit 0123456789</span>
							</div>
							<div class="field-block">
								<Label id="density-label">ระยะห่างของเนื้อหา</Label><RadioGroup.Root
									value={draft.density}
									onValueChange={(value) => {
										if (isDensity(value)) {
											draft = { ...draft, density: value };
											saveNotice = '';
										}
									}}
									aria-labelledby="density-label"
									class="density-options"
									><Label
										for="comfortable"
										class={`density-option ${draft.density === 'comfortable' ? 'selected' : ''}`}
										><span><strong>สบายตา</strong><small>เว้นระยะ อ่านได้คล่อง</small></span
										><RadioGroup.Item id="comfortable" value="comfortable" /></Label
									><Label
										for="compact"
										class={`density-option ${draft.density === 'compact' ? 'selected' : ''}`}
										><span><strong>กระชับ</strong><small>เห็นเนื้อหาได้มากขึ้น</small></span
										><RadioGroup.Item id="compact" value="compact" /></Label
									></RadioGroup.Root
								>
							</div>
						</CardContent></Card
					>

					<div class="save-panel">
						<p>
							<Laptop class="size-4 shrink-0" /><span
								>บันทึกเป็นตัวอย่างเฉพาะเบราว์เซอร์นี้<br /><strong
									>ยังไม่เปลี่ยนการตั้งค่าจริงของโรงเรียน</strong
								></span
							>
						</p>
						<div class="save-actions">
							<Button type="button" variant="outline" onclick={resetDraft}
								><RotateCcw class="size-4" />คืนค่าเริ่มต้น</Button
							><Button type="submit" disabled={!ready || !!colorError}
								><Save class="size-4" />บันทึกตัวอย่าง</Button
							>
						</div>
						<div aria-live="polite" class="save-status">
							{#if storageError}<p role="alert" class="error-text">
									<CircleAlert class="size-4 shrink-0" />{storageError}
								</p>{:else if saveNotice && !dirty}<p class="saved-text">
									<CheckCheck class="size-4" />{saveNotice}
								</p>{:else}<p>
									{dirty
										? 'มีการเปลี่ยนแปลงที่ยังไม่บันทึก'
										: hasSaved
											? 'ตรงกับตัวอย่างที่บันทึกไว้'
											: 'ค่าเริ่มต้น · ยังไม่ได้บันทึกตัวอย่าง'}
								</p>{/if}
						</div>
					</div>
				</form>

				<section class="preview-column" aria-labelledby="live-preview-heading">
					<Card class="preview-card gap-0 overflow-hidden py-0"
						><div class="preview-toolbar">
							<div class="preview-title">
								<span class="preview-live-dot"></span>
								<h2 id="live-preview-heading">ตัวอย่างหน้าจอจริง</h2>
								<Badge variant="secondary">ข้อมูลสมมติ</Badge>
							</div>
							<div class="view-controls">
								<div class="control-group" role="group" aria-label="โหมดแสงของพรีวิว">
									<Button
										type="button"
										size="sm"
										variant={mode === 'light' ? 'secondary' : 'ghost'}
										aria-pressed={mode === 'light'}
										onclick={() => {
											mode = 'light';
										}}><Sun class="size-4" /><span>สว่าง</span></Button
									><Button
										type="button"
										size="sm"
										variant={mode === 'dark' ? 'secondary' : 'ghost'}
										aria-pressed={mode === 'dark'}
										onclick={() => {
											mode = 'dark';
										}}><Moon class="size-4" /><span>มืด</span></Button
									>
								</div>
								<div class="control-group" role="group" aria-label="ขนาดหน้าจอพรีวิว">
									<Button
										type="button"
										size="sm"
										variant={device === 'desktop' ? 'secondary' : 'ghost'}
										aria-label="พรีวิวคอมพิวเตอร์"
										aria-pressed={device === 'desktop'}
										onclick={() => {
											device = 'desktop';
										}}><Monitor class="size-4" /></Button
									><Button
										type="button"
										size="sm"
										variant={device === 'mobile' ? 'secondary' : 'ghost'}
										aria-label="พรีวิวมือถือ"
										aria-pressed={device === 'mobile'}
										onclick={() => {
											device = 'mobile';
										}}><Smartphone class="size-4" /></Button
									>
								</div>
								<Button
									href={previewUrl}
									target="_blank"
									rel="noopener noreferrer"
									size="sm"
									variant="outline"
									aria-label="เปิดตัวอย่างขนาดจริงในแท็บใหม่"
									><ArrowUpRight class="size-4" /><span>ขนาดจริง</span></Button
								>
							</div>
						</div>
						<div class="preview-context">
							<span
								><Eye class="size-3.5" />หน้าหลักบุคลากร · {device === 'desktop'
									? 'คอมพิวเตอร์'
									: 'มือถือ'}
								{viewportWidth}px · ย่อ {Math.round(scale * 100)}%</span
							><span role="status"
								>{#if frameLoading}<LoaderCircle
										class="size-3.5 animate-spin"
									/>กำลังแสดงตัวอย่าง{:else}<Check class="size-3.5" />พรีวิวพร้อมแล้ว{/if}</span
							>
						</div>
						<div class="frame-stage" bind:this={frameArea}>
							<div
								class="iframe-window"
								style:width={`${viewportWidth * scale}px`}
								style:height={`${viewportHeight * scale}px`}
							>
								{#key frameKey}<iframe
										src={previewUrl}
										title="ตัวอย่างหน้าหลักบุคลากรตามรูปแบบที่เลือก"
										style:width={`${viewportWidth}px`}
										style:height={`${viewportHeight}px`}
										style:transform={`scale(${scale})`}
										onload={() => (frameLoading = false)}
									></iframe>{/key}
							</div>
						</div>
						<div class="preview-footer">
							<ShieldCheck class="size-4" />
							<p>สีสถานะยังคงความหมายเดิม</p>
							<span class="status-success">สำเร็จ</span><span class="status-warning">ควรติดตาม</span
							><span class="status-danger">เกินกำหนด</span>
						</div>
					</Card>
					<p class="preview-caption">
						โหมดสว่าง/มืดและขนาดหน้าจอใช้สำหรับดูตัวอย่างเท่านั้น
						ไม่รวมอยู่ในการบันทึกรูปแบบโรงเรียน
					</p>
				</section>
			</div>
		</PageShell>
	</main>
</div>

<style>
	.appearance-editor {
		--background: #f5f7f8;
		--foreground: #243c3b;
		--card: #ffffff;
		--card-foreground: #243c3b;
		--popover: #ffffff;
		--popover-foreground: #243c3b;
		--primary: #176c63;
		--primary-foreground: #ffffff;
		--secondary: #eaf2ef;
		--secondary-foreground: #265e53;
		--muted: #edf2f0;
		--muted-foreground: #5f716e;
		--accent: #eaf2ef;
		--accent-foreground: #245a50;
		--destructive: #ad3e3e;
		--border: #dfe7e3;
		--input: #d1ddd6;
		--ring: #238276;
		--radius: 0.8rem;
		font-family: 'Kanit Preview', sans-serif;
		color: var(--foreground);
		background: var(--background);
		min-height: 100vh;
		color-scheme: light;
	}
	.appearance-editor :global(button),
	.appearance-editor :global(input),
	.appearance-editor :global([role='listbox']) {
		font-family: inherit;
	}
	.appearance-editor :global([data-slot='badge']) {
		font-size: 12px;
	}
	.editor-topbar {
		height: 68px;
		padding: 0 28px;
		background: var(--card);
		border-bottom: 1px solid var(--border);
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
	}
	.editor-brand {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.editor-brand > span:first-child {
		display: grid;
		place-items: center;
		width: 33px;
		height: 33px;
		border-radius: 10px;
		color: var(--primary-foreground);
		background: var(--primary);
	}
	.editor-brand strong {
		font-size: 20px;
		letter-spacing: -0.6px;
	}
	.editor-brand p {
		font-size: 13px;
		color: var(--muted-foreground);
	}
	.brand-divider {
		height: 20px;
		width: 1px;
		margin: 0 10px;
		background: var(--border);
	}
	.breadcrumb {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.breadcrumb span {
		color: var(--primary);
	}
	.appearance-editor :global(.local-badge) {
		margin-top: 16px;
		padding: 7px 11px;
		font-weight: 400;
	}
	.editor-grid {
		display: grid;
		grid-template-columns: 340px minmax(0, 1fr);
		align-items: start;
		gap: 22px;
	}
	.settings-column {
		display: grid;
		gap: 16px;
		min-width: 0;
	}
	.appearance-editor :global(.settings-card) {
		box-shadow: 0 2px 4px #203a3704;
	}
	.appearance-editor :global(.settings-card-header) {
		display: flex;
		align-items: center;
		gap: 11px;
		padding: 19px 20px 16px;
		border-bottom: 1px solid var(--border);
	}
	.section-icon {
		width: 36px;
		height: 36px;
		border-radius: 10px;
		display: grid;
		place-items: center;
		background: var(--secondary);
		color: var(--primary);
		flex-shrink: 0;
	}
	.appearance-editor :global(.settings-card-content) {
		padding: 18px 20px;
		display: grid;
		gap: 18px;
	}
	.appearance-editor :global(.palette-options) {
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 8px;
	}
	.appearance-editor :global(.palette-option) {
		border: 1px solid var(--border);
		border-radius: 9px;
		padding: 12px 4px 10px;
		flex-direction: column;
		gap: 10px;
		font-size: 12px;
		cursor: pointer;
	}
	.appearance-editor :global(.palette-option.selected) {
		border-color: var(--primary);
		background: var(--secondary);
	}
	.appearance-editor :global(.palette-option:has(:focus-visible)) {
		outline: 3px solid var(--ring);
		outline-offset: 2px;
	}
	.palette-swatch {
		height: 31px;
		width: 31px;
		border-radius: 50%;
		display: grid;
		place-items: center;
		color: #ffffff;
	}
	.field-block {
		display: grid;
		gap: 10px;
	}
	.field-help {
		font-size: 12px;
		color: var(--muted-foreground);
		line-height: 1.5;
	}
	.color-controls {
		display: flex;
		gap: 10px;
	}
	.appearance-editor :global(.color-picker) {
		padding: 4px;
		width: 44px;
		height: 36px;
		cursor: pointer;
	}
	.appearance-editor :global(.hex-input) {
		flex: 1;
		font-size: 13px;
		text-transform: uppercase;
		letter-spacing: 1px;
	}
	.contrast-note {
		display: flex;
		align-items: flex-start;
		gap: 8px;
		padding: 11px;
		border-radius: 8px;
		background: var(--background);
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.contrast-note > :global(svg) {
		margin-top: 3px;
		color: var(--primary);
	}
	.contrast-note strong {
		font-size: 12px;
		font-weight: 500;
		color: var(--foreground);
	}
	.contrast-note p {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 5px;
		margin-top: 3px;
	}
	.effective-color {
		width: 10px;
		height: 10px;
		border-radius: 50%;
	}
	.contrast-note code {
		font-size: 12px;
	}
	.font-sample {
		border: 1px dashed var(--border);
		padding: 16px;
		border-radius: 9px;
		background: var(--background);
	}
	.font-sample p {
		font-size: 19px;
		font-weight: 600;
	}
	.font-sample span {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.appearance-editor :global(.density-options) {
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 8px;
	}
	.appearance-editor :global(.density-option) {
		border: 1px solid var(--border);
		border-radius: 9px;
		padding: 12px 10px;
		gap: 5px;
		justify-content: space-between;
		align-items: flex-start;
		cursor: pointer;
	}
	.appearance-editor :global(.density-option.selected) {
		background: var(--secondary);
		border-color: var(--primary);
	}
	:global(.density-option) strong {
		display: block;
		font-size: 13px;
		font-weight: 500;
	}
	:global(.density-option) small {
		display: block;
		font-size: 12px;
		color: var(--muted-foreground);
		line-height: 1.4;
		margin-top: 6px;
	}
	.save-panel {
		padding: 2px 2px 0;
	}
	.save-panel > p {
		display: flex;
		align-items: flex-start;
		gap: 8px;
		color: var(--muted-foreground);
		font-size: 12px;
		line-height: 1.7;
	}
	.save-panel > p :global(svg) {
		margin-top: 3px;
	}
	.save-panel > p strong {
		font-weight: 400;
		color: var(--foreground);
	}
	.save-actions {
		display: flex;
		gap: 8px;
		margin-top: 15px;
	}
	.save-actions :global(button) {
		flex: 1;
		font-size: 12px;
		gap: 6px;
	}
	.save-status {
		font-size: 12px;
		line-height: 1.6;
		margin-top: 10px;
		min-height: 22px;
		color: var(--muted-foreground);
	}
	.save-status p {
		display: flex;
		gap: 7px;
		align-items: flex-start;
	}
	.save-status :global(svg) {
		margin-top: 2px;
		flex-shrink: 0;
	}
	.error-text {
		color: var(--destructive);
	}
	.saved-text {
		color: var(--primary);
	}
	.preview-column {
		min-width: 0;
	}
	.appearance-editor :global(.preview-card) {
		box-shadow: 0 4px 20px #203a3705;
	}
	.preview-toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		flex-wrap: wrap;
		gap: 14px;
		padding: 16px;
		border-bottom: 1px solid var(--border);
	}
	.preview-title {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.preview-title h2 {
		font-size: 14px;
		font-weight: 500;
	}
	.preview-live-dot {
		height: 6px;
		width: 6px;
		border-radius: 50%;
		background: var(--primary);
	}
	.view-controls {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 9px;
	}
	.control-group {
		display: flex;
		gap: 2px;
		padding: 3px;
		border-radius: 8px;
		background: var(--background);
		border: 1px solid var(--border);
	}
	.view-controls :global(button),
	.view-controls :global(a) {
		font-size: 12px;
		gap: 5px;
	}
	.preview-context {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		flex-wrap: wrap;
		padding: 10px 16px;
		background: var(--card);
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.preview-context > span {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.frame-stage {
		padding: 16px;
		display: flex;
		justify-content: center;
		background: #e9eeeb;
		min-width: 0;
		overflow: hidden;
	}
	.iframe-window {
		position: relative;
		flex-shrink: 0;
		border-radius: 10px;
		overflow: hidden;
		background: var(--card);
		box-shadow: 0 4px 16px #203a3710;
	}
	iframe {
		display: block;
		border: 0;
		position: absolute;
		top: 0;
		left: 0;
		transform-origin: top left;
	}
	.preview-footer {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 10px;
		padding: 13px 16px;
		border-top: 1px solid var(--border);
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.preview-footer p {
		margin-right: auto;
	}
	.preview-footer > span {
		padding: 3px 7px;
		border-radius: 5px;
	}
	.status-success {
		color: #276442;
		background: #e9f3eb;
	}
	.status-warning {
		color: #98651b;
		background: #fcf3df;
	}
	.status-danger {
		color: #b54441;
		background: #fbecea;
	}
	.preview-caption {
		font-size: 12px;
		color: var(--muted-foreground);
		margin: 12px 2px;
		line-height: 1.7;
	}
	@media (max-width: 1100px) {
		.editor-grid {
			grid-template-columns: 310px minmax(0, 1fr);
			gap: 16px;
		}
		.appearance-editor :global(.settings-card-content),
		.appearance-editor :global(.settings-card-header) {
			padding: 16px;
		}
		.preview-toolbar {
			align-items: flex-start;
		}
		.save-actions {
			flex-wrap: wrap;
		}
	}
	@media (max-width: 850px) {
		.editor-grid {
			grid-template-columns: 1fr;
		}
		.settings-column {
			grid-template-columns: repeat(2, minmax(0, 1fr));
			align-items: start;
		}
		.save-panel {
			grid-column: 1/-1;
		}
		.save-actions {
			justify-content: flex-end;
		}
		.save-actions :global(button) {
			flex: initial;
		}
		.editor-topbar {
			padding: 0 16px;
		}
		.editor-brand p,
		.brand-divider {
			display: none;
		}
	}
	@media (max-width: 570px) {
		.settings-column {
			grid-template-columns: 1fr;
		}
		.editor-topbar {
			padding: 0 16px;
		}
		.editor-brand strong {
			font-size: 18px;
		}
		.appearance-editor :global(.prototype-badge) {
			font-size: 12px;
		}
		.appearance-editor :global(.local-badge) {
			margin-top: 0;
		}
		.preview-toolbar {
			padding: 12px;
		}
		.view-controls {
			gap: 6px;
		}
		.preview-title {
			flex-wrap: wrap;
		}
		.preview-context {
			padding: 10px 12px;
		}
		.frame-stage {
			padding: 8px;
		}
		.save-actions :global(button) {
			flex: 1;
		}
		.preview-footer {
			gap: 8px;
		}
		.preview-footer p {
			margin-right: 0;
		}
	}
</style>
