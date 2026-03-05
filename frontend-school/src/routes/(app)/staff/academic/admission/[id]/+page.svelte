<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import {
		getRound,
		listTracks,
		listSubjects,
		createTrack,
		updateTrack,
		deleteTrack,
		createSubject,
		updateSubject,
		deleteSubject,
		updateRoundStatus,
		type AdmissionRound,
		type AdmissionTrack,
		type AdmissionExamSubject,
		roundStatusLabel,
		roundStatusColor
	} from '$lib/api/admission';
	import { apiClient } from '$lib/api/client';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { toast } from 'svelte-sonner';
	import {
		ArrowLeft,
		Settings,
		BookOpen,
		GraduationCap,
		Plus,
		Pencil,
		Trash2,
		Check,
		X,
		ClipboardList,
		Users
	} from 'lucide-svelte';

	let id = $derived($page.params.id);
	let round: AdmissionRound | null = $state(null);
	let tracks: AdmissionTrack[] = $state([]);
	let subjects: AdmissionExamSubject[] = $state([]);
	let studyPlans: { id: string; nameTh: string }[] = $state([]);
	let loading = $state(true);

	// Track form
	let showTrackForm = $state(false);
	let editingTrack: AdmissionTrack | null = $state(null);
	let trackForm = $state({
		studyPlanId: '',
		name: '',
		capacityOverride: '',
		tiebreakMethod: 'applied_at'
	});
	let savingTrack = $state(false);

	// Subject form
	let showSubjectForm = $state(false);
	let editingSubject: AdmissionExamSubject | null = $state(null);
	let subjectForm = $state({ name: '', code: '', maxScore: '100', displayOrder: '0' });
	let savingSubject = $state(false);

	// Status update
	const statusFlow: AdmissionRound['status'][] = [
		'draft',
		'open',
		'exam',
		'scoring',
		'announced',
		'enrolling',
		'closed'
	];

	async function load() {
		if (!id) return;
		loading = true;
		try {
			const [r, t, s] = await Promise.all([getRound(id), listTracks(id), listSubjects(id)]);
			round = r;
			tracks = t;
			subjects = s;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
		// โหลด study plans
		const sp = await apiClient.get<{ id: string; nameTh: string }[]>('/api/academic/study-plans');
		if (sp.success) studyPlans = sp.data ?? [];
	}

	async function handleStatusChange(status: AdmissionRound['status']) {
		if (!round) return;
		try {
			await updateRoundStatus(round.id, status);
			toast.success(`สถานะ → ${roundStatusLabel[status]}`);
			await load();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'เปลี่ยนสถานะไม่สำเร็จ');
		}
	}

	// ===== Track =====
	function openNewTrack() {
		editingTrack = null;
		trackForm = { studyPlanId: '', name: '', capacityOverride: '', tiebreakMethod: 'applied_at' };
		showTrackForm = true;
	}
	function openEditTrack(t: AdmissionTrack) {
		editingTrack = t;
		trackForm = {
			studyPlanId: t.studyPlanId,
			name: t.name,
			capacityOverride: t.capacityOverride?.toString() ?? '',
			tiebreakMethod: t.tiebreakMethod
		};
		showTrackForm = true;
	}
	async function saveTrack() {
		if (!id) return;
		savingTrack = true;
		try {
			if (editingTrack) {
				await updateTrack(editingTrack.id, {
					name: trackForm.name,
					capacityOverride: trackForm.capacityOverride
						? parseInt(trackForm.capacityOverride)
						: undefined,
					tiebreakMethod: trackForm.tiebreakMethod as 'applied_at' | 'gpa'
				});
				toast.success('อัปเดตสายแล้ว');
			} else {
				await createTrack(id, {
					studyPlanId: trackForm.studyPlanId,
					name: trackForm.name,
					capacityOverride: trackForm.capacityOverride
						? parseInt(trackForm.capacityOverride)
						: undefined,
					tiebreakMethod: trackForm.tiebreakMethod
				});
				toast.success('เพิ่มสายการเรียนแล้ว');
			}
			showTrackForm = false;
			await load();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่สำเร็จ');
		} finally {
			savingTrack = false;
		}
	}
	async function removeTrack(t: AdmissionTrack) {
		if (!confirm(`ลบสาย "${t.name}"?`)) return;
		try {
			await deleteTrack(t.id);
			toast.success('ลบสายแล้ว');
			await load();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ลบไม่สำเร็จ');
		}
	}

	// ===== Subject =====
	function openNewSubject() {
		editingSubject = null;
		subjectForm = { name: '', code: '', maxScore: '100', displayOrder: subjects.length.toString() };
		showSubjectForm = true;
	}
	function openEditSubject(s: AdmissionExamSubject) {
		editingSubject = s;
		subjectForm = {
			name: s.name,
			code: s.code ?? '',
			maxScore: s.maxScore.toString(),
			displayOrder: s.displayOrder.toString()
		};
		showSubjectForm = true;
	}
	async function saveSubject() {
		if (!id) return;
		savingSubject = true;
		try {
			if (editingSubject) {
				await updateSubject(editingSubject.id, {
					name: subjectForm.name,
					code: subjectForm.code || undefined,
					maxScore: parseFloat(subjectForm.maxScore),
					displayOrder: parseInt(subjectForm.displayOrder)
				});
				toast.success('อัปเดตวิชาแล้ว');
			} else {
				await createSubject(id, {
					name: subjectForm.name,
					code: subjectForm.code || undefined,
					maxScore: parseFloat(subjectForm.maxScore),
					displayOrder: parseInt(subjectForm.displayOrder)
				});
				toast.success('เพิ่มวิชาแล้ว');
			}
			showSubjectForm = false;
			await load();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่สำเร็จ');
		} finally {
			savingSubject = false;
		}
	}
	async function removeSubject(s: AdmissionExamSubject) {
		if (!confirm(`ลบวิชา "${s.name}"?`)) return;
		try {
			await deleteSubject(s.id);
			toast.success('ลบวิชาแล้ว');
			await load();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ลบไม่สำเร็จ');
		}
	}

	function formatDate(d?: string) {
		if (!d) return '-';
		return new Date(d).toLocaleDateString('th-TH', {
			year: 'numeric',
			month: 'short',
			day: 'numeric'
		});
	}

	onMount(load);
