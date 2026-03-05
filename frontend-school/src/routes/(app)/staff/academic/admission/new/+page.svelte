<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { createRound } from '$lib/api/admission';
	import { apiClient } from '$lib/api/client';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { toast } from 'svelte-sonner';
	import { ArrowLeft, Plus } from 'lucide-svelte';

	let { data } = $props();

	interface AcademicYear {
		id: string;
		name: string;
		year: number;
	}
	interface GradeLevel {
		id: string;
		levelType: string;
		year: number;
		name?: string;
	}

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
		const [yRes, gRes] = await Promise.all([
			apiClient.get<AcademicYear[]>('/api/academic/years'),
			apiClient.get<GradeLevel[]>('/api/academic/grade-levels')
		]);
		if (yRes.success) years = yRes.data ?? [];
		if (gRes.success) gradeLevels = gRes.data ?? [];
	}

	function gradeName(g: GradeLevel) {
		const prefix = g.levelType === 'kindergarten' ? 'อ.' : g.levelType === 'primary' ? 'ป.' : 'ม.';
		return `${prefix}${g.year}`;
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

	<form onsubmit={handleSubmit} class="bg-card border border-border rounded-lg p-6 space-y-5">
		<!-- ปีการศึกษา -->
		<div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
			<div class="space-y-1.5">
				<label class="text-sm font-medium" for="year">ปีการศึกษา *</label>
				<select
					id="year"
					bind:value={form.academicYearId}
					class="w-full px-3 py-2 rounded-md border border-border bg-background text-sm"
				>
					<option value="">-- เลือกปีการศึกษา --</option>
					{#each years as y (y.id)}
						<option value={y.id}>{y.name}</option>
					{/each}
				</select>
			</div>
			<div class="space-y-1.5">
				<label class="text-sm font-medium" for="grade">ระดับชั้น *</label>
				<select
					id="grade"
					bind:value={form.gradeLevelId}
					class="w-full px-3 py-2 rounded-md border border-border bg-background text-sm"
				>
					<option value="">-- เลือกระดับชั้น --</option>
					{#each gradeLevels as g (g.id)}
						<option value={g.id}>{gradeName(g)}</option>
					{/each}
				</select>
			</div>
		</div>

		<!-- ชื่อรอบ -->
		<div class="space-y-1.5">
			<label class="text-sm font-medium" for="name">ชื่อรอบรับสมัคร *</label>
			<Input
				id="name"
				bind:value={form.name}
				placeholder="เช่น รับสมัครนักเรียน ม.1 ปีการศึกษา 2569"
				required
			/>
		</div>

		<!-- คำอธิบาย -->
		<div class="space-y-1.5">
			<label class="text-sm font-medium" for="desc">คำอธิบาย (ไม่บังคับ)</label>
			<textarea
				id="desc"
				bind:value={form.description}
				rows="2"
				class="w-full px-3 py-2 rounded-md border border-border bg-background text-sm resize-none"
				placeholder="รายละเอียดเพิ่มเติม..."
			></textarea>
		</div>

		<!-- ช่วงรับสมัคร -->
		<div class="space-y-2">
			<p class="text-sm font-semibold text-muted-foreground">ช่วงรับสมัคร *</p>
			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-1">
					<label class="text-xs text-muted-foreground" for="apply-start">เริ่ม</label>
					<Input id="apply-start" type="date" bind:value={form.applyStartDate} required />
				</div>
				<div class="space-y-1">
					<label class="text-xs text-muted-foreground" for="apply-end">สิ้นสุด</label>
					<Input id="apply-end" type="date" bind:value={form.applyEndDate} required />
				</div>
			</div>
		</div>

		<!-- วันสอบ -->
		<div class="grid grid-cols-2 gap-4">
			<div class="space-y-1.5">
				<label class="text-sm font-medium" for="exam-date">วันสอบ</label>
				<Input id="exam-date" type="date" bind:value={form.examDate} />
			</div>
			<div class="space-y-1.5">
				<label class="text-sm font-medium" for="result-date">วันประกาศผล</label>
				<Input id="result-date" type="date" bind:value={form.resultAnnounceDate} />
			</div>
		</div>

		<!-- ช่วงมอบตัว -->
		<div class="space-y-2">
			<p class="text-sm font-semibold text-muted-foreground">ช่วงมอบตัว</p>
			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-1">
					<label class="text-xs text-muted-foreground" for="enroll-start">เริ่ม</label>
					<Input id="enroll-start" type="date" bind:value={form.enrollmentStartDate} />
				</div>
				<div class="space-y-1">
					<label class="text-xs text-muted-foreground" for="enroll-end">สิ้นสุด</label>
					<Input id="enroll-end" type="date" bind:value={form.enrollmentEndDate} />
				</div>
			</div>
		</div>

		<div class="flex gap-3 pt-2">
			<Button type="submit" disabled={saving} class="flex items-center gap-2">
				<Plus class="w-4 h-4" />
				{saving ? 'กำลังสร้าง...' : 'สร้างรอบรับสมัคร'}
			</Button>
			<Button type="button" variant="outline" href="/staff/academic/admission">ยกเลิก</Button>
		</div>
	</form>
</div>
