<script lang="ts">
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { createRound } from '$lib/api/admission';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
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
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import * as Card from '$lib/components/ui/card';
	import * as Select from '$lib/components/ui/select';
	import { Separator } from '$lib/components/ui/separator';
	import DatePicker from '$lib/components/ui/date-picker/DatePicker.svelte';
	import { toast } from 'svelte-sonner';
	import { Plus, Loader2 } from '@lucide/svelte';
	import { can } from '$lib/stores/permissions';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const canManageAdmission = $derived($can.has(PERMISSIONS.ADMISSION_MANAGE_ALL));
	const yearsRequest = new LatestRequest();
	const gradesRequest = new LatestRequest();

	function goToAdmissionRound(id: string) {
		goto(resolve(`/staff/academic/admission/${id}`));
	}

	let years: AcademicYearLookupItem[] = $state([]);
	let gradeLevels: GradeLevelLookupItem[] = $state([]);
	let loading = $state(false);
	let loadingGrades = $state(false);
	let gradeError = $state('');
	let saving = $state(false);
	let error = $state('');

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
		if (!canManageAdmission) return;
		const { revision, signal } = gradesRequest.begin();
		if (!yearId) {
			gradeLevels = [];
			loadingGrades = false;
			return;
		}
		loadingGrades = true;
		gradeError = '';
		gradeLevels = [];
		try {
			const rows = await lookupGradeLevels({ academicYearId: yearId }, { signal });
			if (gradesRequest.isCurrent(revision)) gradeLevels = rows;
		} catch (cause) {
			if (!isAbortError(cause) && gradesRequest.isCurrent(revision))
				gradeError = cause instanceof Error ? cause.message : 'โหลดระดับชั้นไม่สำเร็จ';
		} finally {
			if (gradesRequest.isCurrent(revision)) loadingGrades = false;
		}
	}

	async function load() {
		if (!canManageAdmission) return;
		const { revision, signal } = yearsRequest.begin();
		loading = true;
		error = '';
		try {
			const rows = await lookupAcademicYears({ activeOnly: false }, { signal });
			if (!yearsRequest.isCurrent(revision)) return;
			years = rows;
			const activeYear =
				rows.find((year) => year.id === data.preferredYearId) ??
				rows.find((year) => year.status === 'active') ??
				rows[0];
			if (activeYear) {
				form.academicYearId = activeYear.id;
				await loadGradeLevels(activeYear.id);
			}
		} catch (loadError) {
			if (!isAbortError(loadError) && yearsRequest.isCurrent(revision))
				error = loadError instanceof Error ? loadError.message : 'โหลดข้อมูลปีการศึกษาไม่สำเร็จ';
		} finally {
			if (yearsRequest.isCurrent(revision)) loading = false;
		}
	}

	async function handleSubmit(e: Event) {
		e.preventDefault();
		if (!canManageAdmission) {
			toast.error('ไม่มีสิทธิ์สร้างรอบรับสมัคร');
			return;
		}
		if (
			!form.academicYearId ||
			!form.gradeLevelId ||
			!gradeLevels.some((grade) => grade.id === form.gradeLevelId) ||
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

	$effect.pre(() => {
		const routeYears = data.years;
		const routeGrades = data.grades;
		const { revision: yearRevision } = yearsRequest.begin();
		const { revision: gradeRevision } = gradesRequest.begin();
		untrack(() => {
			years = [];
			gradeLevels = [];
			loading = true;
			loadingGrades = true;
			error = '';
			gradeError = '';
		});
		void routeYears.then((result) => {
			if (!yearsRequest.isCurrent(yearRevision)) return;
			untrack(() => {
				if (result.ok) {
					years = result.data;
					const activeYear =
						result.data.find((year) => year.id === data.preferredYearId) ??
						result.data.find((year) => year.status === 'active') ??
						result.data[0];
					const nextYearId = activeYear?.id ?? '';
					if (nextYearId !== form.academicYearId) form.gradeLevelId = '';
					form.academicYearId = nextYearId;
				} else error = result.error;
				loading = false;
			});
		});
		void routeGrades.then((result) => {
			if (!gradesRequest.isCurrent(gradeRevision)) return;
			untrack(() => {
				if (result.ok && result.data.yearId === form.academicYearId) gradeLevels = result.data.rows;
				else if (!result.ok) gradeError = result.error;
				loadingGrades = false;
			});
		});
		return () => {
			yearsRequest.abort();
			gradesRequest.abort();
		};
	});
</script>

<PageShell
	title="สร้างรอบรับสมัครใหม่"
	description="กรอกข้อมูลสำหรับเปิดรอบรับสมัครนักเรียนใหม่"
	backHref="/staff/academic/admission"
>
	{#if !canManageAdmission}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์สร้างรอบรับสมัคร"
			description="บัญชีนี้เข้า module รับสมัครได้ แต่ยังไม่มีสิทธิ์จัดการรอบรับสมัคร"
		/>
	{:else if loading}
		<PageSkeleton variant="form" rows={7} />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดข้อมูลสำหรับสร้างรอบไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={load}
		/>
	{:else}
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
								value={form.academicYearId}
								onValueChange={(v) => {
									const nextYearId = v ?? '';
									if (nextYearId === form.academicYearId) return;
									form.academicYearId = nextYearId;
									form.gradeLevelId = '';
									void loadGradeLevels(nextYearId);
								}}
							>
								<Select.Trigger id="year-select" class="w-full">
									{years.find((y) => y.id === form.academicYearId)?.name ?? '-- เลือกปีการศึกษา --'}
								</Select.Trigger>
								<Select.Content>
									{#each years as y (y.id)}
										<Select.Item value={y.id}>
											{y.name}{y.status === 'active' ? ' (กำลังใช้งาน)' : ''}
										</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>
						<div class="space-y-2">
							<Label for="grade-select">ระดับชั้น <span class="text-destructive">*</span></Label>
							{#if gradeError}<p role="alert" class="text-sm text-destructive">
									{gradeError}
									<Button
										variant="outline"
										size="sm"
										onclick={() => void loadGradeLevels(form.academicYearId)}>ลองใหม่</Button
									>
								</p>{/if}
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
	{/if}
</PageShell>
