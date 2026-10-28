<script lang="ts">
	import { tick, onDestroy } from 'svelte';
	import type { HomeroomDeliveryWorkspace } from '#lib/api/learning-delivery.js';
	import {
		deliveryAlignmentLabels,
		type DeliveryAlignmentFacts,
		type MissingDeliveryRequirement
	} from '#lib/academic/homeroom-delivery.js';
	import type { CurriculumPreparationFocus } from '#lib/academic/synchronized-activity-delivery.js';
	import { deliveryChangePage } from '#lib/academic/learning-delivery-change-list.js';
	import { deliveryOfferingHref } from '#lib/navigation/learning-delivery.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { PageSkeleton } from '#lib/components/app-state/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import DeliveryAlignmentBadges from './DeliveryAlignmentBadges.svelte';
	let {
		workspace,
		facts,
		loading,
		error,
		canManage,
		preparing,
		returnTo,
		onRetry,
		onPrepare
	}: {
		workspace: HomeroomDeliveryWorkspace | null;
		facts: DeliveryAlignmentFacts | null;
		loading: boolean;
		error: string;
		canManage: boolean;
		preparing: boolean;
		returnTo: string;
		onRetry: () => Promise<void>;
		onPrepare: (focus: CurriculumPreparationFocus) => Promise<void>;
	} = $props();
	let open = $state(false),
		search = $state(''),
		missingPage = $state(0),
		existingPage = $state(0);
	const query = $derived(search.trim().toLocaleLowerCase('th-TH'));
	const missing = $derived(
		deliveryChangePage(
			(facts?.missingRequirements ?? []).filter((item) =>
				`${item.code} ${item.name} ${item.rooms.map((room) => `${room.name} ${room.studyProgramName}`).join(' ')}`
					.toLocaleLowerCase('th-TH')
					.includes(query)
			),
			missingPage
		)
	);
	const existing = $derived(
		deliveryChangePage(
			[...(facts?.offerings.values() ?? [])].filter(
				(item) =>
					(item.unlinkedReasons.length > 0 ||
						item.states.some((entry) => entry.state !== 'matches_curriculum')) &&
					`${item.code} ${item.name} ${item.rooms.map((room) => room.name).join(' ')} ${item.unlinkedReasons.map((entry) => entry.reason).join(' ')}`
						.toLocaleLowerCase('th-TH')
						.includes(query)
			),
			existingPage
		)
	);
	const attention = $derived(
		Boolean(
			facts &&
			(facts.missingCount ||
				facts.extraCount ||
				facts.unlinkedCount ||
				facts.prerequisites.length ||
				[...facts.offerings.values()].some((item) =>
					item.states.some((entry) => entry.state !== 'matches_curriculum')
				))
		)
	);
	let trigger = $state<HTMLButtonElement | null>(null);
	let launching = $state(false);
	let returnFocusOnClose = true,
		disposed = false,
		observedContext = '';
	const context = $derived(
		workspace
			? `${workspace.academicYearId}:${workspace.academicTermId}:${workspace.deliveryVersionId ?? 'no-version'}`
			: ''
	);
	function showReport() {
		returnFocusOnClose = true;
		open = true;
	}
	async function prepare(item: MissingDeliveryRequirement) {
		if (launching || !canManage || !context) return;
		const owner = context,
			action = onPrepare;
		launching = true;
		returnFocusOnClose = false;
		open = false;
		await tick();
		try {
			if (disposed || context !== owner || !canManage) return;
			trigger?.focus();
			await action({ resourceKind: item.resourceKind, catalogVersionId: item.catalogVersionId });
		} finally {
			if (!disposed && context === owner) launching = false;
		}
	}
	$effect.pre(() => {
		const owner = context;
		if (owner !== observedContext) {
			observedContext = owner;
			returnFocusOnClose = false;
			open = false;
			launching = false;
			search = '';
			missingPage = 0;
			existingPage = 0;
		}
	});
	onDestroy(() => {
		disposed = true;
	});
</script>

<section
	class="flex flex-wrap items-center justify-between gap-3 rounded-xl border bg-card px-3 py-2"
	aria-label="เทียบกับหลักสูตรปัจจุบัน"
	aria-busy={loading}
