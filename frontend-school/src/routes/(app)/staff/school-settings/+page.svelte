<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { PageProps } from './$types';
	import { authStore } from '#lib/stores/auth.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '#lib/components/ui/card/index.js';
	import { Save, Upload, ImageOff, X } from '@lucide/svelte';
	import { toast } from 'svelte-sonner';
	import { getSchoolSettings, updateSchoolSettings, deleteSchoolLogo } from '#lib/api/school.js';
	import { publicFileUrl, uploadFile } from '#lib/api/files.js';

	let logoFileId = $state<string | undefined>(undefined); // file ID สำหรับลบ
	let saving = $state(false);
	let loading = $state(true);
	let pendingFile = $state<File | undefined>(undefined);
	let previewUrl = $state<string | undefined>(undefined);

	const canReadSettings = $derived($can.has(PERMISSIONS.SETTINGS_READ_ALL));
	const canUpdateSettings = $derived($can.has(PERMISSIONS.SETTINGS_UPDATE_ALL));

	let { data }: PageProps = $props();
	const source = $derived(data.settings),
		request = new LatestRequest();
	let loaded = $state(false),
		loadError = $state('');
	let owner = '',
		epoch = 0,
		disposed = false,
		pendingUploadId: string | undefined;
	function clearDraft() {
		if (previewUrl) URL.revokeObjectURL(previewUrl);
		previewUrl = undefined;
		pendingFile = undefined;
		pendingUploadId = undefined;
	}
	function applySettings(result: Awaited<typeof data.settings>, revision: number) {
		if (!request.isCurrent(revision)) return;
		loading = false;
		if (result.ok) {
			logoFileId = result.data?.logoFileId;
			loaded = true;
		} else loadError = result.error;
	}
	$effect.pre(() => {
		const operation = source,
			identity = `${$authStore.user?.id ?? ''}|${canReadSettings}`;
		untrack(() => {
			if (owner !== identity) {
				owner = identity;
				epoch++;
				logoFileId = undefined;
				loaded = false;
				saving = false;
				clearDraft();
			}
			const ticket = request.begin();
			loading = true;
			loadError = '';
			void operation.then((result) => applySettings(result, ticket.revision));
		});
		return () => request.abort();
	});
	$effect.pre(() => {
		const allowed = canUpdateSettings;
		untrack(() => {
			void allowed;
			epoch++;
			saving = false;
		});
	});
	onDestroy(() => {
		disposed = true;
		epoch++;
		request.abort();
		clearDraft();
	});
	async function loadSettings() {
		if (!canReadSettings) return;
		const ticket = request.begin();
		loading = true;
		loadError = '';
		applySettings(
			await captureRouteLoad(
				getSchoolSettings({ signal: ticket.signal }),
				'โหลดการตั้งค่าโรงเรียนไม่สำเร็จ'
			),
			ticket.revision
		);
	}

	function handleLogoSelect(e: Event) {
		if (!canUpdateSettings || saving || disposed) return;
		const file = (e.target as HTMLInputElement).files?.[0];
		if (!file) return;
		if (previewUrl) URL.revokeObjectURL(previewUrl);
		pendingUploadId = undefined;
		pendingFile = file;
		previewUrl = URL.createObjectURL(file);
	}

	async function handleSave() {
		if (!canUpdateSettings) {
			toast.error('ไม่มีสิทธิ์บันทึกการตั้งค่าโรงเรียน');
			return;
		}

		if (saving || !loaded || disposed) return;
		const mutationEpoch = epoch,
			file = pendingFile;
		const current = () => !disposed && mutationEpoch === epoch && canUpdateSettings;
		request.abort();
		loading = false;
		saving = true;
		try {
			let nextLogoId = logoFileId;
			if (file) {
				const uploadedId = pendingUploadId ?? (await uploadFile(file, 'school_logo')).id;
				if (!current()) return;
				pendingUploadId = uploadedId;
				nextLogoId = uploadedId;
			}
			await updateSchoolSettings({ logoFileId: nextLogoId ?? null });
			if (!current()) return;
			request.abort();
			loading = false;
			logoFileId = nextLogoId;
			if (pendingFile === file) clearDraft();
			toast.success('บันทึกการตั้งค่าสำเร็จ');
		} catch (err) {
			if (current()) toast.error(err instanceof Error ? err.message : 'บันทึกไม่สำเร็จ');
		} finally {
			if (current()) saving = false;
		}
	}

	async function handleDeleteLogo() {
		if (!canUpdateSettings) {
			toast.error('ไม่มีสิทธิ์ลบ logo โรงเรียน');
			return;
		}

		if (saving || !loaded || disposed) return;
		const mutationEpoch = epoch;
		const current = () => !disposed && mutationEpoch === epoch && canUpdateSettings;
		request.abort();
		loading = false;
		saving = true;
		try {
			await deleteSchoolLogo();
			if (!current()) return;
			request.abort();
			loading = false;
			logoFileId = undefined;
			toast.success('ลบ logo สำเร็จ');
		} catch (err) {
			if (current()) toast.error(err instanceof Error ? err.message : 'ลบไม่สำเร็จ');
		} finally {
			if (current()) saving = false;
		}
	}
