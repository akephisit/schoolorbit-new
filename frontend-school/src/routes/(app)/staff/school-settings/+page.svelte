<script lang="ts">
	import { onMount } from 'svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { Button } from '$lib/components/ui/button';
	import { Label } from '$lib/components/ui/label';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { Save, Upload, ImageOff, X } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { getSchoolSettings, updateSchoolSettings, deleteSchoolLogo } from '$lib/api/school';
	import { uploadFile } from '$lib/api/files';

	let logoUrl = $state<string | undefined>(undefined); // URL สำหรับ preview (จาก backend)
	let logoPath = $state<string | undefined>(undefined); // storage_path ที่เก็บใน DB
	let logoFileId = $state<string | undefined>(undefined); // file ID สำหรับลบ
	let saving = $state(false);
	let loading = $state(true);
	let pendingFile = $state<File | undefined>(undefined);
	let previewUrl = $state<string | undefined>(undefined);

	const canReadSettings = $derived($can.has(PERMISSIONS.SETTINGS_READ_ALL));
	const canUpdateSettings = $derived($can.has(PERMISSIONS.SETTINGS_UPDATE_ALL));

	onMount(async () => {
		if (!canReadSettings) {
			loading = false;
			return;
		}

		try {
			const s = await getSchoolSettings();
			logoUrl = s.logoUrl;
			logoFileId = s.logoFileId;
		} catch {
			toast.error('ไม่สามารถโหลดข้อมูลได้');
		} finally {
			loading = false;
		}
	});

	function handleLogoSelect(e: Event) {
		if (!canUpdateSettings) return;
		const file = (e.target as HTMLInputElement).files?.[0];
		if (!file) return;
		if (previewUrl) URL.revokeObjectURL(previewUrl);
		pendingFile = file;
		previewUrl = URL.createObjectURL(file);
	}

	async function handleSave() {
		if (!canUpdateSettings) {
			toast.error('ไม่มีสิทธิ์บันทึกการตั้งค่าโรงเรียน');
			return;
		}

		saving = true;
		try {
			let pathToSave = logoPath;
			if (pendingFile) {
				const uploaded = (await uploadFile(pendingFile, 'school_logo')).file;
				pathToSave = uploaded.storage_path;
				logoUrl = uploaded.url;
				logoFileId = uploaded.id;
				if (previewUrl) URL.revokeObjectURL(previewUrl);
				previewUrl = undefined;
				pendingFile = undefined;
			}
			await updateSchoolSettings({ logoPath: pathToSave, logoFileId });
			logoPath = pathToSave;
			toast.success('บันทึกการตั้งค่าสำเร็จ');
		} catch (err) {
			toast.error(err instanceof Error ? err.message : 'บันทึกไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	async function handleDeleteLogo() {
		if (!canUpdateSettings) {
			toast.error('ไม่มีสิทธิ์ลบ logo โรงเรียน');
			return;
		}

		saving = true;
		try {
			await deleteSchoolLogo();
			logoUrl = undefined;
			logoPath = undefined;
			logoFileId = undefined;
			toast.success('ลบ logo สำเร็จ');
		} catch (err) {
			toast.error(err instanceof Error ? err.message : 'ลบไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}
</script>

<svelte:head>
	<title>ตั้งค่าโรงเรียน - SchoolOrbit</title>
</svelte:head>

<PageShell
	title="ตั้งค่าโรงเรียน"
	description="จัดการข้อมูลและการแสดงผลของโรงเรียน"
	backHref="/staff"
>
	{#if !canReadSettings}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูการตั้งค่าโรงเรียน"
			description="บัญชีนี้เข้า module ตั้งค่าได้ แต่ยังไม่มีสิทธิ์อ่านข้อมูลการตั้งค่าโรงเรียน"
		/>
	{:else if loading}
		<PageSkeleton variant="form" rows={3} />
	{:else}
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
						{#if previewUrl ?? logoUrl}
							<img
								src={previewUrl ?? logoUrl}
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
									if (previewUrl) URL.revokeObjectURL(previewUrl);
									pendingFile = undefined;
									previewUrl = undefined;
								}}
								disabled={saving}
							>
								<X class="w-4 h-4" /> ยกเลิก
							</Button>
						{:else if logoUrl}
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
</PageShell>
