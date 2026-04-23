<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { createRound } from '$lib/api/admission';
	import {
		lookupAcademicYears,
		lookupGradeLevels,
		type AcademicYearLookupItem,
		type GradeLevelLookupItem
	} from '$lib/api/lookup';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import * as Card from '$lib/components/ui/card';
	import * as Select from '$lib/components/ui/select';
	import { Separator } from '$lib/components/ui/separator';
	import DatePicker from '$lib/components/ui/date-picker/DatePicker.svelte';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, Plus, Loader2 } from 'lucide-svelte';

	let { data } = $props();

	function goToAdmissionRound(id: string) {
		// eslint-disable-next-line @typescript-eslint/no-explicit-any -- SvelteKit typed route dynamic interpolation
		goto(resolve(`/staff/academic/admission/${id}` as any));
	}

	let years: AcademicYearLookupItem[] = $state([]);
	let gradeLevels: GradeLevelLookupItem[] = $state([]);
	let loadingGrades = $state(false);
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

	async function loadGradeLevels(yearId: string) {
		if (!yearId) {
			gradeLevels = [];
			return;
		}
		loadingGrades = true;
		try {
			gradeLevels = await lookupGradeLevels({ academicYearId: yearId });
		} catch {
			gradeLevels = [];
			toast.error('โหลดระดับชั้นไม่สำเร็จ');
		} finally {
			loadingGrades = false;
		}
	}

	async function load() {
		try {
			years = await lookupAcademicYears({ activeOnly: false });
			const activeYear = years.find((y) => y.is_current) ?? years[0];
			if (activeYear) {
				form.academicYearId = activeYear.id;
				await loadGradeLevels(activeYear.id);
			}
		} catch {
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
			goToAdmissionRound(String(round.id));
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
						<Select.Root
							type="single"
							bind:value={form.academicYearId}
							onValueChange={(v) => {
								form.gradeLevelId = '';
								loadGradeLevels(v ?? '');
							}}
						>
							<Select.Trigger id="year-select" class="w-full">
								{years.find((y) => y.id === form.academicYearId)?.name ?? '-- เลือกปีการศึกษา --'}
							</Select.Trigger>
							<Select.Content>
								{#each years as y (y.id)}
									<Select.Item value={y.id}>
										{y.name}{y.is_current ? ' (ปัจจุบัน)' : ''}
									</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>
					<div class="space-y-2">
						<Label for="grade-select">ระดับชั้น <span class="text-destructive">*</span></Label>
						<Select.Root
							type="single"
							bind:value={form.gradeLevelId}
							disabled={loadingGrades || !form.academicYearId}
						>
							<Select.Trigger id="grade-select" class="w-full">
								{loadingGrades
									? 'กำลังโหลด...'
									: (gradeLevels.find((g) => g.id === form.gradeLevelId)?.short_name ??
										(gradeLevels.length === 0 && form.academicYearId
											? 'ไม่มีระดับชั้นที่เปิด'
											: '-- เลือกระดับชั้น --'))}
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
						<div class="space-y-2 flex flex-col">
							<Label for="apply-start">วันเริ่มรับสมัคร</Label>
							<DatePicker bind:value={form.applyStartDate} />
						</div>
						<div class="space-y-2 flex flex-col">
							<Label for="apply-end">วันสิ้นสุดรับสมัคร</Label>
							<DatePicker bind:value={form.applyEndDate} />
						</div>
					</div>
				</div>

				<!-- วันสอบ + ประกาศผล -->
				<div class="grid grid-cols-2 gap-4">
					<div class="space-y-2 flex flex-col">
						<Label for="exam-date">วันสอบ</Label>
						<DatePicker bind:value={form.examDate} />
					</div>
					<div class="space-y-2 flex flex-col">
						<Label for="result-date">วันประกาศผล</Label>
						<DatePicker bind:value={form.resultAnnounceDate} />
					</div>
				</div>

				<Separator />

				<!-- ช่วงมอบตัว -->
				<div class="space-y-3">
					<p class="text-sm font-medium">ช่วงมอบตัว</p>
					<div class="grid grid-cols-2 gap-4">
						<div class="space-y-2 flex flex-col">
							<Label for="enroll-start">วันเริ่มมอบตัว</Label>
							<DatePicker bind:value={form.enrollmentStartDate} />
						</div>
						<div class="space-y-2 flex flex-col">
							<Label for="enroll-end">วันสิ้นสุดมอบตัว</Label>
							<DatePicker bind:value={form.enrollmentEndDate} />
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
