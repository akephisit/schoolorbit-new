<script lang="ts">
	import type { Student } from '#lib/api/students.js';
	import { Card } from '#lib/components/ui/card/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import PrivateFileImage from '#lib/components/files/PrivateFileImage.svelte';
	import { GraduationCap, User } from '@lucide/svelte';

	let { student, yearName }: { student: Student; yearName?: string } = $props();
	const yearLabel = $derived(yearName?.trim());
</script>

<Card
	class="overflow-hidden border-primary/20 bg-linear-to-br from-primary/10 via-card to-card p-5 sm:p-6"
>
	<div class="flex items-start gap-4 sm:gap-6">
		<div
			class="flex size-16 shrink-0 items-center justify-center overflow-hidden rounded-2xl border bg-background sm:size-20"
		>
			{#if student.profile_image_file_id}
				<PrivateFileImage
					fileId={student.profile_image_file_id}
					resourceId={student.id}
					alt={`ภาพของ ${student.first_name}`}
					class="size-full object-cover"
				/>
			{:else}<User class="size-8 text-primary" />{/if}
		</div>
		<div class="min-w-0 flex-1 space-y-3">
			<p class="flex items-center gap-2 text-sm font-medium text-primary">
				<GraduationCap class="size-4 shrink-0" />ข้อมูลนักเรียน
			</p>
			<h2 class="break-words text-xl font-semibold sm:text-2xl">
				{student.title ?? ''}{student.first_name}
				{student.last_name}
			</h2>
			{#if yearLabel}<Badge variant="secondary"
					>{yearLabel.startsWith('ปีการศึกษา') ? yearLabel : `ปีการศึกษา ${yearLabel}`}</Badge
				>{/if}
		</div>
	</div>
	<dl class="mt-6 grid grid-cols-2 gap-x-6 gap-y-4 border-t border-primary/10 pt-5 sm:grid-cols-4">
		<div>
			<dt class="text-sm text-muted-foreground">รหัสนักเรียน</dt>
			<dd class="mt-1 break-words font-semibold">{student.student_id || 'ไม่ระบุ'}</dd>
		</div>
		<div>
			<dt class="text-sm text-muted-foreground">ชั้นเรียน</dt>
			<dd class="mt-1 break-words font-semibold">{student.grade_level || 'ไม่ระบุ'}</dd>
		</div>
		<div>
			<dt class="text-sm text-muted-foreground">ห้องเรียน</dt>
			<dd class="mt-1 break-words font-semibold">{student.homeroom || 'ไม่ระบุ'}</dd>
		</div>
		<div>
			<dt class="text-sm text-muted-foreground">เลขที่</dt>
			<dd class="mt-1 font-semibold">{student.student_number ?? 'ไม่ระบุ'}</dd>
		</div>
	</dl>
</Card>
