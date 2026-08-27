<script lang="ts">
	import {
		applyRecommendedAcademicMenuTemplate,
		previewRecommendedAcademicMenuTemplate,
		type AcademicMenuTemplatePreview
	} from '$lib/api/menu-admin';
	import { ApiClientError } from '$lib/api/client';
	import { LoadingButton } from '$lib/components/app-state';
	import * as Alert from '$lib/components/ui/alert';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import * as Table from '$lib/components/ui/table';
	import { ArrowRight, LayoutTemplate, RefreshCw, ShieldCheck } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	let {
		open = $bindable(false),
		canApply,
		onApplied
	}: {
		open?: boolean;
		canApply: boolean;
		onApplied: () => Promise<void> | void;
	} = $props();

	let preview = $state.raw<AcademicMenuTemplatePreview | null>(null);
	let loading = $state(false);
	let applying = $state(false);
	let errorMessage = $state('');

	const hasChanges = $derived(
		Boolean(preview && (preview.moves.length > 0 || preview.sectionsToCreate.length > 0))
	);

	async function loadPreview() {
		if (loading) return;
		loading = true;
		errorMessage = '';
		try {
			preview = await previewRecommendedAcademicMenuTemplate();
		} catch (error) {
			preview = null;
			errorMessage =
				error instanceof Error ? error.message : 'ไม่สามารถโหลดตัวอย่างโครงสร้างงานวิชาการได้';
		} finally {
			loading = false;
		}
	}

	function openPreview() {
		open = true;
		void loadPreview();
	}

	function handleOpenChange(nextOpen: boolean) {
		open = nextOpen;
		if (nextOpen && !preview && !loading) void loadPreview();
	}

	async function applyPreview() {
		if (!preview || !canApply || !preview.recommendationsReady || !hasChanges) return;

		applying = true;
		errorMessage = '';
		try {
			const result = await applyRecommendedAcademicMenuTemplate(preview.revision);
			await onApplied();
			open = false;
			toast.success(
				`ใช้โครงสร้างแนะนำแล้ว ย้าย ${result.movedCount} เมนู และสร้าง ${result.createdSectionCount} งาน`
			);
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await loadPreview();
				errorMessage = 'ข้อมูลเมนูเปลี่ยนแล้ว กรุณาตรวจสอบรายการอีกครั้ง';
				return;
			}
			errorMessage =
				error instanceof Error ? error.message : 'ไม่สามารถใช้โครงสร้างงานวิชาการแนะนำได้';
		} finally {
			applying = false;
		}
	}
</script>

<Button variant="outline" onclick={openPreview}>
	<LayoutTemplate class="h-4 w-4" />
	ใช้โครงสร้างงานวิชาการแนะนำ
</Button>