>
	{#if !workspace || !facts}
		{#if loading}<PageSkeleton variant="cards" rows={1} />
		{:else if error}<div
				role="alert"
				class="flex flex-wrap items-center gap-2 text-sm text-destructive"
			>
				<p>โหลดผลเทียบหลักสูตรไม่สำเร็จ: {error}</p>
				<Button size="sm" variant="outline" onclick={onRetry}>ลองโหลดผลเทียบอีกครั้ง</Button>
			</div>
		{:else}<p class="text-sm text-muted-foreground">
				ยังไม่มีข้อมูลสำหรับเทียบหลักสูตรของรุ่นที่เลือก
			</p>{/if}
	{:else}
		<div class="min-w-0 text-sm">
			<p class="font-medium">เทียบกับหลักสูตรปัจจุบัน · ทุกห้องของรุ่นที่เลือก</p>
			<p class="text-xs text-muted-foreground" data-testid="delivery-alignment-summary">
				ยังไม่เปิดสอน {facts.missingCount} รายการ · รายการเดิมที่ต้องตรวจ {facts.affectedExistingCount}
				รายการ · นอกโครงสร้าง {facts.extraCount} รายการ · ยังเชื่อมห้องไม่ได้ {facts.unlinkedCount} รายการ{facts
					.prerequisites.length
					? ` · ขาดโครงสร้าง ${facts.prerequisites.length} ห้อง`
					: ''}
			</p>
		</div>
		<Button variant="outline" size="sm" bind:ref={trigger} onclick={showReport}
			>{attention ? 'ตรวจเทียบหลักสูตร' : 'ดูผลเทียบหลักสูตร'}</Button
		>
	{/if}
</section>
<Dialog.Root bind:open>
	<Dialog.Content
		class="flex max-h-[calc(100dvh-2rem)] flex-col overflow-hidden sm:max-w-4xl"
		onCloseAutoFocus={(event) => {
			event.preventDefault();
			if (returnFocusOnClose && !disposed && trigger?.isConnected) trigger.focus();
		}}
	>
		<Dialog.Header class="shrink-0"
			><Dialog.Title>ผลเทียบหลักสูตรของรุ่นเปิดสอน</Dialog.Title><Dialog.Description
				>แสดงทุกห้องของรุ่นที่เลือก โดยไม่ใช้ตัวกรองในตารางรายการเปิดสอน
				รายการเดียวอาจมีหลายสถานะในคนละห้อง จึงไม่นำจำนวนแต่ละสถานะมาบวกรวม</Dialog.Description
			></Dialog.Header
		>
		<div class="min-h-0 space-y-5 overflow-y-auto">
			<div class="space-y-2 rounded-lg bg-muted/40 p-3 text-sm text-muted-foreground">
				<p>
					รายการที่เปิดสอนเก็บชื่อและฉบับทะเบียนที่ใช้อ้างอิงไว้
					การเผยแพร่หลักสูตรใหม่ไม่เปลี่ยนชื่อหรือแทนรายการเดิมให้อัตโนมัติ
				</p>
				<p>
					รายการนอกโครงสร้างอาจเป็นรายการเพิ่มเติมที่ตั้งใจเปิด ควรตรวจรายละเอียดก่อนตัดสินใจลบ
					หากต้องใช้รายการตามหลักสูตรปัจจุบัน ให้ตรวจและเปิดในแบบร่าง
					ส่วนการลบมีผลเฉพาะรุ่นแบบร่างที่เลือกและไม่ลบประวัติที่เผยแพร่แล้ว
				</p>
			</div>
			<Input
				value={search}
				aria-label="ค้นหาผลเทียบหลักสูตร"
				placeholder="ค้นหารายวิชา กิจกรรม ห้อง หรือเหตุผล"
				oninput={(event) => {
					search = event.currentTarget.value;
					missingPage = 0;
					existingPage = 0;
				}}
			/>
			<section class="space-y-3" aria-label="หลักสูตรกำหนดไว้แต่ยังไม่เปิดสอน">
				<h3 class="font-medium">หลักสูตรกำหนดไว้แต่ยังไม่เปิดสอน</h3>
				{#each missing.items as item (item.key)}<article
						class="space-y-2 rounded-lg border border-warning/30 p-3"
						data-testid="delivery-missing-requirement"
					>
						<p class="font-medium">
							{item.resourceKind === 'course' ? `${item.code} · ` : ''}{item.name}
						</p>
						<p class="text-sm text-muted-foreground">
							{item.rooms.map((room) => room.name).join(', ')} · {item.requirementIds.length} ข้อกำหนด
						</p>
						<p class="text-xs text-muted-foreground">
							ยังไม่มีรายการเปิดสอนที่ตรงกับฉบับนี้
							ระบบจะตรวจวันเริ่มใช้ของฉบับทะเบียนและข้อจำกัดของภาคเรียนอีกครั้ง
							รายการใหม่ไม่โอนกลุ่ม ครู หรือคะแนนจากรายการเดิมให้อัตโนมัติ
						</p>
						{#if canManage && workspace?.deliveryVersionId}<Button
								size="sm"
								variant="outline"
								disabled={preparing || launching}
								onclick={() => prepare(item)}
								aria-label={`เปิดตามหลักสูตร · ${item.name}`}>เปิดตามหลักสูตร</Button
							>{:else if canManage && !workspace?.deliveryVersionId}<p
								class="text-xs text-muted-foreground"
							>
								สร้างรุ่นเปิดสอนจากปุ่มด้านบนก่อนเปิดรายการตามหลักสูตร
							</p>{/if}
					</article>{:else}<p class="text-sm text-muted-foreground">
						{query
							? 'ไม่พบรายการที่ตรงกับการค้นหา'
							: facts?.prerequisites.length
								? 'ห้องที่ขาดโครงสร้างยังตรวจข้อกำหนดไม่ได้'
								: 'ไม่มีข้อกำหนดที่ยังไม่เปิดสอน'}
					</p>{/each}
				{#if missing.pageCount > 1}<div class="flex items-center justify-end gap-2">
						<Button
							size="sm"
							variant="outline"
							disabled={missing.page === 0}
							onclick={() => (missingPage = missing.page - 1)}>ข้อกำหนดก่อนหน้า</Button
						><span class="text-sm">{missing.page + 1}/{missing.pageCount}</span><Button
							size="sm"
							variant="outline"
							disabled={missing.page + 1 === missing.pageCount}
							onclick={() => (missingPage = missing.page + 1)}>ข้อกำหนดถัดไป</Button
						>
					</div>{/if}
			</section>
			<section class="space-y-3" aria-label="รายการเปิดสอนที่ต้องตรวจเทียบ">
				<h3 class="font-medium">รายการเปิดสอนที่ต้องตรวจเทียบ</h3>
				{#each existing.items as item (item.offeringId)}<article
						class="space-y-2 rounded-lg border p-3"
						data-testid="delivery-existing-alignment"
					>
						<p class="font-medium">
							{item.resourceKind === 'course' ? `${item.code} · ` : ''}{item.name}
						</p>
						<DeliveryAlignmentBadges fact={item} />{#each item.states as entry (entry.state)}<p
								class="text-xs text-muted-foreground"
							>
								{deliveryAlignmentLabels[entry.state]}: {entry.rooms
									.map((room) => room.name)
									.join(', ')}
							</p>{/each}{#each item.unlinkedReasons as entry (`${entry.groupId ?? ''}:${entry.reason}`)}<p
								class="text-sm text-muted-foreground"
							>
								{entry.reason}
							</p>{/each}{#if workspace}<Button
								href={deliveryOfferingHref(item.offeringId, workspace, returnTo)}
								size="sm"
								variant="outline"
								data-sveltekit-preload-data="tap">ตรวจรายการเปิดสอน</Button
							>{/if}
					</article>{:else}<p class="text-sm text-muted-foreground">
						{query
							? 'ไม่พบรายการที่ตรงกับการค้นหา'
							: !facts?.offerings.size
								? 'ยังไม่มีผลเทียบรายการเปิดสอน'
								: 'ไม่มีรายการเปิดสอนที่ต้องตรวจเทียบเพิ่มเติม'}
					</p>{/each}
				{#if existing.pageCount > 1}<div class="flex items-center justify-end gap-2">
						<Button
							size="sm"
							variant="outline"
							disabled={existing.page === 0}
							onclick={() => (existingPage = existing.page - 1)}>รายการก่อนหน้า</Button
						><span class="text-sm">{existing.page + 1}/{existing.pageCount}</span><Button
							size="sm"
							variant="outline"
							disabled={existing.page + 1 === existing.pageCount}
							onclick={() => (existingPage = existing.page + 1)}>รายการถัดไป</Button
						>
					</div>{/if}
			</section>
			{#if facts?.prerequisites.length}<section
					class="space-y-2"
					aria-label="ห้องที่ยังขาดโครงสร้างหลักสูตร"
				>
					<h3 class="font-medium">ห้องที่ยังขาดโครงสร้างหลักสูตร</h3>
					{#each facts.prerequisites as entry (entry.room.id)}<p class="text-sm">
							{entry.room.name} · {entry.blockers.map((blocker) => blocker.message).join(' · ')}
						</p>{/each}
				</section>{/if}
		</div>
		<Dialog.Footer class="shrink-0"
			><Button variant="outline" onclick={() => (open = false)}>ปิดผลเทียบ</Button></Dialog.Footer
		>
	</Dialog.Content>
</Dialog.Root>