</script>

<svelte:head>
	<title>{round?.name ?? 'จัดการรอบรับสมัคร'} - SchoolOrbit</title>
</svelte:head>

{#if loading}
	<div class="flex justify-center items-center py-20">
		<div
			class="w-8 h-8 border-4 border-primary border-t-transparent rounded-full animate-spin"
		></div>
	</div>
{:else if round}
	<div class="space-y-6">
		<!-- Header -->
		<div class="flex items-center gap-3">
			<Button href="/staff/academic/admission" variant="ghost" size="sm">
				<ArrowLeft class="w-4 h-4 mr-1" /> ย้อนกลับ
			</Button>
		</div>

		<!-- Round Info Card -->
		<div class="bg-card border border-border rounded-lg p-5">
			<div class="flex flex-col md:flex-row md:items-start justify-between gap-4">
				<div>
					<div class="flex items-center gap-2 flex-wrap">
						<h1 class="text-2xl font-bold">{round.name}</h1>
						<span
							class="text-xs px-2 py-0.5 rounded-full font-medium {roundStatusColor[round.status]}"
						>
							{roundStatusLabel[round.status]}
						</span>
					</div>
					<div class="mt-2 text-sm text-muted-foreground space-y-0.5">
						<p>ปีการศึกษา: {round.academicYearName} | ชั้น: {round.gradeLevelName}</p>
						<p>รับสมัคร: {formatDate(round.applyStartDate)} – {formatDate(round.applyEndDate)}</p>
						{#if round.examDate}<p>วันสอบ: {formatDate(round.examDate)}</p>{/if}
						{#if round.resultAnnounceDate}<p>
								ประกาศผล: {formatDate(round.resultAnnounceDate)}
							</p>{/if}
					</div>
				</div>
				<!-- Status Stepper -->
				<div class="flex flex-col gap-1">
					<p class="text-xs text-muted-foreground font-medium mb-1">เปลี่ยนสถานะ:</p>
					<div class="flex flex-wrap gap-1">
						{#each statusFlow as s}
							<button
								onclick={() => handleStatusChange(s)}
								class="text-xs px-2 py-1 rounded border transition-colors {round.status === s
									? 'border-primary bg-primary text-primary-foreground'
									: 'border-border hover:bg-accent'}"
							>
								{roundStatusLabel[s]}
							</button>
						{/each}
					</div>
				</div>
			</div>

			<!-- Quick Links -->
			<div class="flex flex-wrap gap-2 mt-4 pt-4 border-t border-border">
				<Button
					href="/staff/academic/admission/{id}/applications"
					variant="outline"
					size="sm"
					class="gap-1.5"
				>
					<Users class="w-3.5 h-3.5" /> ใบสมัคร ({round.applicationCount ?? 0})
				</Button>
				<Button
					href="/staff/academic/admission/{id}/scores"
					variant="outline"
					size="sm"
					class="gap-1.5"
				>
					<ClipboardList class="w-3.5 h-3.5" /> กรอกคะแนน
				</Button>
				<Button
					href="/staff/academic/admission/{id}/selections"
					variant="outline"
					size="sm"
					class="gap-1.5"
				>
					<GraduationCap class="w-3.5 h-3.5" /> จัดห้อง
				</Button>
				<Button
					href="/staff/academic/admission/{id}/enrollment"
					variant="outline"
					size="sm"
					class="gap-1.5"
				>
					<Check class="w-3.5 h-3.5" /> รับมอบตัว
				</Button>
			</div>
		</div>

		<div class="grid md:grid-cols-2 gap-6">
			<!-- Section: Tracks -->
			<div class="bg-card border border-border rounded-lg">
				<div class="flex items-center justify-between px-5 py-4 border-b border-border">
					<h2 class="font-semibold flex items-center gap-2">
						<BookOpen class="w-4 h-4" /> สายการเรียน ({tracks.length})
					</h2>
					<Button size="sm" onclick={openNewTrack} class="gap-1">
						<Plus class="w-3.5 h-3.5" /> เพิ่ม
					</Button>
				</div>

				{#if showTrackForm}
					<div class="p-4 border-b border-border bg-muted/30 space-y-3">
						{#if !editingTrack}
							<div>
								<label class="text-xs font-medium">แผนการเรียน *</label>
								<select
									bind:value={trackForm.studyPlanId}
									class="mt-1 w-full px-3 py-1.5 text-sm rounded border border-border bg-background"
								>
									<option value="">-- เลือก --</option>
									{#each studyPlans as sp (sp.id)}
										<option value={sp.id}>{sp.nameTh}</option>
									{/each}
								</select>
							</div>
						{/if}
						<div>
							<label class="text-xs font-medium">ชื่อสาย *</label>
							<Input
								bind:value={trackForm.name}
								placeholder="เช่น สายวิทย์-คณิต"
								class="mt-1 h-8 text-sm"
							/>
						</div>
						<div class="grid grid-cols-2 gap-2">
							<div>
								<label class="text-xs font-medium">จำนวนรับ (override)</label>
								<Input
									bind:value={trackForm.capacityOverride}
									type="number"
									placeholder="อัตโนมัติ"
									class="mt-1 h-8 text-sm"
								/>
							</div>
							<div>
								<label class="text-xs font-medium">Tie-breaking</label>
								<select
									bind:value={trackForm.tiebreakMethod}
									class="mt-1 w-full px-2 py-1.5 text-sm rounded border border-border bg-background"
								>
									<option value="applied_at">สมัครก่อนได้ก่อน</option>
									<option value="gpa">GPA สูงกว่าได้ก่อน</option>
								</select>
							</div>
						</div>
						<div class="flex gap-2">
							<Button size="sm" onclick={saveTrack} disabled={savingTrack} class="h-7 text-xs">
								{savingTrack ? 'บันทึก...' : 'บันทึก'}
							</Button>
							<Button
								size="sm"
								variant="ghost"
								onclick={() => (showTrackForm = false)}
								class="h-7 text-xs">ยกเลิก</Button
							>
						</div>
					</div>
				{/if}

				<div class="divide-y divide-border">
					{#each tracks as t (t.id)}
						<div class="px-5 py-3 flex items-center justify-between">
							<div>
								<p class="font-medium text-sm">{t.name}</p>
								<p class="text-xs text-muted-foreground">
									{t.studyPlanName ?? '-'} • รับ {t.computedCapacity ?? '-'} คน ({t.roomCount ?? 0} ห้อง)
									• สมัครแล้ว {t.applicationCount ?? 0}
								</p>
							</div>
							<div class="flex gap-1">
								<Button variant="ghost" size="sm" onclick={() => openEditTrack(t)}>
									<Pencil class="w-3.5 h-3.5" />
								</Button>
								<Button
									variant="ghost"
									size="sm"
									onclick={() => removeTrack(t)}
									class="text-destructive"
								>
									<Trash2 class="w-3.5 h-3.5" />
								</Button>
							</div>
						</div>
					{:else}
						<p class="px-5 py-6 text-center text-sm text-muted-foreground">ยังไม่มีสายการเรียน</p>
					{/each}
				</div>
			</div>

			<!-- Section: Exam Subjects -->
			<div class="bg-card border border-border rounded-lg">
				<div class="flex items-center justify-between px-5 py-4 border-b border-border">
					<h2 class="font-semibold flex items-center gap-2">
						<Settings class="w-4 h-4" /> วิชาที่สอบ ({subjects.length})
					</h2>
					<Button size="sm" onclick={openNewSubject} class="gap-1">
						<Plus class="w-3.5 h-3.5" /> เพิ่ม
					</Button>
				</div>

				{#if showSubjectForm}
					<div class="p-4 border-b border-border bg-muted/30 space-y-3">
						<div class="grid grid-cols-2 gap-2">
							<div>
								<label class="text-xs font-medium">ชื่อวิชา *</label>
								<Input
									bind:value={subjectForm.name}
									placeholder="วิชาคณิตศาสตร์"
									class="mt-1 h-8 text-sm"
								/>
							</div>
							<div>
								<label class="text-xs font-medium">รหัส</label>
								<Input bind:value={subjectForm.code} placeholder="MATH" class="mt-1 h-8 text-sm" />
							</div>
						</div>
						<div class="grid grid-cols-2 gap-2">
							<div>
								<label class="text-xs font-medium">คะแนนเต็ม</label>
								<Input bind:value={subjectForm.maxScore} type="number" class="mt-1 h-8 text-sm" />
							</div>
							<div>
								<label class="text-xs font-medium">ลำดับ</label>
								<Input
									bind:value={subjectForm.displayOrder}
									type="number"
									class="mt-1 h-8 text-sm"
								/>
							</div>
						</div>
						<div class="flex gap-2">
							<Button size="sm" onclick={saveSubject} disabled={savingSubject} class="h-7 text-xs">
								{savingSubject ? 'บันทึก...' : 'บันทึก'}
							</Button>
							<Button
								size="sm"
								variant="ghost"
								onclick={() => (showSubjectForm = false)}
								class="h-7 text-xs">ยกเลิก</Button
							>
						</div>
					</div>
				{/if}

				<div class="divide-y divide-border">
					{#each subjects as s (s.id)}
						<div class="px-5 py-3 flex items-center justify-between">
							<div>
								<p class="font-medium text-sm">{s.name}</p>
								<p class="text-xs text-muted-foreground">
									{s.code ? `[${s.code}]` : ''} คะแนนเต็ม {s.maxScore}
								</p>
							</div>
							<div class="flex gap-1">
								<Button variant="ghost" size="sm" onclick={() => openEditSubject(s)}
									><Pencil class="w-3.5 h-3.5" /></Button
								>
								<Button
									variant="ghost"
									size="sm"
									onclick={() => removeSubject(s)}
									class="text-destructive"><Trash2 class="w-3.5 h-3.5" /></Button
								>
							</div>
						</div>
					{:else}
						<p class="px-5 py-6 text-center text-sm text-muted-foreground">ยังไม่มีวิชา</p>
					{/each}
				</div>
			</div>
		</div>
	</div>
{/if}
