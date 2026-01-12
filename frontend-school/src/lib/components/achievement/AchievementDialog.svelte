<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
    import { DatePicker } from '$lib/components/ui/date-picker';
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
	import { achievementSchema } from '$lib/validation/schemas';
	import type { z } from 'zod';

	interface Props {
		open: boolean;
		achievement: Achievement | null;
		userId: string;
        canSelectUser?: boolean;
	}

	let { open = $bindable(false), achievement = null, userId, canSelectUser = false }: Props = $props();

	let loading = $state(false);
	
	// Form State
	let title = $state('');
	let description = $state('');
	let date = $state(new Date().toISOString().split('T')[0]);
	let imageFile = $state<File | null>(null);
	let imagePreview = $state<string | null>(null);
	let currentImagePath = $state<string | null>(null);
    let targetUserId = $state(''); // For selecting user
	
	// Validation State
	let errors = $state<Record<string, string>>({});

    // Staff List for selection
    import { listStaff, type StaffListItem } from '$lib/api/staff';
    import { uploadFile } from '$lib/api/files';
    import * as Popover from '$lib/components/ui/popover';
    import * as Command from '$lib/components/ui/command';
    import { Check, ChevronsUpDown } from 'lucide-svelte';
    import { cn } from '$lib/utils';
    import { tick } from 'svelte';

    let staffList = $state<StaffListItem[]>([]);
    let openCombobox = $state(false);
    let triggerRef = $state<HTMLButtonElement>(null!);

	const dispatch = createEventDispatcher();

    // Reset or Load form when dialog opens/changes
	$effect(() => {
		if (open) {
            // Load staff list if can select user
            if (canSelectUser && staffList.length === 0) {
                listStaff({ page_size: 1000 }).then(res => {
                    if(res.success) staffList = res.data;
                });
            }

            loading = false; // Reset loading state
			if (achievement) {
				title = achievement.title;
				description = achievement.description || '';
				date = achievement.achievement_date;
				currentImagePath = achievement.image_path || null;
                targetUserId = achievement.user_id;
				imagePreview = null;
				imageFile = null;
			} else {
				// Reset for create mode
				title = '';
				description = '';
				date = new Date().toISOString().split('T')[0];
				currentImagePath = null;
                targetUserId = userId || '';
				imagePreview = null;
				imageFile = null;
			}
			errors = {};
		}
	});

    async function compressImage(file: File): Promise<File> {
        // Basic configuration
        const MAX_WIDTH = 1920;
        const QUALITY = 0.8;
        
        // Skip non-images
        if (!file.type.startsWith('image/')) return file;
        // Skip small images (e.g. < 500KB)
        if (file.size < 500 * 1024) return file;

        return new Promise((resolve) => {
            const reader = new FileReader();
            reader.readAsDataURL(file);
            reader.onload = (e) => {
                const img = new Image();
                img.src = e.target?.result as string;
                img.onload = () => {
                   // Calculate new dimensions
                   let w = img.width;
                   let h = img.height;
                   
                   if (w > MAX_WIDTH) {
                       h = Math.round(h * (MAX_WIDTH / w));
                       w = MAX_WIDTH;
                   }
                   
                   const canvas = document.createElement('canvas');
                   canvas.width = w;
                   canvas.height = h;
                   const ctx = canvas.getContext('2d');
                   if(!ctx) { resolve(file); return; }
                   
                   // Draw
                   ctx.drawImage(img, 0, 0, w, h);
                   
                   // Export
                   canvas.toBlob((blob) => {
                       if (!blob) { resolve(file); return; }
                       
                       // Convert to File (Force JPEG for better compression)
                       const newName = file.name.replace(/\.[^/.]+$/, "") + ".jpg";
                       const processedFile = new File([blob], newName, {
                           type: 'image/jpeg',
                           lastModified: Date.now()
                       });
                       
                       // If compressed is somehow bigger, keep original (unless original was not jpg)
                       if (processedFile.size > file.size && file.type === 'image/jpeg') {
                           resolve(file);
                       } else {
                           resolve(processedFile);
                       }
                   }, 'image/jpeg', QUALITY);
                };
                img.onerror = () => resolve(file); // Fallback to original
            };
            reader.onerror = () => resolve(file);
        });
    }

	async function handleFileChange(e: Event) {
		const input = e.target as HTMLInputElement;
		if (input.files && input.files[0]) {
			const file = input.files[0];
            
            // Allow larger input files (e.g. 25MB) because we will compress
			if (file.size > 25 * 1024 * 1024) {
				toast.error('ไฟล์ต้นฉบับต้องไม่เกิน 25MB');
				return;
			}
            
            const toastId = toast.loading('กำลังประมวลผลรูปภาพ...');
            try {
			    const compressed = await compressImage(file);
                
                // Final check
                if (compressed.size > 5 * 1024 * 1024) {
                    toast.error('ไฟล์มีขนาดใหญ่เกินไป (แม้หลังบีบอัด) กรุณาใช้ไฟล์อื่น');
                    return;
                }
                
                imageFile = compressed;
                imagePreview = URL.createObjectURL(compressed);
                
                // Show success if compression happened significantly
                if (compressed.size < file.size * 0.9) {
                     toast.success(`ลดขนาดไฟล์เหลือ ${(compressed.size / 1024 / 1024).toFixed(2)} MB`);
                }
            } catch (err) {
                console.error(err);
                toast.error('ไม่สามารถประมวลผลรูปภาพได้');
            } finally {
                toast.dismiss(toastId);
            }
		}
	}

	function removeImage() {
		imageFile = null;
		imagePreview = null;
		currentImagePath = null;
	}

    // Helper to get selected staff name
    function getSelectedStaffName() {
        if (!targetUserId) return 'เลือกบุคลากร';
        const staff = staffList.find(s => s.id === targetUserId);
        return staff ? `${staff.first_name} ${staff.last_name}` : 'เลือกบุคลากร';
    }

	async function handleSubmit() {
		errors = {};
		
        if (canSelectUser && !targetUserId) {
            errors.targetUserId = 'กรุณาเลือกบุคลากร';
			toast.error('กรุณาเลือกบุคลากร');
            return;
        }

		// 1. Validate with Zod
		const result = achievementSchema.safeParse({
			title,
			achievement_date: date,
			description,
			image_path: currentImagePath || '' 
		});

		if (!result.success) {
			const formattedErrors: Record<string, string> = {};
			const fieldErrors = result.error.flatten().fieldErrors;
			
			Object.entries(fieldErrors).forEach(([key, messages]) => {
				if (messages && messages.length > 0) {
					formattedErrors[key] = messages[0];
				}
			});

			errors = formattedErrors;
			toast.error('กรุณาตรวจสอบข้อมูลให้ถูกต้อง');
			return;
		}

		loading = true;
		try {
			let imagePath = currentImagePath;

			// Upload image if selected
			if (imageFile) {
                const uploadData = await uploadFile(imageFile, 'other', false);

				if (!uploadData.success) {
					throw new Error('Failed to upload image');
				}
				imagePath = uploadData.file.url;
			}

			dispatch('save', {
				id: achievement?.id,
				user_id: canSelectUser && targetUserId ? targetUserId : userId,
				title,
				description,
				achievement_date: date,
				image_path: imagePath
			});
            
            loading = false;
            
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
			{#if canSelectUser}
				<div class="grid gap-2">
					<Label class="required">เจ้าของผลงาน</Label>
					<Popover.Root bind:open={openCombobox}>
						<Popover.Trigger bind:ref={triggerRef}>
							{#snippet child({ props })}
								<Button
									variant="outline"
									class="w-full justify-between"
									{...props}
									role="combobox"
									aria-expanded={openCombobox}
								>
									{getSelectedStaffName()}
									<ChevronsUpDown class="ml-2 h-4 w-4 shrink-0 opacity-50" />
								</Button>
							{/snippet}
						</Popover.Trigger>
						<Popover.Content class="w-full p-0">
							<Command.Root>
								<Command.Input placeholder="ค้นหาบุคลากร..." />
								<Command.List>
									<Command.Empty>ไม่พบรายชื่อ</Command.Empty>
									<Command.Group>
										{#each staffList as staff (staff.id)}
											<Command.Item
												value={staff.id}
												onSelect={() => {
													targetUserId = staff.id;
													openCombobox = false;
												}}
											>
												<Check
													class={cn(
														'mr-2 h-4 w-4',
														targetUserId === staff.id ? 'opacity-100' : 'opacity-0'
													)}
												/>
												{staff.first_name}
												{staff.last_name}
											</Command.Item>
										{/each}
									</Command.Group>
								</Command.List>
							</Command.Root>
						</Popover.Content>
					</Popover.Root>
					{#if errors.targetUserId}
						<p class="text-xs text-destructive">{errors.targetUserId}</p>
					{/if}
				</div>
			{/if}

			<div class="grid gap-2">
				<Label for="title" class="required">ชื่อผลงาน / รางวัล</Label>
				<Input
					id="title"
					bind:value={title}
					placeholder="เช่น รางวัลครูดีเด่นประจำปี 2567"
					class={errors.title ? 'border-destructive focus-visible:ring-destructive' : ''}
				/>
				{#if errors.title}
					<p class="text-xs text-destructive">{errors.title}</p>
				{/if}
			</div>

			<div class="grid gap-2">
				<Label for="date">วันที่ได้รับ</Label>
				<DatePicker bind:value={date} placeholder="เลือกวันที่ได้รับผลงาน" />
				{#if errors.achievement_date}
					<p class="text-xs text-destructive">{errors.achievement_date}</p>
				{/if}
			</div>

			<div class="grid gap-2">
				<Label for="description">รายละเอียดเพิ่มเติม</Label>
				<Textarea
					id="description"
					bind:value={description}
					placeholder="รายละเอียดของผลงาน หน่วยงานที่มอบ หรือหมายเหตุอื่นๆ"
					rows={3}
					class={errors.description ? 'border-destructive focus-visible:ring-destructive' : ''}
				/>
				{#if errors.description}
					<p class="text-xs text-destructive">{errors.description}</p>
				{/if}
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
							accept="image/png, image/jpeg, image/webp"
							class="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
							onchange={handleFileChange}
						/>
						<Upload class="w-8 h-8 mb-2" />
						<span class="text-sm">คลิกเพื่ออัปโหลดรูปภาพ</span>
						<span class="text-xs text-muted-foreground mt-1">PNG, JPG, WebP ไม่เกิน 25MB</span>
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
    :global(.required)::after {
        content: " *";
        color: hsl(var(--destructive));
    }
</style>