</script>

<PageShell
	title="ตั้งค่าโรงเรียน"
	description="จัดการข้อมูลและการแสดงผลของโรงเรียน"
	backHref="/staff"
>
	<section data-testid="settings-school" aria-busy={loading}>
		{#if loading && loaded}<p role="status">กำลังอัปเดตการตั้งค่า</p>{/if}
		{#if loadError}<PageState
				variant="error"
				title="โหลดการตั้งค่าไม่สำเร็จ"
				description={loadError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadSettings}
			/>{/if}
		{#if !canReadSettings}
			<PageState
				variant="permission"
				title="ไม่มีสิทธิ์ดูการตั้งค่าโรงเรียน"
				description="บัญชีนี้เข้า module ตั้งค่าได้ แต่ยังไม่มีสิทธิ์อ่านข้อมูลการตั้งค่าโรงเรียน"
			/>
		{:else if loading && !loaded}
			<div role="status" aria-label="กำลังโหลดการตั้งค่า">
				<PageSkeleton variant="form" rows={3} />
			</div>
		{:else if loaded}
			<!-- Logo Card -->
			<Card class="max-w-lg">
				<CardHeader>
					<CardTitle>Logo โรงเรียน</CardTitle>
					<CardDescription>แสดงบนหน้ารับสมัครนักเรียนและหน้าอื่นๆ ของระบบ</CardDescription>
				</CardHeader>
				<CardContent class="space-y-6">
					<!-- Preview -->
					<div class="flex flex-col items-center gap-3">
						<div
							class="w-24 h-24 rounded-2xl border-2 border-dashed border-border flex items-center justify-center bg-muted overflow-hidden"
						>
							{#if previewUrl}
								<img src={previewUrl} alt="school logo" class="w-full h-full object-contain p-1" />
							{:else if logoFileId}
								<img
									src={publicFileUrl(logoFileId)}
									alt="school logo"
									class="w-full h-full object-contain p-1"
								/>
							{:else}
								<ImageOff class="w-8 h-8 text-muted-foreground" />
							{/if}
						</div>
						<p class="text-xs text-muted-foreground">
							{#if pendingFile}
								<span class="text-amber-600">ยังไม่ได้บันทึก — กด "บันทึก" เพื่อยืนยัน</span>
							{:else}
								ตัวอย่าง logo
							{/if}
						</p>
					</div>

					{#if canUpdateSettings}
						<!-- Upload -->
						<div class="space-y-2">
							<Label>เลือกไฟล์ logo</Label>
							<p class="text-xs text-muted-foreground">รองรับ JPG, PNG, WEBP ขนาดไม่เกิน 2 MB</p>
							<label class="cursor-pointer">
								<input
									type="file"
									accept="image/jpeg,image/png,image/webp"
									class="hidden"
									onchange={handleLogoSelect}
									disabled={saving}
								/>
								<Button variant="outline" class="gap-2 pointer-events-none" disabled={saving}>
									<Upload class="w-4 h-4" />
									เลือกไฟล์
								</Button>
							</label>
						</div>
					{/if}

					{#if canUpdateSettings}
						<!-- Actions -->
						<div class="flex items-center gap-3 pt-2">
							<Button onclick={handleSave} disabled={saving} class="gap-2">
								<Save class="w-4 h-4" />
								{saving ? 'กำลังบันทึก...' : 'บันทึก'}
							</Button>
							{#if pendingFile}
								<Button
									variant="ghost"
									class="gap-1.5 text-muted-foreground"
									onclick={() => {
										clearDraft();
									}}
									disabled={saving}
								>
									<X class="w-4 h-4" /> ยกเลิก
								</Button>
							{:else if logoFileId}
								<Button
									variant="ghost"
									class="text-destructive hover:text-destructive"
									onclick={handleDeleteLogo}
									disabled={saving}
								>
									ลบ logo
								</Button>
							{/if}
						</div>
					{/if}
				</CardContent>
			</Card>
		{/if}
	</section>
</PageShell>