<Dialog.Root bind:open onOpenChange={handleOpenChange}>
	<Dialog.Content class="max-h-[90vh] overflow-y-auto sm:max-w-4xl">
		<Dialog.Header class="space-y-3 text-left">
			<div class="flex items-center gap-2 text-xs font-medium tracking-wide text-primary">
				<LayoutTemplate class="h-4 w-4" />
				<span>โครงสร้างแนะนำ · กลุ่มบริหารวิชาการ</span>
			</div>
			<Dialog.Title>ใช้โครงสร้างงานวิชาการแนะนำ</Dialog.Title>
			<Dialog.Description>
				ตรวจรายการที่จะย้ายก่อนยืนยัน ระบบจะไม่เปลี่ยนชื่อ ไอคอน สถานะ หรือเมนูที่โรงเรียนสร้างเอง
			</Dialog.Description>
		</Dialog.Header>

		{#if loading}
			<div class="space-y-3 py-2" aria-label="กำลังโหลดตัวอย่างโครงสร้างเมนู">
				<Skeleton class="h-20 w-full rounded-xl" />
				<Skeleton class="h-44 w-full rounded-xl" />
			</div>
		{:else if errorMessage && !preview}
			<Alert.Root variant="destructive">
				<Alert.Title>โหลดตัวอย่างไม่สำเร็จ</Alert.Title>
				<Alert.Description class="space-y-3">
					<p>{errorMessage}</p>
					<Button size="sm" variant="outline" onclick={() => void loadPreview()}>
						<RefreshCw class="h-4 w-4" />
						ลองอีกครั้ง
					</Button>
				</Alert.Description>
			</Alert.Root>
		{:else if preview}
			<div class="space-y-4">
				{#if errorMessage}
					<Alert.Root variant="destructive">
						<Alert.Title>ต้องตรวจรายการใหม่</Alert.Title>
						<Alert.Description>{errorMessage}</Alert.Description>
					</Alert.Root>
				{/if}

				{#if !preview.recommendationsReady}
					<Alert.Root>
						<RefreshCw class="h-4 w-4" />
						<Alert.Title>คำแนะนำเส้นทางยังไม่ครบ</Alert.Title>
						<Alert.Description>
							ให้ระบบซิงก์เมนูจาก frontend รุ่นล่าสุดก่อน แล้วจึงเปิดตัวอย่างอีกครั้ง
						</Alert.Description>
					</Alert.Root>
				{:else}
					<div class="grid gap-px overflow-hidden rounded-xl border bg-border sm:grid-cols-3">
						<div class="bg-card p-4">
							<p class="text-xs text-muted-foreground">เมนูที่จะจัดใหม่</p>
							<p class="mt-1 text-2xl font-semibold tabular-nums">{preview.moves.length}</p>
						</div>
						<div class="bg-card p-4">
							<p class="text-xs text-muted-foreground">งานที่จะสร้างเพิ่ม</p>
							<p class="mt-1 text-2xl font-semibold tabular-nums">
								{preview.sectionsToCreate.length}
							</p>
						</div>
						<div class="bg-card p-4">
							<p class="flex items-center gap-1.5 text-xs text-muted-foreground">
								<ShieldCheck class="h-3.5 w-3.5" />
								เมนูสร้างเองที่ไม่แตะต้อง
							</p>
							<p class="mt-1 text-2xl font-semibold tabular-nums">
								{preview.untouchedCustomItemCount}
							</p>
						</div>
					</div>

					{#if preview.sectionsToCreate.length > 0}
						<section class="space-y-2" aria-labelledby="template-new-sections">
							<h3 id="template-new-sections" class="text-sm font-semibold">งานที่จะสร้างเพิ่ม</h3>
							<div class="flex flex-wrap gap-2">
								{#each preview.sectionsToCreate as section (section.code)}
									<Badge variant="secondary">{section.name}</Badge>
								{/each}
							</div>
						</section>
					{/if}

					<section class="space-y-2" aria-labelledby="template-route-ledger">
						<div class="flex items-center justify-between gap-3">
							<h3 id="template-route-ledger" class="text-sm font-semibold">รายการก่อน → หลัง</h3>
							<Badge variant="outline">ไม่เปลี่ยนสิทธิ์เข้าถึง</Badge>
						</div>

						{#if preview.moves.length === 0}
							<div class="rounded-xl border border-dashed p-6 text-center">
								<p class="font-medium">โครงสร้างเมนูตรงกับคำแนะนำแล้ว</p>
								<p class="mt-1 text-sm text-muted-foreground">ไม่มีรายการที่ต้องย้ายหรือเรียงใหม่</p>
							</div>
						{:else}
							<div class="hidden overflow-hidden rounded-xl border md:block">
								<Table.Root>
									<Table.Header>
										<Table.Row>
											<Table.Head>เมนูบริการ</Table.Head>
											<Table.Head>ตำแหน่งปัจจุบัน</Table.Head>
											<Table.Head class="w-10"><span class="sr-only">ไปยัง</span></Table.Head>
											<Table.Head>ตำแหน่งแนะนำ</Table.Head>
										</Table.Row>
									</Table.Header>
									<Table.Body>
										{#each preview.moves as item (item.menuItemId)}
											<Table.Row>
												<Table.Cell class="font-medium">{item.menuItemName}</Table.Cell>
												<Table.Cell>
													<div>{item.currentGroupName ?? 'ยังไม่จัดงาน'}</div>
													<div class="text-xs text-muted-foreground">ลำดับ {item.currentOrder}</div>
												</Table.Cell>
												<Table.Cell><ArrowRight class="h-4 w-4 text-muted-foreground" /></Table.Cell>
												<Table.Cell>
													<div>{item.targetGroupName}</div>
													<div class="text-xs text-muted-foreground">ลำดับ {item.targetOrder}</div>
												</Table.Cell>
											</Table.Row>
										{/each}
									</Table.Body>
								</Table.Root>
							</div>

							<div class="grid gap-2 md:hidden">
								{#each preview.moves as item (item.menuItemId)}
									<article class="rounded-xl border p-3">
										<h4 class="font-medium">{item.menuItemName}</h4>
										<div class="mt-2 grid grid-cols-[1fr_auto_1fr] items-center gap-2 text-sm">
											<span class="text-muted-foreground">{item.currentGroupName ?? 'ยังไม่จัดงาน'}</span>
											<ArrowRight class="h-4 w-4 text-primary" />
											<span class="font-medium">{item.targetGroupName}</span>
										</div>
									</article>
								{/each}
							</div>
						{/if}
					</section>
				{/if}
			</div>
		{/if}

		<Dialog.Footer class="gap-2">
			<Button variant="outline" onclick={() => (open = false)} disabled={applying}>ปิด</Button>
			<LoadingButton
				loading={applying}
				loadingLabel="กำลังจัดโครงสร้าง..."
				disabled={!preview || !preview.recommendationsReady || !canApply || !hasChanges}
				onclick={() => void applyPreview()}
			>
				ยืนยันใช้โครงสร้างนี้
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
