<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import { formatEffectiveResultValue, resultKindLabel } from '$lib/academic/results/presentation';
	import {
		correctEffectiveAcademicResult,
		searchEffectiveAcademicResults,
		type AcademicResultCorrectionInput,
		type EffectiveResultKind,
		type EffectiveResultSearchItem
	} from '$lib/api/academicResults';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import AcademicPrerequisiteNotice from '$lib/components/academic-workflow/AcademicPrerequisiteNotice.svelte';
	import ResultCorrectionDialog from '$lib/components/academic/results/ResultCorrectionDialog.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { History, Search } from 'lucide-svelte';

	const academicContext = getAcademicContextStore();
	const request = new LatestRequest();

	let results = $state.raw<EffectiveResultSearchItem[]>([]);
	let searchText = $state('');
	let selectedKind = $state<'all' | EffectiveResultKind>('all');
	let loading = $state(false);
	let errorMessage = $state('');
	let selectedItem = $state.raw<EffectiveResultSearchItem | null>(null);
	let dialogOpen = $state(false);
	let dialogRevision = $state(0);
	let correcting = $state(false);
	let correctionError = $state('');

	const academicYearId = $derived($academicContext.selected.academicYearId);
	const academicTermId = $derived($academicContext.selected.academicTermId);
	const canCorrectCourseResults = $derived($can.has(PERMISSIONS.ACADEMIC_RESULT_CORRECT_SCHOOL));
	const canCorrectLearnerEvaluations = $derived(
		$can.has(PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_CORRECT_SCHOOL)
	);

	function contextValue() {
		return academicYearId && academicTermId ? { academicYearId, academicTermId } : null;
	}

	async function searchResults(): Promise<void> {
		const context = contextValue();
		if (!context) return;
		const { revision, signal } = request.begin();
		loading = true;
		errorMessage = '';
		try {
			const cleanSearch = searchText.trim();
			const next = await searchEffectiveAcademicResults(
				{
					...context,
					...(cleanSearch ? { search: cleanSearch } : {}),
					...(selectedKind === 'all' ? {} : { kind: selectedKind }),
					limit: 100
				},
				{ signal }
			);
			if (request.isCurrent(revision)) results = next;
		} catch (error) {
			if (isAbortError(error)) return;
			if (request.isCurrent(revision)) {
				errorMessage = error instanceof Error ? error.message : 'ค้นหาผลการเรียนไม่สำเร็จ';
			}
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	function openCorrection(item: EffectiveResultSearchItem): void {
		selectedItem = item;
		correctionError = '';
		dialogRevision += 1;
		dialogOpen = true;
	}

	async function correctResult(input: AcademicResultCorrectionInput): Promise<void> {
		const context = contextValue();
		if (!context || !selectedItem) return;
		if (input.kind === 'learner_evaluation' && !canCorrectLearnerEvaluations) return;
		if (input.kind !== 'learner_evaluation' && !canCorrectCourseResults) return;
		correcting = true;
		correctionError = '';
		try {
			const updated = await correctEffectiveAcademicResult(context, input);
			results = results.map((item) =>
				item.result.resultId === updated.resultId ? { ...item, result: updated } : item
			);
			selectedItem = selectedItem ? { ...selectedItem, result: updated } : null;
			toast.success('บันทึกผลใหม่และเพิ่มประวัติแล้ว');
			dialogOpen = false;
		} catch (error) {
			correctionError = error instanceof Error ? error.message : 'แก้ผลการเรียนไม่สำเร็จ';
		} finally {
			correcting = false;
		}
	}

	async function refreshSelected(): Promise<void> {
		const resultId = selectedItem?.result.resultId;
		await searchResults();
		if (!resultId) return;
		const refreshed = results.find((item) => item.result.resultId === resultId);
		if (refreshed) {
			selectedItem = refreshed;
			correctionError = '';
			dialogRevision += 1;
		} else {
			dialogOpen = false;
			toast.error('ไม่พบผลรายการเดิมในข้อมูลล่าสุด');
		}
	}

	onMount(() => {
		let loadedContextKey = '';
		const unsubscribe = academicContext.subscribe((state) => {
			const contextKey =
				state.selected.academicYearId && state.selected.academicTermId
					? `${state.selected.academicYearId}:${state.selected.academicTermId}`
					: '';
			if (contextKey && contextKey !== loadedContextKey) {
				loadedContextKey = contextKey;
				void searchResults();
			} else if (!contextKey) {
				loadedContextKey = '';
				request.abort();
				results = [];
			}
		});
		return () => {
			unsubscribe();
			request.abort();
		};
	});
</script>

<PageShell
	title="แก้ผลการเรียน"
	description="แก้เฉพาะผลที่ล็อกแล้ว โดยคงผลเริ่มต้นและเพิ่มประวัติทุกครั้ง"
>
	{#if !canCorrectCourseResults && !canCorrectLearnerEvaluations}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์แก้ผลการเรียน"
			description="หน้านี้ใช้สำหรับฝ่ายวิชาการที่ได้รับสิทธิ์แก้ผลระดับโรงเรียน"
		/>
	{:else if !academicYearId || !academicTermId}
		<PageState
			variant="empty"
			title="เลือกปีการศึกษาและภาคเรียนก่อน"
			description="ใช้ตัวเลือกบนแถบด้านบนเพื่อค้นหาผลในภาคเรียนที่ต้องการ"
		/>
	{:else}
		<div class="space-y-4">
			<form
				class="grid gap-3 rounded-xl border bg-card p-4 md:grid-cols-[minmax(0,1fr)_16rem_auto] md:items-end"
				onsubmit={(event) => {
					event.preventDefault();
					void searchResults();
				}}
			>
				<div class="space-y-1.5">
					<Label for="result-search">ค้นหานักเรียนหรือรายวิชา</Label><Input
						id="result-search"
						bind:value={searchText}
						placeholder="ชื่อ เลขประจำตัว รหัส หรือชื่อรายวิชา"
					/>
				</div>
				<div class="space-y-1.5">
					<Label for="result-kind">ประเภทผล</Label><Select.Root
						type="single"
						bind:value={selectedKind}
						><Select.Trigger id="result-kind" class="w-full"
							>{selectedKind === 'all'
								? 'ทุกประเภท'
								: resultKindLabel(selectedKind)}</Select.Trigger
						><Select.Content
							><Select.Item value="all">ทุกประเภท</Select.Item><Select.Item value="course"
								>ผลการเรียนรายวิชา</Select.Item
							><Select.Item value="activity">ผลกิจกรรม</Select.Item><Select.Item
								value="learner_evaluation">ผลประเมินผู้เรียน</Select.Item
							></Select.Content
						></Select.Root
					>
				</div>
				<Button type="submit" disabled={loading}><Search class="size-4" /> ค้นหา</Button>
			</form>

			{#if loading}
				<PageSkeleton variant="table" rows={8} columns={5} />
			{:else if errorMessage}
				<PageState
					variant="error"
					title="ค้นหาผลการเรียนไม่สำเร็จ"
					description={errorMessage}
					actionLabel="ลองอีกครั้ง"
					onaction={() => void searchResults()}
				/>
			{:else if results.length === 0}
				<AcademicPrerequisiteNotice
					prerequisite={{
						key: 'result-correction-search',
						status: 'missing',
						title: 'ยังไม่พบผลที่ล็อกแล้ว',
						description: 'ตรวจภาคเรียนและตัวกรอง หรือสร้างผลเริ่มต้นด้วยการล็อกผลก่อน',
						actionLabel: 'ไปล็อกผลการเรียน',
						href: '/staff/academic/result-locks'
					}}
				/>
			{:else}
				<div class="overflow-hidden rounded-xl border bg-card">
					<div class="border-b px-4 py-3">
						<p class="font-semibold">ผลที่ค้นพบ {results.length} รายการ</p>
						<p class="text-sm text-muted-foreground">การแก้ไขจะไม่เปลี่ยนคะแนนหรือผลเริ่มต้น</p>
					</div>
					<div class="overflow-x-auto">
						<Table.Root
							><Table.Header
								><Table.Row
									><Table.Head>นักเรียน</Table.Head><Table.Head>รายการ</Table.Head><Table.Head
										>กลุ่ม</Table.Head
									><Table.Head>ผลปัจจุบัน</Table.Head><Table.Head class="w-32"
									></Table.Head></Table.Row
								></Table.Header
							><Table.Body>
								{#each results as item (`${item.kind}:${item.result.resultId}`)}
									<Table.Row
										><Table.Cell
											><p class="font-medium">{item.displayName}</p>
											{#if item.studentCode}<p class="text-xs text-muted-foreground">
													{item.studentCode}
												</p>{/if}</Table.Cell
										><Table.Cell
											><p>{item.offeringCode} · {item.offeringName}</p>
											<Badge variant="outline" class="mt-1">{resultKindLabel(item.kind)}</Badge
											>{#if item.criterionName}<p class="mt-1 text-xs text-muted-foreground">
													{item.criterionName}
												</p>{/if}</Table.Cell
										><Table.Cell>{item.groupName}</Table.Cell><Table.Cell class="font-semibold"
											>{formatEffectiveResultValue(item.result.effective)}</Table.Cell
										><Table.Cell
											><Button size="sm" variant="outline" onclick={() => openCorrection(item)}
												><History class="size-4" /> แก้ผล</Button
											></Table.Cell
										></Table.Row
									>
								{/each}
							</Table.Body></Table.Root
						>
					</div>
				</div>
			{/if}
		</div>
	{/if}
</PageShell>

{#if selectedItem}
	{#key dialogRevision}
		<ResultCorrectionDialog
			open={dialogOpen}
			item={selectedItem}
			busy={correcting}
			errorMessage={correctionError}
			onopenchange={(open) => (dialogOpen = open)}
			oncorrect={(input) => void correctResult(input)}
			onrefresh={() => void refreshSelected()}
		/>
	{/key}
{/if}
