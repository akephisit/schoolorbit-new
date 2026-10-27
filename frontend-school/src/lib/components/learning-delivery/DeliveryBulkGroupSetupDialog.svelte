<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import {
		applyBulkGroupSetup,
		previewBulkGroupSetup,
		type BulkGroupSetupRequest,
		type BulkGroupSetupPreview,
		type BulkGroupSetupResponse,
		type DeliveryManagementOptions,
		type DeliveryVersion,
		type LearningGroup,
		type LearningOffering
	} from '#lib/api/learning-delivery.js';
	import { ApiClientError } from '#lib/api/client.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { deliveryChangePage } from '#lib/academic/learning-delivery-change-list.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as RadioGroup from '#lib/components/ui/radio-group/index.js';
	import DeliveryOptionCombobox from './DeliveryOptionCombobox.svelte';

	let {
		offering,
		version,
		groups,
		canManage,
		onRequestManagementOptions,
		onApplied,
		onReload
	}: {
		offering: LearningOffering;
		version: DeliveryVersion;
		groups: LearningGroup[];
		canManage: boolean;
		onRequestManagementOptions: () => Promise<DeliveryManagementOptions | null>;
		onApplied: (response: BulkGroupSetupResponse) => void;
		onReload: () => Promise<void>;
	} = $props();

	let open = $state(false);
	let trigger = $state<HTMLButtonElement | null>(null);
	let targets = $state.raw<LearningGroup[]>([]);
	let versionRowVersion = $state(0);
	let selectedIds = $state<string[]>([]);
	let options = $state.raw<DeliveryManagementOptions | null>(null);
	let optionsLoading = $state(false);
	let teacherEnabled = $state(false);
	let teacherId = $state('');
	let replaceExistingPrimary = $state(false);
	let rosterEnabled = $state(false);
	let mode = $state<'manual' | 'homeroom'>('manual');
	let effectiveFrom = $state<string | undefined>();
	let search = $state('');
	let groupPage = $state(0);
	let previewPage = $state(0);
	let preview = $state.raw<BulkGroupSetupPreview | null>(null);
	let previewKey = $state('');
	let previewing = $state(false);
	let saving = $state(false);
	let reloading = $state(false);
	let needsReload = $state(false);
	let errorMessage = $state('');
	let session = 0;
	let disposed = false;
	const previewRequest = new LatestRequest();
	const busy = $derived(previewing || saving || reloading);
	const context = $derived(`${offering.id}:${version.id}`);
	const filteredGroups = $derived(
		targets.filter((group) =>
			`${group.name} ${group.code}`
				.toLocaleLowerCase('th-TH')
				.includes(search.trim().toLocaleLowerCase('th-TH'))
		)
	);
	const visibleGroups = $derived(deliveryChangePage(filteredGroups, groupPage));
	const eligibleGroups = $derived(targets.filter(eligible));
	const request = $derived.by((): BulkGroupSetupRequest => ({
		deliveryVersionId: version.id,
		deliveryVersionRowVersion: versionRowVersion,
		groups: targets
			.filter((group) => selectedIds.includes(group.id))
			.map((group) => ({ learningGroupId: group.id, rowVersion: group.rowVersion })),
		...(teacherEnabled ? { primaryTeacher: { teacherId, replaceExistingPrimary } } : {}),
		...(rosterEnabled
			? {
					rosterTracking: {
						mode,
						effectiveFrom: mode === 'homeroom' ? (effectiveFrom ?? null) : null
					}
				}
			: {})
	}));
	const requestKey = $derived(JSON.stringify(request));
	const previewCurrent = $derived(preview !== null && previewKey === requestKey && !needsReload);
	const valid = $derived(
		selectedIds.length > 0 &&
			(teacherEnabled || rosterEnabled) &&
			(!teacherEnabled || Boolean(teacherId)) &&
			(!rosterEnabled || mode === 'manual' || Boolean(effectiveFrom))
	);
	const visiblePreview = $derived(
		deliveryChangePage(previewCurrent ? (preview?.targets ?? []) : [], previewPage)
	);
	const teacherActions = {
		unchanged: 'ครูหลักคงเดิม',
		set: 'เพิ่มครูหลัก',
		replace: 'แทนครูหลักเดิม',
		preserve_existing: 'ข้ามครูหลักที่กำหนดแล้ว'
	};

	function eligible(group: LearningGroup): boolean {
		return group.status === 'draft' && group.rosterStatus === 'draft' && !group.teachersLocked;
	}
	function current(token: number, owner: string): boolean {
		return !disposed && open && canManage && session === token && context === owner;
	}
	function changeOpen(value: boolean) {
		if (busy) return;
		open = value;
		if (!value) {
			session += 1;
			previewRequest.abort();
		}
	}
	async function show() {
		if (!canManage || busy) return;
		const token = ++session;
		const owner = context;
		targets = [...groups];
		versionRowVersion = version.rowVersion;
		selectedIds = [];
		teacherEnabled = false;
		teacherId = '';
		replaceExistingPrimary = false;
		rosterEnabled = false;
		mode = 'manual';
		effectiveFrom = undefined;
		search = '';
		groupPage = 0;
		preview = null;
		previewKey = '';
		needsReload = false;
		errorMessage = '';
		open = true;
		options = null;
		await loadOptions(token, owner);
	}
	async function loadOptions(token = session, owner = context) {
		optionsLoading = true;
		try {
			const loaded = await onRequestManagementOptions();
			if (current(token, owner)) {
				options = loaded;
				if (!loaded) errorMessage = 'โหลดตัวเลือกครูไม่สำเร็จ กรุณาลองอีกครั้ง';
			}
		} catch (error) {
			if (current(token, owner))
				errorMessage = error instanceof Error ? error.message : 'โหลดตัวเลือกครูไม่สำเร็จ';
		} finally {
			if (current(token, owner)) optionsLoading = false;
		}
	}
	function toggleGroup(id: string, checked: boolean) {
		selectedIds = checked
			? [...selectedIds, id]
			: selectedIds.filter((selected) => selected !== id);
	}
	function fail(error: unknown, fallback: string) {
		if (error instanceof ApiClientError && error.status === 409) {
			needsReload = true;
			preview = null;
			errorMessage = 'ข้อมูลเปลี่ยนจากที่อื่น โหลดกลุ่มล่าสุดแล้วตรวจการตั้งค่าอีกครั้งก่อนบันทึก';
		} else errorMessage = error instanceof Error ? error.message : fallback;
	}
	async function check() {
		if (!canManage || busy || !valid || needsReload) return;
		const token = session,
			owner = context,
			submitted = request,
			key = requestKey;
		const ticket = previewRequest.begin();
		previewing = true;
		errorMessage = '';
		preview = null;
		try {
			const result = await previewBulkGroupSetup(offering.id, submitted, { signal: ticket.signal });
			if (!current(token, owner) || !previewRequest.isCurrent(ticket.revision)) return;
			preview = result;
			previewKey = key;
			previewPage = 0;
		} catch (error) {
			if (!isAbortError(error) && current(token, owner)) fail(error, 'ตรวจการตั้งค่าไม่สำเร็จ');
		} finally {
			if (current(token, owner)) previewing = false;
		}
	}
	async function apply() {
		if (
			!canManage ||
			busy ||
			!previewCurrent ||
			!preview ||
			preview.blockedCount > 0 ||
			preview.changedCount === 0
		)
			return;
		const token = session,
			owner = context,
			submitted = request;
		saving = true;
		errorMessage = '';
		try {
			const result = await applyBulkGroupSetup(offering.id, submitted);
			if (!current(token, owner)) return;
			onApplied(result);
			open = false;
			session += 1;
		} catch (error) {
			if (current(token, owner)) fail(error, 'บันทึกการตั้งค่าหลายกลุ่มไม่สำเร็จ');
		} finally {
			if (!disposed && context === owner) saving = false;
		}
	}
	async function reload() {
		if (busy || !canManage) return;
		const token = session,
			owner = context;
		reloading = true;
		errorMessage = '';
		try {
			await onReload();
			await tick();
			if (!current(token, owner)) return;
			targets = [...groups];
			versionRowVersion = version.rowVersion;
			selectedIds = selectedIds.filter((id) =>
				targets.some((group) => group.id === id && eligible(group))
			);
			needsReload = false;
			preview = null;
		} catch (error) {
			if (current(token, owner)) fail(error, 'โหลดกลุ่มล่าสุดไม่สำเร็จ');
		} finally {
			if (current(token, owner)) reloading = false;
		}
	}
	$effect.pre(() => {
		if (!canManage) {
			open = false;
			session += 1;
			previewRequest.abort();
		}
	});
	onDestroy(() => {
		disposed = true;
		session += 1;
		previewRequest.abort();
	});
