<script lang="ts">
	import { onMount } from 'svelte';
	import { Button } from '$lib/components/ui/button';
	import { Label } from '$lib/components/ui/label';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { ArrowLeft, Save, Upload, ImageOff, X } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { getSchoolSettings, updateSchoolSettings, deleteSchoolLogo } from '$lib/api/school';
	import { apiClient } from '$lib/api/client';

	let logoUrl = $state<string | undefined>(undefined);    // URL สำหรับ preview (จาก backend)
	let logoPath = $state<string | undefined>(undefined);   // storage_path ที่เก็บใน DB
	let logoFileId = $state<string | undefined>(undefined); // file ID สำหรับลบ
	let saving = $state(false);
	let loading = $state(true);
	let pendingFile = $state<File | undefined>(undefined);
	let previewUrl = $state<string | undefined>(undefined);

	onMount(async () => {
		try {
			const s = await getSchoolSettings();
			logoUrl = s.logoUrl;
			logoFileId = s.logoFileId;
		} catch (err) {
			toast.error('ไม่สามารถโหลดข้อมูลได้');
		} finally {
			loading = false;
		}
	});

	function handleLogoSelect(e: Event) {
		const file = (e.target as HTMLInputElement).files?.[0];
		if (!file) return;
		if (previewUrl) URL.revokeObjectURL(previewUrl);
		pendingFile = file;
		previewUrl = URL.createObjectURL(file);
	}

	async function handleSave() {
		saving = true;
		try {
			let pathToSave = logoPath;
			if (pendingFile) {
				const form = new FormData();
				form.append('file', pendingFile);
				form.append('file_type', 'school_logo');
				form.append('is_public', 'true');
				const res = await apiClient.postMultipart<never>('/api/files/upload', form);
				const uploaded = (res as unknown as { file?: { id: string; url: string; storage_path: string } }).file;
				if (!res.success || !uploaded) throw new Error(res.error ?? 'อัปโหลดไม่สำเร็จ');
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
</script>

<svelte:head>
	<title>ตั้งค่าโรงเรียน - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center gap-4">
		<Button variant="ghost" size="icon" onclick={() => goto(resolve('/staff'))}>
			<ArrowLeft class="h-5 w-5" />
		</Button>
		<div>
			<h1 class="text-3xl font-bold text-foreground">ตั้งค่าโรงเรียน</h1>
			<p class="text-muted-foreground mt-1">จัดการข้อมูลและการแสดงผลของโรงเรียน</p>
		</div>
	</div>

	<!-- Logo Card -->
	<Card class="max-w-lg">
		<CardHeader>
			<CardTitle>Logo โรงเรียน</CardTitle>
			<CardDescription>แสดงบนหน้ารับสมัครนักเรียนและหน้าอื่นๆ ของระบบ</CardDescription>
		</CardHeader>
		<CardContent class="space-y-6">
			{#if loading}
				<div class="flex justify-center py-4">
					<div class="animate-pulse w-24 h-24 rounded-2xl bg-muted"></div>
				</div>
			{:else}
				<!-- Preview -->
				<div class="flex flex-col items-center gap-3">
					<div
						class="w-24 h-24 rounded-2xl border-2 border-dashed border-border flex items-center justify-center bg-muted overflow-hidden"
					>
						{#if previewUrl ?? logoUrl}
							<img src={previewUrl ?? logoUrl} alt="school logo" class="w-full h-full object-contain p-1" />
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
							onclick={async () => {
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
							}}
							disabled={saving}
						>
							ลบ logo
						</Button>
					{/if}
				</div>
			{/if}
		</CardContent>
	</Card>
</div>
