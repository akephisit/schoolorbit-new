<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { createRound } from '$lib/api/admission';
	import { getAcademicStructure, type AcademicYear, type GradeLevel } from '$lib/api/academic';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import * as Card from '$lib/components/ui/card';
	import * as Select from '$lib/components/ui/select';
	import { Separator } from '$lib/components/ui/separator';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, Plus, Loader2 } from 'lucide-svelte';

	let { data } = $props();

	let years: AcademicYear[] = $state([]);
	let gradeLevels: GradeLevel[] = $state([]);
	let saving = $state(false);

	let form = $state({
		academicYearId: '',
		gradeLevelId: '',
		name: '',
		description: '',
		applyStartDate: '',
		applyEndDate: '',
		examDate: '',
		resultAnnounceDate: '',
		enrollmentStartDate: '',
		enrollmentEndDate: ''
	});

	async function load() {
		try {
			const res = await getAcademicStructure();
			years = res.data.years;
			gradeLevels = res.data.levels.filter((l) => l.is_active);
			// Auto-select active year
			const activeYear = years.find((y) => y.is_active) ?? years[0];
			if (activeYear) form.academicYearId = activeYear.id;
		} catch (e) {
			toast.error('โหลดข้อมูลปีการศึกษาไม่สำเร็จ');
		}
	}

	async function handleSubmit(e: Event) {
		e.preventDefault();
		if (
			!form.academicYearId ||
			!form.gradeLevelId ||
			!form.name ||
			!form.applyStartDate ||
			!form.applyEndDate
		) {
			toast.error('กรุณากรอกข้อมูลที่จำเป็น');
			return;
		}
		saving = true;
		try {
			const round = await createRound({
				...form,
				examDate: form.examDate || undefined,
				resultAnnounceDate: form.resultAnnounceDate || undefined,
				enrollmentStartDate: form.enrollmentStartDate || undefined,
				enrollmentEndDate: form.enrollmentEndDate || undefined,
				description: form.description || undefined
			});
			toast.success('สร้างรอบรับสมัครแล้ว');
			goto(`/staff/academic/admission/${round.id}`);
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'สร้างไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	onMount(load);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<div class="max-w-2xl mx-auto space-y-6">
	<div class="flex items-center gap-3">
		<Button href="/staff/academic/admission" variant="ghost" size="sm">
			<ArrowLeft class="w-4 h-4 mr-1" />
			ย้อนกลับ
		</Button>
		<h1 class="text-2xl font-bold text-foreground">สร้างรอบรับสมัครใหม่</h1>
	</div>

	<form onsubmit={handleSubmit}>
		<Card.Root>
			<Card.Header>
				<Card.Title>ข้อมูลรอบรับสมัคร</Card.Title>
				<Card.Description>กรอกข้อมูลสำหรับเปิดรอบรับสมัครนักเรียนใหม่</Card.Description>
			</Card.Header>
			<Card.Content class="space-y-5">
				<!-- ปีการศึกษา + ระดับชั้น -->
				<div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
					<div class="space-y-2">
						<Label for="year-select">ปีการศึกษา <span class="text-destructive">*</span></Label>
						<Select.Root type="single" bind:value={form.academicYearId}>
							<Select.Trigger id="year-select" class="w-full">
								{years.find((y) => y.id === form.academicYearId)?.name ?? '-- เลือกปีการศึกษา --'}
							</Select.Trigger>
							<Select.Content>
								{#each years as y (y.id)}
									<Select.Item value={y.id}>
										{y.name}{y.is_active ? ' (ปัจจุบัน)' : ''}
									</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="space-y-2">
						<Label for="grade-select">ระดับชั้น <span class="text-destructive">*</span></Label>
						<Select.Root type="single" bind:value={form.gradeLevelId}>
							<Select.Trigger id="grade-select" class="w-full">
								{gradeLevels.find((g) => g.id === form.gradeLevelId)?.short_name ??
									'-- เลือกระดับชั้น --'}
							</Select.Trigger>
							<Select.Content>
								{#each gradeLevels as g (g.id)}
									<Select.Item value={g.id}>{g.short_name} — {g.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
				</div>

				<!-- ชื่อรอบ -->
				<div class="space-y-2">
					<Label for="round-name">ชื่อรอบรับสมัคร <span class="text-destructive">*</span></Label>
					<Input
						id="round-name"
						bind:value={form.name}
						placeholder="เช่น รับสมัครนักเรียน ม.1 ปีการศึกษา 2569"
					/>
				</div>

				<!-- คำอธิบาย -->
				<div class="space-y-2">
					<Label for="round-desc">คำอธิบาย</Label>
					<Textarea
						id="round-desc"
						bind:value={form.description}
						placeholder="รายละเอียดเพิ่มเติม..."
						rows={2}
					/>
				</div>

				<Separator />

				<!-- ช่วงรับสมัคร -->
				<div class="space-y-3">
					<p class="text-sm font-medium">ช่วงรับสมัคร <span class="text-destructive">*</span></p>
					<div class="grid grid-cols-2 gap-4">
						<div class="space-y-2">
							<Label for="apply-start">วันเริ่มรับสมัคร</Label>
							<Input id="apply-start" type="date" bind:value={form.applyStartDate} />
						</div>
						<div class="space-y-2">
							<Label for="apply-end">วันสิ้นสุดรับสมัคร</Label>
							<Input id="apply-end" type="date" bind:value={form.applyEndDate} />
						</div>
					</div>
				</div>

				<!-- วันสอบ + ประกาศผล -->
				<div class="grid grid-cols-2 gap-4">
					<div class="space-y-2">
						<Label for="exam-date">วันสอบ</Label>
						<Input id="exam-date" type="date" bind:value={form.examDate} />
					</div>
					<div class="space-y-2">
						<Label for="result-date">วันประกาศผล</Label>
						<Input id="result-date" type="date" bind:value={form.resultAnnounceDate} />
					</div>
				</div>

				<Separator />

				<!-- ช่วงมอบตัว -->
				<div class="space-y-3">
					<p class="text-sm font-medium">ช่วงมอบตัว</p>
					<div class="grid grid-cols-2 gap-4">
						<div class="space-y-2">
							<Label for="enroll-start">วันเริ่มมอบตัว</Label>
							<Input id="enroll-start" type="date" bind:value={form.enrollmentStartDate} />
						</div>
						<div class="space-y-2">
							<Label for="enroll-end">วันสิ้นสุดมอบตัว</Label>
							<Input id="enroll-end" type="date" bind:value={form.enrollmentEndDate} />
						</div>
					</div>
				</div>
			</Card.Content>

			<Card.Footer class="flex gap-3">
				<Button type="submit" disabled={saving} class="flex items-center gap-2">
					{#if saving}
						<Loader2 class="w-4 h-4 animate-spin" />
					{:else}
						<Plus class="w-4 h-4" />
					{/if}
					{saving ? 'กำลังสร้าง...' : 'สร้างรอบรับสมัคร'}
				</Button>
				<Button type="button" variant="outline" href="/staff/academic/admission">ยกเลิก</Button>
			</Card.Footer>
		</Card.Root>
	</form>
</div>
