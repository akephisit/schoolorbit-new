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
	import { ArrowLeft, Save, Upload, ImageOff } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { getSchoolSettings, updateSchoolSettings } from '$lib/api/school';
	import { apiClient } from '$lib/api/client';

	let logoUrl = $state<string | undefined>(undefined);
	let savingLogo = $state(false);
	let uploadingLogo = $state(false);
	let loading = $state(true);

	onMount(async () => {
		try {
			const s = await getSchoolSettings();
			logoUrl = s.logoUrl;
		} catch (err) {
			toast.error('ไม่สามารถโหลดข้อมูลได้');
		} finally {
			loading = false;
		}
	});

	async function handleLogoUpload(e: Event) {
		const file = (e.target as HTMLInputElement).files?.[0];
		if (!file) return;
		uploadingLogo = true;
		try {
			const form = new FormData();
			form.append('file', file);
			form.append('file_type', 'school_logo');
			form.append('is_public', 'true');
			const res = await apiClient.postMultipart<{ file: { url: string } }>('/api/files/upload', form);
			if (!res.success || !res.data?.file?.url) throw new Error(res.error ?? 'อัปโหลดไม่สำเร็จ');
			logoUrl = res.data.file.url;
			toast.success('อัปโหลด logo สำเร็จ กด "บันทึก" เพื่อยืนยัน');
		} catch (err) {
			toast.error(err instanceof Error ? err.message : 'อัปโหลดไม่สำเร็จ');
		} finally {
			uploadingLogo = false;
		}
	}

	async function handleSave() {
		savingLogo = true;
		try {
			await updateSchoolSettings({ logoUrl });
			toast.success('บันทึกการตั้งค่าสำเร็จ');
		} catch (err) {
			toast.error(err instanceof Error ? err.message : 'บันทึกไม่สำเร็จ');
		} finally {
			savingLogo = false;
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
						{#if logoUrl}
							<img src={logoUrl} alt="school logo" class="w-full h-full object-contain p-1" />
						{:else}
							<ImageOff class="w-8 h-8 text-muted-foreground" />
						{/if}
					</div>
					<p class="text-xs text-muted-foreground">ตัวอย่าง logo</p>
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
							onchange={handleLogoUpload}
							disabled={uploadingLogo}
						/>
						<Button variant="outline" class="gap-2 pointer-events-none" disabled={uploadingLogo}>
							<Upload class="w-4 h-4" />
							{uploadingLogo ? 'กำลังอัปโหลด...' : 'เลือกไฟล์'}
						</Button>
					</label>
				</div>

				<!-- Actions -->
				<div class="flex items-center gap-3 pt-2">
					<Button onclick={handleSave} disabled={savingLogo} class="gap-2">
						<Save class="w-4 h-4" />
						{savingLogo ? 'กำลังบันทึก...' : 'บันทึก'}
					</Button>
					{#if logoUrl}
						<Button
							variant="ghost"
							class="text-destructive hover:text-destructive"
							onclick={() => (logoUrl = undefined)}
							disabled={savingLogo}
						>
							ลบ logo
						</Button>
					{/if}
				</div>
			{/if}
		</CardContent>
	</Card>
</div>