</script>

<Button
	bind:ref={trigger}
	variant="outline"
	size="sm"
	onclick={show}
	disabled={!canManage || groups.length === 0}>ตั้งค่าหลายกลุ่ม</Button
>
<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content
		class="flex max-h-[calc(100dvh-2rem)] flex-col overflow-hidden sm:max-w-3xl"
		showCloseButton={!busy}
		escapeKeydownBehavior={busy ? 'ignore' : 'close'}
		interactOutsideBehavior={busy ? 'ignore' : 'close'}
		onCloseAutoFocus={(event) => {
			event.preventDefault();
			if (!disposed && trigger?.isConnected) trigger.focus();
		}}
	>
		<Dialog.Header>
			<Dialog.Title>ตั้งค่าหลายกลุ่ม · {offering.nameSnapshot}</Dialog.Title>
			<Dialog.Description
				>เลือกกลุ่มในร่างนี้ กำหนดครูหลักหรือวิธีจัดรายชื่อพร้อมกัน แล้วตรวจผลก่อนบันทึก
				แก้ข้อยกเว้นในแต่ละกลุ่มได้ภายหลัง</Dialog.Description
			>
		</Dialog.Header>
		<div class="min-h-0 space-y-5 overflow-y-auto px-1 py-2" data-testid="delivery-bulk-setup-body">
			<section class="space-y-3">
				<div class="flex flex-wrap items-center justify-between gap-2">
					<h3 class="font-medium">
						กลุ่มเป้าหมาย · เลือก {selectedIds.length}/{targets.length} กลุ่ม
					</h3>
					<div class="flex flex-wrap gap-2">
						<Button
							size="sm"
							variant="ghost"
							disabled={busy}
							onclick={() => (selectedIds = eligibleGroups.map((group) => group.id))}
							>เลือกทุกกลุ่มที่แก้ไขได้ในรายการนี้ ({eligibleGroups.length})</Button
						><Button size="sm" variant="ghost" disabled={busy} onclick={() => (selectedIds = [])}
							>ล้างที่เลือก</Button
						>
					</div>
				</div>
				<Label for="bulk-group-search">ค้นหากลุ่มเป้าหมาย</Label><Input
					id="bulk-group-search"
					value={search}
					oninput={(event) => {
						search = event.currentTarget.value;
						groupPage = 0;
					}}
					disabled={busy}
				/>
				<div class="divide-y rounded-lg border">
					{#each visibleGroups.items as group (group.id)}
						<div class="flex items-start gap-3 p-3">
							<Checkbox
								id={`bulk-group-${group.id}`}
								checked={selectedIds.includes(group.id)}
								onCheckedChange={(checked) => toggleGroup(group.id, checked)}
								disabled={busy || !eligible(group)}
							/><Label
								for={`bulk-group-${group.id}`}
								class="min-w-0 flex-1 flex-col items-start gap-1 leading-normal"
								><span class="break-words">{group.name}</span><span
									class="text-xs font-normal text-muted-foreground"
									>{eligible(group)
										? `ครู ${group.teacherAssignments.length} คน · ห้องต้นทาง ${group.homeroomIds.length} ห้อง`
										: 'กลุ่มนี้แก้ไขไม่ได้'}</span
								></Label
							>
						</div>
					{:else}<p class="p-4 text-sm text-muted-foreground">ไม่พบกลุ่มที่ตรงกับการค้นหา</p>{/each}
				</div>
				{#if visibleGroups.pageCount > 1}<div class="flex items-center justify-end gap-2 text-sm">
						<Button
							size="sm"
							variant="outline"
							disabled={busy || visibleGroups.page === 0}
							onclick={() => (groupPage = visibleGroups.page - 1)}>กลุ่มก่อนหน้า</Button
						><span>{visibleGroups.page + 1}/{visibleGroups.pageCount}</span><Button
							size="sm"
							variant="outline"
							disabled={busy || visibleGroups.page + 1 === visibleGroups.pageCount}
							onclick={() => (groupPage = visibleGroups.page + 1)}>กลุ่มถัดไป</Button
						>
					</div>{/if}
			</section>
			<section class="space-y-3 rounded-lg border p-3">
				<div class="flex items-center gap-2">
					<Checkbox id="bulk-teacher-enabled" bind:checked={teacherEnabled} disabled={busy} /><Label
						for="bulk-teacher-enabled">กำหนดครูหลัก</Label
					>
				</div>
				{#if teacherEnabled}
					{#if optionsLoading}<p class="text-sm text-muted-foreground">
							กำลังโหลดตัวเลือกครู
						</p>{:else if options}<DeliveryOptionCombobox
							bind:value={teacherId}
							options={options.teachers.map((teacher) => ({ id: teacher.id, label: teacher.name }))}
							placeholder="เลือกครูหลักสำหรับกลุ่มเป้าหมาย"
							searchPlaceholder="ค้นหาครูหลัก..."
							disabled={busy}
						/>{:else}<Button
							size="sm"
							variant="outline"
							onclick={() => loadOptions()}
							disabled={busy}>ลองโหลดตัวเลือกครูอีกครั้ง</Button
						>{/if}
					<div class="flex items-start gap-2">
						<Checkbox
							id="bulk-replace-primary"
							bind:checked={replaceExistingPrimary}
							disabled={busy}
						/><Label for="bulk-replace-primary" class="leading-normal"
							>แทนครูหลักเดิมในกลุ่มที่เลือกด้วย</Label
						>
					</div>
					<p class="text-xs text-muted-foreground">
						{replaceExistingPrimary
							? 'ครูหลักเดิมจะถูกแทนที่ตามผลตรวจด้านล่าง'
							: 'ค่าเริ่มต้น: ข้ามกลุ่มที่มีครูหลักแล้ว'} ครูร่วมและผู้ช่วยสอนคงเดิม
					</p>
				{/if}
			</section>
			<section class="space-y-3 rounded-lg border p-3">
				<div class="flex items-center gap-2">
					<Checkbox id="bulk-roster-enabled" bind:checked={rosterEnabled} disabled={busy} /><Label
						for="bulk-roster-enabled">กำหนดวิธีจัดรายชื่อ</Label
					>
				</div>
				{#if rosterEnabled}
					<div class="grid gap-3 sm:grid-cols-2">
						<div class="space-y-2">
							<p id="bulk-roster-mode-label" class="text-sm font-medium">
								วิธีจัดรายชื่อสำหรับกลุ่มเป้าหมาย
							</p>
							<RadioGroup.Root
								value={mode}
								onValueChange={(value) => (mode = value === 'homeroom' ? 'homeroom' : 'manual')}
								disabled={busy}
								aria-labelledby="bulk-roster-mode-label"
								class="space-y-2"
							>
								<div class="flex items-center gap-2">
									<RadioGroup.Item id="bulk-roster-manual" value="manual" /><Label
										for="bulk-roster-manual">จัดรายชื่อเอง</Label
									>
								</div>
								<div class="flex items-center gap-2">
									<RadioGroup.Item id="bulk-roster-homeroom" value="homeroom" /><Label
										for="bulk-roster-homeroom">ติดตามห้องอัตโนมัติ</Label
									>
								</div>
							</RadioGroup.Root>
						</div>
						{#if mode === 'homeroom'}<div class="space-y-2">
								<Label for="bulk-roster-date">วันที่เริ่มติดตาม</Label><DatePicker
									id="bulk-roster-date"
									ariaLabel="วันที่เริ่มติดตามสำหรับกลุ่มเป้าหมาย"
									bind:value={effectiveFrom}
									disabled={busy}
								/>
							</div>{/if}
					</div>
					<p class="text-xs text-muted-foreground">
						ใช้ห้องต้นทางที่แต่ละกลุ่มกำหนดไว้ การตั้งค่านี้ไม่มีผลกับกลุ่มที่เพิ่มในภายหลัง
					</p>
				{/if}
			</section>
			{#if previewCurrent && preview}
				<section class="space-y-3" data-testid="delivery-bulk-preview">
					<h3 class="font-medium">
						ผลตรวจ: เปลี่ยน {preview.changedCount} · ข้าม {preview.skippedCount} · ติดข้อจำกัด {preview.blockedCount}
						กลุ่ม
					</h3>
					{#each visiblePreview.items as target (target.group.id)}<article
							class="space-y-1 rounded-lg border p-3 text-sm"
						>
							<h4 class="font-medium">{target.group.name}</h4>
							<p class="text-muted-foreground">
								ครูหลักปัจจุบัน: {target.group.teacherAssignments
									.filter((teacher) => teacher.role === 'primary')
									.map((teacher) => teacher.displayName)
									.join(', ') || 'ยังไม่กำหนด'}
							</p>
							<p class="text-muted-foreground">
								วิธีจัดรายชื่อปัจจุบัน: {target.rosterTracking.mode === 'homeroom'
									? `ติดตามห้องอัตโนมัติ เริ่ม ${target.rosterTracking.effectiveFrom ?? target.rosterTracking.startsOn}`
									: 'จัดรายชื่อเอง'}
							</p>
							<p class="text-muted-foreground">
								{teacherEnabled ? teacherActions[target.primaryTeacherAction] : 'ครูคงเดิม'} · {target.rosterTrackingChanged
									? `เปลี่ยนเป็น${mode === 'homeroom' ? 'ติดตามห้องอัตโนมัติ' : 'จัดรายชื่อเอง'}`
									: 'วิธีจัดรายชื่อคงเดิม'}
							</p>
							{#if target.blockedReason}<p class="text-destructive">{target.blockedReason}</p>{/if}
						</article>{/each}
					{#if visiblePreview.pageCount > 1}<div
							class="flex items-center justify-end gap-2 text-sm"
						>
							<Button
								size="sm"
								variant="outline"
								disabled={busy || visiblePreview.page === 0}
								onclick={() => (previewPage = visiblePreview.page - 1)}>ผลก่อนหน้า</Button
							><span>{visiblePreview.page + 1}/{visiblePreview.pageCount}</span><Button
								size="sm"
								variant="outline"
								disabled={busy || visiblePreview.page + 1 === visiblePreview.pageCount}
								onclick={() => (previewPage = visiblePreview.page + 1)}>ผลถัดไป</Button
							>
						</div>{/if}
				</section>
			{:else if preview}<p class="text-sm text-muted-foreground">
					การเลือกเปลี่ยนแล้ว กรุณาตรวจผลอีกครั้ง
				</p>{/if}
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
			{#if needsReload}<LoadingButton
					variant="outline"
					loading={reloading}
					loadingLabel="กำลังโหลดกลุ่มล่าสุด"
					onclick={reload}>โหลดกลุ่มล่าสุด</LoadingButton
				>{/if}
		</div>
		<Dialog.Footer class="flex-wrap"
			><Button variant="outline" disabled={busy} onclick={() => changeOpen(false)}>ยกเลิก</Button
			><LoadingButton
				variant="outline"
				loading={previewing}
				loadingLabel="กำลังตรวจ"
				disabled={saving || reloading || !valid || needsReload}
				onclick={check}>ตรวจการตั้งค่า</LoadingButton
			><LoadingButton
				loading={saving}
				loadingLabel="กำลังบันทึก"
				disabled={previewing ||
					reloading ||
					!previewCurrent ||
					!preview ||
					preview.blockedCount > 0 ||
					preview.changedCount === 0}
				onclick={apply}>บันทึกให้กลุ่มที่เลือก</LoadingButton
			></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
