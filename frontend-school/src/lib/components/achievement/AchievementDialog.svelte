<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import {
		Dialog,
		DialogContent,
		DialogDescription,
		DialogFooter,
		DialogHeader,
		DialogTitle
	} from '$lib/components/ui/dialog';
	import { LoaderCircle, Upload, X } from 'lucide-svelte';
	import type { Achievement } from '$lib/types/achievement';
	import { toast } from 'svelte-sonner';

	export let open: boolean = false;
	export let achievement: Achievement | null = null;
	export let userId: string; // Target user ID

	let loading = false;
	let title = '';
	let description = '';
	let date = new Date().toISOString().split('T')[0];
	let imageFile: File | null = null;
	let imagePreview: string | null = null;
	let currentImagePath: string | null = null;

	const dispatch = createEventDispatcher();

    // Reset or Load form when dialog opens/changes
	$: if (open) {
		if (achievement) {
			title = achievement.title;
			description = achievement.description || '';
			date = achievement.achievement_date;
			currentImagePath = achievement.image_path || null;
			imagePreview = null;
			imageFile = null;
		} else {
            // Reset for create mode
			title = '';
			description = '';
			date = new Date().toISOString().split('T')[0];
			currentImagePath = null;
			imagePreview = null;
			imageFile = null;
		}
	}

	function handleFileChange(e: Event) {
		const input = e.target as HTMLInputElement;
		if (input.files && input.files[0]) {
			const file = input.files[0];
            // Simple validation
			if (file.size > 5 * 1024 * 1024) {
				toast.error('ขนาดไฟล์ต้องไม่เกิน 5MB');
				return;
			}
			imageFile = file;
			imagePreview = URL.createObjectURL(file);
		}
	}

	function removeImage() {
		imageFile = null;
		imagePreview = null;
		currentImagePath = null;
	}

	async function handleSubmit() {
		if (!title || !date) {
			toast.error('กรุณากรอกข้อมูลที่จำเป็น');
			return;
		}

		loading = true;
		try {
			let imagePath = currentImagePath;

			// Upload image if selected
			if (imageFile) {
				const formData = new FormData();
				formData.append('file', imageFile);
				formData.append('folder', 'achievements'); // Optional: organize files

				const res = await fetch('/api/files/upload', {
					method: 'POST',
					body: formData
				});

				const uploadData = await res.json();
				if (!uploadData.success) {
					throw new Error(uploadData.error || 'Failed to upload image');
				}
				imagePath = uploadData.url; // Use the returned URL/Path
			}

			dispatch('save', {
				id: achievement?.id,
				user_id: userId,
				title,
				description,
				achievement_date: date,
				image_path: imagePath
			});
            
            // Wait for parent to close or handle state
		} catch (e) {
			console.error(e);
			toast.error('เกิดข้อผิดพลาดในการบันทึก');
            loading = false;
		}
	}
</script>

<Dialog
	bind:open
	onOpenChange={(v) => {
		if (!v) dispatch('close');
	}}
>
	<DialogContent class="sm:max-w-[500px]">
		<DialogHeader>
			<DialogTitle>{achievement ? 'แก้ไขผลงาน' : 'เพิ่มผลงานใหม่'}</DialogTitle>
			<DialogDescription>
				บันทึกรายละเอียดรางวัล เกียรติบัตร หรือผลงานที่น่าภาคภูมิใจ
			</DialogDescription>
		</DialogHeader>

		<div class="grid gap-4 py-4">
			<div class="grid gap-2">
				<Label for="title" class="required">ชื่อผลงาน / รางวัล</Label>
				<Input
					id="title"
					bind:value={title}
					placeholder="เช่น รางวัลครูดีเด่นประจำปี 2567"
					required
				/>
			</div>

			<div class="grid gap-2">
				<Label for="date">วันที่ได้รับ</Label>
				<Input id="date" type="date" bind:value={date} required />
			</div>

			<div class="grid gap-2">
				<Label for="description">รายละเอียดเพิ่มเติม</Label>
				<Textarea
					id="description"
					bind:value={description}
					placeholder="รายละเอียดของผลงาน หน่วยงานที่มอบ หรือหมายเหตุอื่นๆ"
					rows={3}
				/>
			</div>

			<div class="grid gap-2">
				<Label>รูปภาพประกอบ / เกียรติบัตร</Label>

				{#if imagePreview || currentImagePath}
					<div
						class="relative aspect-video w-full rounded-lg overflow-hidden border bg-muted group"
					>
						<img
							src={imagePreview ||
								(currentImagePath?.startsWith('http')
									? currentImagePath
									: `/api/files?path=${currentImagePath}`)}
							alt="Preview"
							class="w-full h-full object-cover"
						/>
						<button
							type="button"
							class="absolute top-2 right-2 bg-destructive text-white p-1 rounded-full opacity-0 group-hover:opacity-100 transition-opacity"
							onclick={removeImage}
						>
							<X class="w-4 h-4" />
						</button>
					</div>
				{:else}
					<div
						class="border-2 border-dashed rounded-lg p-6 flex flex-col items-center justify-center text-muted-foreground hover:bg-muted/50 transition-colors cursor-pointer relative"
					>
						<input
							type="file"
							accept="image/*"
							class="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
							onchange={handleFileChange}
						/>
						<Upload class="w-8 h-8 mb-2" />
						<span class="text-sm">คลิกเพื่ออัปโหลดรูปภาพ</span>
						<span class="text-xs text-muted-foreground mt-1">PNG, JPG, WebP ไม่เกิน 5MB</span>
					</div>
				{/if}
			</div>
		</div>

		<DialogFooter>
			<Button variant="outline" onclick={() => dispatch('close')} disabled={loading}>ยกเลิก</Button>
			<Button onclick={handleSubmit} disabled={loading} class="w-[120px]">
				{#if loading}
					<LoaderCircle class="w-4 h-4 mr-2 animate-spin" />
					บันทึก...
				{:else}
					บันทึก
				{/if}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>

<style>
    .required::after {
        content: " *";
        color: hsl(var(--destructive));
    }
</style>
