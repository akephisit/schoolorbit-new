<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import type { PageProps } from './$types';
	import {
		getRound,
		listTracks,
		listSubjects,
		listRounds,
		createTrack,
		updateTrack,
		deleteTrack,
		createSubject,
		updateSubject,
		deleteSubject,
		updateRoundStatus,
		updateRoundVisibility,
		updateRound,
		deleteRound,
		updateSelectionSettings,
		type AdmissionRound,
		type AdmissionTrack,
		type AdmissionExamSubject,
		type ReportConfig,
		roundStatusLabel,
		roundStatusColor
	} from '$lib/api/admission';
	import { listStudyPlans } from '$lib/api/academic';
	import SchoolCombobox from '$lib/components/ui/SchoolCombobox.svelte';

	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { Separator } from '$lib/components/ui/separator';
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
		ClipboardList,
		Users,
		Loader2,
		Copy,
		Link as LinkIcon,
		BarChart2,
		X,
		DoorOpen,
		Hash
	} from 'lucide-svelte';

	let { data, params }: PageProps = $props();

	let id = $derived(params.id);
	let round: AdmissionRound | null = $state(null);
	let tracks: AdmissionTrack[] = $state([]);
	let subjects: AdmissionExamSubject[] = $state([]);
	let studyPlans: { id: string; nameTh: string }[] = $state([]);
	let loading = $state(true);

	let showDeleteDialog = $state(false);
	let deletingRound = $state(false);

	let deletingTrackTarget: AdmissionTrack | null = $state(null);
	let deletingTrack = $state(false);

	let deletingSubjectTarget: AdmissionExamSubject | null = $state(null);
	let deletingSubject = $state(false);

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

	// Report config
	let reportConfig: ReportConfig = $state({ reportMode: null, zone: { schools: [] }, institution: { ownSchool: '' } });
	let savingReportConfig = $state(false);
	let allRounds: AdmissionRound[] = $state([]);

	const reportModeLabel: Record<string, string> = {
		zone: 'เขตพื้นที่',
		institution: 'สถานศึกษาเดิม',
		both: 'ทั้งสองประเภท'
	};

	let copyableRounds = $derived(
		allRounds.filter(r => r.id !== id && r.reportConfig?.reportMode != null)
	);

	const statusFlow: AdmissionRound['status'][] = [
		'draft',
		'open',
		'exam_announced',
		'announced',
		'enrolling',
		'closed'
	];

	function copyLink(path: string) {
		const url = `${page.url.origin}${path}`;
		navigator.clipboard.writeText(url);
		toast.success('คัดลอกลิงก์เรียบร้อยแล้ว');
	}

	const statusVariant: Record<string, 'default' | 'secondary' | 'outline' | 'destructive'> = {
		draft: 'secondary',
		open: 'default',
		exam_announced: 'default',
		announced: 'default',
		enrolling: 'default',
		closed: 'destructive'
	};

	let togglingVisibility = $state(false);
	let togglingShowScores = $state(false);
	let pendingStatus = $state<AdmissionRound['status'] | null>(null);

	async function handleVisibilityToggle() {
		if (!round) return;
		togglingVisibility = true;
		const newVal = !round.isVisible;
		try {
			await updateRoundVisibility(round.id, newVal);
			toast.success(newVal ? 'แสดงรอบบน portal แล้ว' : 'ซ่อนรอบจาก portal แล้ว');
			round = { ...round, isVisible: newVal };
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'อัปเดตไม่สำเร็จ');
		} finally {
			togglingVisibility = false;
		}
	}

	async function handleShowScoresToggle() {
		if (!round) return;
		togglingShowScores = true;
		const current = round.selectionSettings?.showScores ?? false;
		try {
			await updateSelectionSettings(round.id, { showScores: !current });
			toast.success(!current ? 'เปิดแสดงคะแนนบน portal แล้ว' : 'ซ่อนคะแนนบน portal แล้ว');
			round = { ...round, selectionSettings: { ...(round.selectionSettings ?? {}), showScores: !current } };
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'อัปเดตไม่สำเร็จ');
		} finally {
			togglingShowScores = false;
		}
	}

	async function load() {
		if (!id) return;
		loading = true;
		try {
			const [r, t, s, allR] = await Promise.all([getRound(id), listTracks(id), listSubjects(id), listRounds()]);
			round = r;
			tracks = t;
			subjects = s;
			allRounds = allR;
			reportConfig = {
				reportMode: r.reportConfig?.reportMode ?? null,
				zone: { schools: r.reportConfig?.zone?.schools ?? [] },
				institution: { ownSchool: r.reportConfig?.institution?.ownSchool ?? '' }
			};
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
		const sp = await listStudyPlans({ active_only: true });
		studyPlans = (sp.data ?? []).map((p) => ({ id: p.id, nameTh: p.name_th }));
	}

	async function confirmStatusChange() {
		if (!round || !pendingStatus) return;
		const status = pendingStatus;
		pendingStatus = null;
		try {
			await updateRoundStatus(round.id, status);
			toast.success(`สถานะ → ${roundStatusLabel[status]}`);
			round = { ...round, status };
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'เปลี่ยนสถานะไม่สำเร็จ');
		}
	}

	async function confirmDeleteRound() {
		if (!round) return;
		deletingRound = true;
		try {
			await deleteRound(round.id);
			toast.success('ลบรอบรับสมัครแล้ว');
			goto('/staff/academic/admission');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ลบไม่สำเร็จ');
		} finally {
			deletingRound = false;
			showDeleteDialog = false;
		}
	}

	// Track CRUD
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
	function removeTrack(t: AdmissionTrack) {
		deletingTrackTarget = t;
	}
	async function confirmDeleteTrack() {
		if (!deletingTrackTarget) return;
		deletingTrack = true;
		try {
			await deleteTrack(deletingTrackTarget.id);
			toast.success('ลบสายแล้ว');
			await load();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ลบไม่สำเร็จ');
		} finally {
			deletingTrack = false;
			deletingTrackTarget = null;
		}
	}

	// Subject CRUD
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
	function removeSubject(s: AdmissionExamSubject) {
		deletingSubjectTarget = s;
	}
	async function confirmDeleteSubject() {
		if (!deletingSubjectTarget) return;
		deletingSubject = true;
		try {
			await deleteSubject(deletingSubjectTarget.id);
			toast.success('ลบวิชาแล้ว');
			await load();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ลบไม่สำเร็จ');
		} finally {
			deletingSubject = false;
			deletingSubjectTarget = null;
		}
	}

	async function saveReportConfig() {
		if (!id) return;
		savingReportConfig = true;
		try {
			const payload: ReportConfig = { reportMode: reportConfig.reportMode };
			if (reportConfig.reportMode === 'zone' || reportConfig.reportMode === 'both') {
				payload.zone = { schools: reportConfig.zone?.schools ?? [] };
			}
			if (reportConfig.reportMode === 'institution' || reportConfig.reportMode === 'both') {
				payload.institution = { ownSchool: reportConfig.institution?.ownSchool ?? '' };
			}
			await updateRound(id, { reportConfig: payload });
			toast.success('บันทึกการตั้งค่ารายงานแล้ว');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่สำเร็จ');
		} finally {
			savingReportConfig = false;
		}
	}

	function addZoneSchool(name: string) {
		if (!name) return;
		const schools = reportConfig.zone?.schools ?? [];
		if (!schools.includes(name)) {
			reportConfig = { ...reportConfig, zone: { schools: [...schools, name] } };
		}
	}

	function copyConfigFromRound(source: AdmissionRound) {
		const cfg = source.reportConfig!;
		reportConfig = {
			reportMode: cfg.reportMode,
			zone: { schools: cfg.zone?.schools ? [...cfg.zone.schools] : [] },
			institution: { ownSchool: cfg.institution?.ownSchool ?? '' }
		};
		toast.info(`คัดลอก config จาก "${source.name}" แล้ว`);
	}

	function removeZoneSchool(school: string) {
		reportConfig = {
			...reportConfig,
			zone: { schools: (reportConfig.zone?.schools ?? []).filter((s) => s !== school) }
		};
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
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

{#if loading}
	<div class="flex justify-center items-center py-20">
		<Loader2 class="w-8 h-8 animate-spin text-primary" />
	</div>
{:else if round}
	<div class="space-y-6">
		<!-- Header Actions -->
		<div class="flex items-center justify-between">
			<Button href="/staff/academic/admission" variant="ghost" size="sm" class="gap-1">
				<ArrowLeft class="w-4 h-4" /> ย้อนกลับ
			</Button>

			<Button
				variant="destructive"
				size="sm"
				class="gap-1"
				onclick={() => {
					showDeleteDialog = true;
				}}
			>
				<Trash2 class="w-4 h-4" /> ลบรอบ
			</Button>
		</div>

		<!-- Round Info Card -->
		<Card.Root>
			<Card.Content class="p-5">
				<div class="flex flex-col md:flex-row md:items-start justify-between gap-4">
					<div class="space-y-1">
						<div class="flex items-center gap-2 flex-wrap">
							<h1 class="text-2xl font-bold">{round.name}</h1>
							<Badge variant={statusVariant[round.status] ?? 'secondary'}>
								{roundStatusLabel[round.status]}
							</Badge>
						</div>
						<div class="text-sm text-muted-foreground space-y-0.5">
							<p>ปีการศึกษา: {round.academicYearName} | ชั้น: {round.gradeLevelName}</p>
							<p>รับสมัคร: {formatDate(round.applyStartDate)} – {formatDate(round.applyEndDate)}</p>
							{#if round.examDate}<p>วันสอบ: {formatDate(round.examDate)}</p>{/if}
							{#if round.resultAnnounceDate}<p>
									ประกาศผล: {formatDate(round.resultAnnounceDate)}
								</p>{/if}
						</div>
					</div>
					<!-- Status Stepper -->
					<div class="flex flex-col gap-1.5">
						<p class="text-xs text-muted-foreground font-medium">เปลี่ยนสถานะ:</p>
						<div class="flex flex-wrap gap-1">
							{#each statusFlow as s}
								<button
									onclick={() => { if (round?.status !== s) pendingStatus = s; }}
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

				<Separator class="my-4" />

				<!-- Visibility Toggle -->
				<div class="flex items-center justify-between py-1">
					<div>
						<p class="text-sm font-medium">แสดงรอบบน portal นักเรียน</p>
						<p class="text-xs text-muted-foreground">
							{round.isVisible
								? 'รอบนี้ปรากฏบน portal — นักเรียนเห็นและสามารถตรวจสอบสถานะได้'
								: 'ซ่อนอยู่ — นักเรียนไม่เห็นรอบนี้บน portal'}
						</p>
					</div>
					<Button
						variant={round.isVisible ? 'default' : 'outline'}
						size="sm"
						disabled={togglingVisibility}
						onclick={handleVisibilityToggle}
						class="shrink-0 gap-1.5"
					>
						{#if togglingVisibility}
							<Loader2 class="w-3 h-3 animate-spin" />
						{:else if round.isVisible}
							<Check class="w-3 h-3" />
						{/if}
						{round.isVisible ? 'แสดงอยู่' : 'ซ่อนอยู่'}
					</Button>
				</div>

				<!-- Show Scores Toggle -->
				{@const showScores = round.selectionSettings?.showScores ?? false}
				<div class="flex items-center justify-between py-1">
					<div>
						<p class="text-sm font-medium">แสดงคะแนนบน portal นักเรียน</p>
						<p class="text-xs text-muted-foreground">
							{showScores
								? 'นักเรียนเห็นคะแนนสอบของตัวเองบน portal'
								: 'ซ่อนคะแนน — นักเรียนไม่เห็นคะแนน'}
						</p>
					</div>
					<Button
						variant={showScores ? 'default' : 'outline'}
						size="sm"
						disabled={togglingShowScores}
						onclick={handleShowScoresToggle}
						class="shrink-0 gap-1.5"
					>
						{#if togglingShowScores}
							<Loader2 class="w-3 h-3 animate-spin" />
						{:else if showScores}
							<Check class="w-3 h-3" />
						{/if}
						{showScores ? 'แสดงอยู่' : 'ซ่อนอยู่'}
					</Button>
				</div>

				<Separator class="my-4" />

				<!-- Share Links Section -->
				<div class="space-y-3">
					<h3 class="text-sm font-semibold flex items-center gap-1.5">
						<LinkIcon class="w-4 h-4 text-primary" /> ลิงก์สำหรับนักเรียน
					</h3>
					<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
						<div class="space-y-1.5">
							<Label class="text-xs text-muted-foreground"
								>ลิงก์กรอกใบสมัคร (เปิดเมื่อสถานะ "เปิดรับสมัคร")</Label
							>
							<div class="flex items-center gap-2">
								<Input
									value={`${page.url.origin}/apply/${id}`}
									readonly
									class="h-9 bg-muted/50 font-mono text-xs"
								/>
								<Button
									variant="secondary"
									size="icon"
									class="h-9 w-9 shrink-0"
									onclick={() => copyLink(`/apply/${id}`)}
								>
									<Copy class="w-4 h-4" />
								</Button>
							</div>
						</div>
						<div class="space-y-1.5">
							<Label class="text-xs text-muted-foreground">ลิงก์รวมรอบรับสมัคร / ระบบมอบตัว</Label>
							<div class="flex items-center gap-2">
								<Input
									value={`${page.url.origin}/apply`}
									readonly
									class="h-9 bg-muted/50 font-mono text-xs"
								/>
								<Button
									variant="secondary"
									size="icon"
									class="h-9 w-9 shrink-0"
									onclick={() => copyLink(`/apply`)}
								>
									<Copy class="w-4 h-4" />
								</Button>
							</div>
						</div>
					</div>
				</div>

				<Separator class="my-4" />

				<!-- Quick Links -->
				<div class="flex flex-wrap gap-2">
					<Button
						href="/staff/academic/admission/{id}/applications"
						variant="outline"
						size="sm"
						class="gap-1.5"
					>
						<Users class="w-3.5 h-3.5" /> ใบสมัคร ({round.applicationCount ?? 0})
					</Button>
					<Button
						href="/staff/academic/admission/{id}/exam-rooms"
						variant="outline"
						size="sm"
						class="gap-1.5"
					>
						<DoorOpen class="w-3.5 h-3.5" /> ห้องสอบ
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
						href="/staff/academic/admission/{id}/student-ids"
						variant="outline"
						size="sm"
						class="gap-1.5"
					>
						<Hash class="w-3.5 h-3.5" /> กำหนดเลขประจำตัว
					</Button>
					<Button
						href="/staff/academic/admission/{id}/enrollment"
						variant="outline"
						size="sm"
						class="gap-1.5"
					>
						<Check class="w-3.5 h-3.5" /> รับมอบตัว
					</Button>
					{#if reportConfig.reportMode !== null}
						<Button
							href="/staff/academic/admission/{id}/report"
							variant="outline"
							size="sm"
							class="gap-1.5"
						>
							<BarChart2 class="w-3.5 h-3.5" /> ดูรายงาน
						</Button>
					{/if}
				</div>
			</Card.Content>
		</Card.Root>

		<div class="grid md:grid-cols-2 gap-6">
			<!-- Tracks -->
			<Card.Root>
				<Card.Header class="flex flex-row items-center justify-between pb-3">
					<Card.Title class="flex items-center gap-2 text-base">
						<BookOpen class="w-4 h-4" /> สายการเรียน ({tracks.length})
					</Card.Title>
					<Button size="sm" onclick={openNewTrack} class="gap-1 h-8">
						<Plus class="w-3.5 h-3.5" /> เพิ่ม
					</Button>
				</Card.Header>

				{#if showTrackForm}
					<div class="px-4 pb-4 space-y-3 border-b border-border">
						{#if !editingTrack}
							<div class="space-y-1.5">
								<Label for="track-plan">แผนการเรียน <span class="text-destructive">*</span></Label>
								<Select.Root type="single" bind:value={trackForm.studyPlanId}>
									<Select.Trigger id="track-plan" class="w-full">
										{studyPlans.find((s) => s.id === trackForm.studyPlanId)?.nameTh ??
											'-- เลือก --'}
									</Select.Trigger>
									<Select.Content>
										{#each studyPlans as sp (sp.id)}
											<Select.Item value={sp.id}>{sp.nameTh}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
							</div>
						{/if}
						<div class="space-y-1.5">
							<Label for="track-name">ชื่อสาย <span class="text-destructive">*</span></Label>
							<Input
								id="track-name"
								bind:value={trackForm.name}
								placeholder="เช่น สายวิทย์-คณิต"
								class="h-8 text-sm"
							/>
						</div>
						<div class="grid grid-cols-2 gap-2">
							<div class="space-y-1.5">
								<Label for="track-cap">จำนวนรับ (override)</Label>
								<Input
									id="track-cap"
									bind:value={trackForm.capacityOverride}
									type="number"
									placeholder="อัตโนมัติ"
									class="h-8 text-sm"
								/>
							</div>
							<div class="space-y-1.5">
								<Label for="track-tie">Tie-breaking</Label>
								<Select.Root type="single" bind:value={trackForm.tiebreakMethod}>
									<Select.Trigger id="track-tie" class="w-full h-8 text-sm">
										{trackForm.tiebreakMethod === 'gpa' ? 'GPA สูงกว่าได้ก่อน' : 'สมัครก่อนได้ก่อน'}
									</Select.Trigger>
									<Select.Content>
										<Select.Item value="applied_at">สมัครก่อนได้ก่อน</Select.Item>
										<Select.Item value="gpa">GPA สูงกว่าได้ก่อน</Select.Item>
									</Select.Content>
								</Select.Root>
							</div>
						</div>
						<div class="flex gap-2">
							<Button size="sm" onclick={saveTrack} disabled={savingTrack} class="h-7 text-xs">
								{#if savingTrack}<Loader2 class="w-3 h-3 mr-1 animate-spin" />{/if}
								บันทึก
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

				<Card.Content class="p-0">
					<div class="divide-y divide-border">
						{#each tracks as t (t.id)}
							<div class="px-4 py-3 flex items-center justify-between">
								<div>
									<p class="font-medium text-sm">{t.name}</p>
									<p class="text-xs text-muted-foreground">
										{t.studyPlanName ?? '-'} • รับ {t.computedCapacity ?? '-'} คน ({t.roomCount ??
											0} ห้อง) • สมัคร {t.applicationCount ?? 0}
									</p>
								</div>
								<div class="flex gap-1">
									<Button
										variant="ghost"
										size="icon"
										class="h-7 w-7"
										onclick={() => openEditTrack(t)}
									>
										<Pencil class="w-3.5 h-3.5" />
									</Button>
									<Button
										variant="ghost"
										size="icon"
										class="h-7 w-7 text-destructive hover:text-destructive"
										onclick={() => removeTrack(t)}
									>
										<Trash2 class="w-3.5 h-3.5" />
									</Button>
								</div>
							</div>
						{:else}
							<p class="px-4 py-8 text-center text-sm text-muted-foreground">ยังไม่มีสายการเรียน</p>
						{/each}
					</div>
				</Card.Content>
			</Card.Root>

			<!-- Exam Subjects -->
			<Card.Root>
				<Card.Header class="flex flex-row items-center justify-between pb-3">
					<Card.Title class="flex items-center gap-2 text-base">
						<Settings class="w-4 h-4" /> วิชาที่สอบ ({subjects.length})
					</Card.Title>
					<Button size="sm" onclick={openNewSubject} class="gap-1 h-8">
						<Plus class="w-3.5 h-3.5" /> เพิ่ม
					</Button>
				</Card.Header>

				{#if showSubjectForm}
					<div class="px-4 pb-4 space-y-3 border-b border-border">
						<div class="grid grid-cols-2 gap-2">
							<div class="space-y-1.5">
								<Label for="sub-name">ชื่อวิชา <span class="text-destructive">*</span></Label>
								<Input
									id="sub-name"
									bind:value={subjectForm.name}
									placeholder="วิชาคณิตศาสตร์"
									class="h-8 text-sm"
								/>
							</div>
							<div class="space-y-1.5">
								<Label for="sub-code">รหัส</Label>
								<Input
									id="sub-code"
									bind:value={subjectForm.code}
									placeholder="MATH"
									class="h-8 text-sm"
								/>
							</div>
						</div>
						<div class="grid grid-cols-2 gap-2">
							<div class="space-y-1.5">
								<Label for="sub-max">คะแนนเต็ม</Label>
								<Input
									id="sub-max"
									bind:value={subjectForm.maxScore}
									type="number"
									class="h-8 text-sm"
								/>
							</div>
							<div class="space-y-1.5">
								<Label for="sub-order">ลำดับ</Label>
								<Input
									id="sub-order"
									bind:value={subjectForm.displayOrder}
									type="number"
									class="h-8 text-sm"
								/>
							</div>
						</div>
						<div class="flex gap-2">
							<Button size="sm" onclick={saveSubject} disabled={savingSubject} class="h-7 text-xs">
								{#if savingSubject}<Loader2 class="w-3 h-3 mr-1 animate-spin" />{/if}
								บันทึก
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

				<Card.Content class="p-0">
					<div class="divide-y divide-border">
						{#each subjects as s (s.id)}
							<div class="px-4 py-3 flex items-center justify-between">
								<div>
									<p class="font-medium text-sm">{s.name}</p>
									<p class="text-xs text-muted-foreground">
										{s.code ? `[${s.code}]` : ''} คะแนนเต็ม {s.maxScore}
									</p>
								</div>
								<div class="flex gap-1">
									<Button
										variant="ghost"
										size="icon"
										class="h-7 w-7"
										onclick={() => openEditSubject(s)}
									>
										<Pencil class="w-3.5 h-3.5" />
									</Button>
									<Button
										variant="ghost"
										size="icon"
										class="h-7 w-7 text-destructive hover:text-destructive"
										onclick={() => removeSubject(s)}
									>
										<Trash2 class="w-3.5 h-3.5" />
									</Button>
								</div>
							</div>
						{:else}
							<p class="px-4 py-8 text-center text-sm text-muted-foreground">ยังไม่มีวิชา</p>
						{/each}
					</div>
				</Card.Content>
			</Card.Root>
		</div>

	<!-- Report Config Card -->
	<Card.Root>
		<Card.Header class="pb-3">
			<div class="flex items-start justify-between gap-2">
				<div>
					<Card.Title class="flex items-center gap-2 text-base">
						<BarChart2 class="w-4 h-4" /> การรายงาน
					</Card.Title>
					<Card.Description>ตั้งค่าการแบ่งกลุ่มผู้สมัครสำหรับรายงานสถิติ</Card.Description>
				</div>
				{#if copyableRounds.length > 0}
					<Select.Root
						type="single"
						onValueChange={(roundId) => {
							const src = copyableRounds.find(r => r.id === roundId);
							if (src) copyConfigFromRound(src);
						}}
					>
						<Select.Trigger class="h-8 text-xs gap-1.5 w-auto shrink-0">
							<Copy class="w-3.5 h-3.5" /> คัดลอกจากรอบอื่น
						</Select.Trigger>
						<Select.Content>
							{#each copyableRounds as r (r.id)}
								<Select.Item value={r.id}>
									<span>{r.name}</span>
									<span class="ml-2 text-xs text-muted-foreground">
										({reportModeLabel[r.reportConfig!.reportMode!]}{#if r.reportConfig?.zone?.schools?.length} · {r.reportConfig.zone.schools.length} โรงเรียน{/if})
									</span>
								</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				{/if}
			</div>
		</Card.Header>
		<Card.Content class="space-y-4">
			<!-- Mode selector -->
			<div class="space-y-1.5">
				<Label>ประเภทรายงาน</Label>
				<Select.Root
					type="single"
					value={reportConfig.reportMode ?? 'none'}
					onValueChange={(v) => {
						const mode = v === 'none' ? null : (v as ReportConfig['reportMode']);
					reportConfig = {
						...reportConfig,
						reportMode: mode,
						zone: reportConfig.zone ?? { schools: [] },
						institution: reportConfig.institution ?? { ownSchool: '' }
					};
					}}
				>
					<Select.Trigger class="w-full max-w-xs">
						{#if reportConfig.reportMode === null}ปิด (ไม่ใช้)
						{:else if reportConfig.reportMode === 'zone'}เขตพื้นที่บริการ
						{:else if reportConfig.reportMode === 'institution'}สถานศึกษาเดิม
						{:else}ทั้งสองประเภท{/if}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="none">ปิด (ไม่ใช้)</Select.Item>
						<Select.Item value="zone">เขตพื้นที่บริการ</Select.Item>
						<Select.Item value="institution">สถานศึกษาเดิม</Select.Item>
						<Select.Item value="both">ทั้งสองประเภท</Select.Item>
					</Select.Content>
				</Select.Root>
			</div>

			{#if reportConfig.reportMode === 'zone' || reportConfig.reportMode === 'both'}
				<!-- Zone schools -->
				<div class="space-y-2">
					<Label>โรงเรียนในเขตพื้นที่บริการ</Label>
					<div class="max-w-sm">
						<SchoolCombobox
							value=""
							onProvinceSelect={() => {}}
							onSelect={(name) => addZoneSchool(name)}
						/>
					</div>
					{#if (reportConfig.zone?.schools ?? []).length > 0}
						<div class="flex flex-wrap gap-1.5 mt-1">
							{#each reportConfig.zone?.schools ?? [] as school (school)}
								<span class="inline-flex items-center gap-1 text-xs bg-primary/10 text-primary px-2 py-0.5 rounded-full">
									{school}
									<button onclick={() => removeZoneSchool(school)} class="hover:text-destructive">
										<X class="w-3 h-3" />
									</button>
								</span>
							{/each}
						</div>
					{/if}
				</div>
			{/if}

			{#if reportConfig.reportMode === 'institution' || reportConfig.reportMode === 'both'}
				<!-- Own school -->
				<div class="space-y-1.5">
					<Label>ชื่อโรงเรียนตนเอง</Label>
					<div class="max-w-sm">
						<SchoolCombobox
							bind:value={reportConfig.institution!.ownSchool}
							onProvinceSelect={() => {}}
						/>
					</div>
				</div>
			{/if}

			<Button onclick={saveReportConfig} disabled={savingReportConfig} size="sm" class="gap-1.5">
				{#if savingReportConfig}<Loader2 class="w-3.5 h-3.5 animate-spin" />{/if}
				บันทึกการตั้งค่า
			</Button>
		</Card.Content>
	</Card.Root>
	</div>

	<!-- Delete Track Dialog -->
	<Dialog.Root open={deletingTrackTarget !== null} onOpenChange={(o) => { if (!o) deletingTrackTarget = null; }}>
		<Dialog.Content>
			<Dialog.Header>
				<Dialog.Title>ยืนยันการลบสายการเรียน</Dialog.Title>
				<Dialog.Description>
					ลบสาย <strong>{deletingTrackTarget?.name}</strong>? ข้อมูลใบสมัครและคะแนนที่เกี่ยวข้องกับสายนี้อาจถูกลบด้วย
				</Dialog.Description>
			</Dialog.Header>
			<Dialog.Footer>
				<Button
					variant="outline"
					onclick={() => (deletingTrackTarget = null)}
					disabled={deletingTrack}
				>
					ยกเลิก
				</Button>
				<Button variant="destructive" onclick={confirmDeleteTrack} disabled={deletingTrack}>
					{#if deletingTrack}<Loader2 class="w-4 h-4 mr-2 animate-spin" />{/if}
					{deletingTrack ? 'กำลังลบ...' : 'ลบสาย'}
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>

	<!-- Delete Subject Dialog -->
	<Dialog.Root open={deletingSubjectTarget !== null} onOpenChange={(o) => { if (!o) deletingSubjectTarget = null; }}>
		<Dialog.Content>
			<Dialog.Header>
				<Dialog.Title>ยืนยันการลบวิชา</Dialog.Title>
				<Dialog.Description>
					ลบวิชา <strong>{deletingSubjectTarget?.name}</strong>? คะแนนของผู้สมัครทุกคนในวิชานี้จะถูกลบด้วย
				</Dialog.Description>
			</Dialog.Header>
			<Dialog.Footer>
				<Button
					variant="outline"
					onclick={() => (deletingSubjectTarget = null)}
					disabled={deletingSubject}
				>
					ยกเลิก
				</Button>
				<Button variant="destructive" onclick={confirmDeleteSubject} disabled={deletingSubject}>
					{#if deletingSubject}<Loader2 class="w-4 h-4 mr-2 animate-spin" />{/if}
					{deletingSubject ? 'กำลังลบ...' : 'ลบวิชา'}
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>

	<!-- Delete Confirm Dialog -->
	<Dialog.Root bind:open={showDeleteDialog}>
		<Dialog.Content>
			<Dialog.Header>
				<Dialog.Title>ยืนยันการลบรอบรับสมัคร</Dialog.Title>
				<Dialog.Description>
					ลบ <strong>{round?.name}</strong>? รอบที่มีใบสมัครอยู่จะไม่สามารถลบได้
				</Dialog.Description>
			</Dialog.Header>
			<Dialog.Footer>
				<Button
					variant="outline"
					onclick={() => (showDeleteDialog = false)}
					disabled={deletingRound}
				>
					ยกเลิก
				</Button>
				<Button variant="destructive" onclick={confirmDeleteRound} disabled={deletingRound}>
					{#if deletingRound}<Loader2 class="w-4 h-4 mr-2 animate-spin" />{/if}
					{deletingRound ? 'กำลังลบ...' : 'ลบรอบ'}
				</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>

	<!-- Confirm Status Change Dialog -->
	<Dialog.Root open={pendingStatus !== null} onOpenChange={(o) => { if (!o) pendingStatus = null; }}>
		<Dialog.Content>
			<Dialog.Header>
				<Dialog.Title>ยืนยันการเปลี่ยนสถานะ</Dialog.Title>
				<Dialog.Description>
					เปลี่ยนสถานะรอบเป็น <strong>{pendingStatus ? roundStatusLabel[pendingStatus] : ''}</strong>?
				</Dialog.Description>
			</Dialog.Header>
			<Dialog.Footer>
				<Button variant="outline" onclick={() => (pendingStatus = null)}>ยกเลิก</Button>
				<Button onclick={confirmStatusChange}>ยืนยัน</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
{/if}
