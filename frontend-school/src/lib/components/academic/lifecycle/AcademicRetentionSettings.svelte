<script lang="ts">
	import { onMount } from 'svelte';
	import { authStore } from '#lib/stores/auth.js';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { ApiClientError } from '#lib/api/client.js';
	import {
		getAcademicRetentionWorkspace,
		updateAcademicRetentionPolicy,
		type AcademicRetentionWorkspace,
		type UpdateAcademicRetentionPolicy
	} from '#lib/api/academic-year-management.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import { LoadingButton, PageSkeleton } from '#lib/components/app-state/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	let { yearId, terms }: { yearId: string; terms: { academicTermId: string; name: string }[] } =
		$props();
	let workspace = $state<AcademicRetentionWorkspace | null>(null);
	let draft = $state<UpdateAcademicRetentionPolicy | null>(null);
	let expanded = $state(false);
	let loading = $state(false);
	let saving = $state(false);
	let error = $state('');
	let savedMessage = $state('');
	let stale = $state(false);
	const latest = new LatestRequest();
	let alive = false;
	const canRead = $derived(
		$can.hasAll(
			PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL,
			PERMISSIONS.ATTENDANCE_DELETE_SCHOOL,
			PERMISSIONS.BEHAVIOR_CLEANUP_EXECUTE_SCHOOL
		)
	);
	const canManage = $derived(canRead && $can.has(PERMISSIONS.ACADEMIC_LIFECYCLE_MANAGE_SCHOOL));
	const reasons: Record<string, string> = {
		policy_disabled: 'ยังไม่เปิดเก็บกวาดอัตโนมัติ',
		term_not_closed: 'ภาคเรียนยังไม่ปิด',
		close_receipt_missing: 'ยังไม่มีหลักฐานปิดภาคเรียน',
		education_level_unknown: 'ยังระบุระดับการศึกษาไม่ได้',
		outside_policy_effective_period: 'ภาคเรียนเดิมอยู่นอกช่วงที่เปิดใช้นโยบาย',
		year_not_closed: 'รอปิดปีการศึกษา',
		grace_period: 'อยู่ในช่วงเก็บรายละเอียด',
		attendance_already_archived:
			'รายละเอียดเช็กชื่อเก็บถาวรไว้ก่อนหน้านี้ จึงไม่นำมาเก็บกวาดซ้ำอัตโนมัติ'
	};
	function date(value: string | null | undefined) {
		return value
			? new Date(value).toLocaleString('th-TH', { timeZone: 'Asia/Bangkok' })
			: 'ยังไม่กำหนด';
	}
	async function load() {
		if (!canRead || saving) return;
		expanded = true;
		const { revision, signal } = latest.begin();
		const identity = appIdentityKey();
		loading = true;
		error = '';
		try {
			const result = await getAcademicRetentionWorkspace(yearId, { signal });
			if (!latest.isCurrent(revision) || identity !== appIdentityKey()) return;
			workspace = result;
			draft = {
				expectedRowVersion: result.policy.rowVersion,
				enabled: result.policy.enabled,
				primaryBoundary: result.policy.primaryBoundary,
				secondaryBoundary: result.policy.secondaryBoundary,
				graceDays: result.policy.graceDays
			};
			stale = false;
		} catch (cause) {
			if (!isAbortError(cause) && latest.isCurrent(revision))
				error = cause instanceof Error ? cause.message : 'โหลดข้อมูลไม่สำเร็จ';
		} finally {
			if (latest.isCurrent(revision)) loading = false;
		}
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!canManage || !draft || !workspace || saving || stale) return;
		const identity = appIdentityKey();
		saving = true;
		error = '';
		savedMessage = '';
		try {
			const policy = await updateAcademicRetentionPolicy({ ...draft });
			if (!alive || identity !== appIdentityKey()) return;
			workspace = { ...workspace, policy };
			draft = { ...draft, expectedRowVersion: policy.rowVersion };
			savedMessage = 'บันทึกนโยบายแล้ว การบันทึกนี้ไม่ลบรายละเอียด';
			try {
				const latestWorkspace = await getAcademicRetentionWorkspace(yearId);
				if (alive && identity === appIdentityKey()) workspace = latestWorkspace;
			} catch {
				error = 'บันทึกนโยบายแล้ว แต่โหลดขอบเขตล่าสุดไม่สำเร็จ กรุณาลองโหลดข้อมูลล่าสุด';
			}
		} catch (cause) {
			if (!alive || identity !== appIdentityKey()) return;
			error = cause instanceof Error ? cause.message : 'บันทึกไม่สำเร็จ';
			if (cause instanceof ApiClientError && cause.status === 409) stale = true;
		} finally {
			saving = false;
		}
	}
	onMount(() => {
		alive = true;
		let identity = appIdentityKey();
		const unsubscribeIdentity = authStore.subscribe(() => {
			const next = appIdentityKey();
			if (identity && identity !== next) {
				latest.abort();
				expanded = false;
				workspace = null;
				draft = null;
			}
			identity = next;
		});
		const unsubscribe = can.subscribe((permissions) => {
			if (
				!permissions.hasAll(
					PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL,
					PERMISSIONS.ATTENDANCE_DELETE_SCHOOL,
					PERMISSIONS.BEHAVIOR_CLEANUP_EXECUTE_SCHOOL
				)
			) {
				latest.abort();
				workspace = null;
				draft = null;
				expanded = false;
			}
		});
		return () => {
			alive = false;
			unsubscribe();
			unsubscribeIdentity();
			latest.abort();
		};
	});
</script>

{#if canRead}<section
		class="space-y-4 rounded-xl border bg-card p-4 sm:p-5"
		aria-label="ระยะเวลาเก็บรายละเอียด"
	>
		<div>
			<h2 class="text-lg font-semibold">ระยะเวลาเก็บรายละเอียด</h2>
			<p class="mt-1 text-sm text-muted-foreground">
				ตั้งค่าแยกจากการปิดปีและการรีเซ็ตคะแนน ใช้กับรายละเอียดเช็กชื่อ หลักฐาน
				และพฤติกรรมของรอบที่ปิดหลังเปิดใช้นโยบายเท่านั้น
			</p>
		</div>
		{#if !expanded}<Button variant="outline" onclick={() => void load()}
				>ดูนโยบายและขอบเขตข้อมูล</Button
			>{/if}{#if loading && !workspace}<PageSkeleton variant="cards" rows={2} />{/if}{#if error}<p
				role="alert"
				class="text-sm text-destructive"
			>
				{error}
			</p>
			<Button variant="outline" disabled={saving} onclick={() => void load()}
				>โหลดข้อมูลล่าสุด</Button
			>{/if}{#if workspace && draft}<p class="rounded-lg bg-muted p-3 text-sm">
				รายละเอียดถูกเก็บกวาดทั้งภาคเรียนพร้อมกัน หากมีทั้งประถมและมัธยม จะรอขอบเขตที่ช้าที่สุด
				ไม่แยกลบตามระดับชั้น
			</p>
			<form class="max-w-2xl space-y-4" onsubmit={save}>
				<div class="flex items-center gap-3">
					<Switch
						id="retention-enabled"
						bind:checked={draft.enabled}
						disabled={!canManage || saving}
					/><Label for="retention-enabled">เปิดเก็บกวาดรายละเอียดอัตโนมัติ</Label>
				</div>
				<div class="grid gap-4 sm:grid-cols-3">
					<div class="space-y-2">
						<Label for="retention-primary">ประถม: เก็บจนถึง</Label><Select.Root
							type="single"
							bind:value={draft.primaryBoundary}
							disabled={!canManage || saving}
							><Select.Trigger id="retention-primary" class="w-full"
								>{draft.primaryBoundary === 'year'
									? 'ปิดปีการศึกษา'
									: 'ปิดภาคเรียน'}</Select.Trigger
							><Select.Content
								><Select.Item value="year">ปิดปีการศึกษา</Select.Item><Select.Item value="term"
									>ปิดภาคเรียน</Select.Item
								></Select.Content
							></Select.Root
						>
					</div>
					<div class="space-y-2">
						<Label for="retention-secondary">มัธยม: เก็บจนถึง</Label><Select.Root
							type="single"
							bind:value={draft.secondaryBoundary}
							disabled={!canManage || saving}
							><Select.Trigger id="retention-secondary" class="w-full"
								>{draft.secondaryBoundary === 'year'
									? 'ปิดปีการศึกษา'
									: 'ปิดภาคเรียน'}</Select.Trigger
							><Select.Content
								><Select.Item value="year">ปิดปีการศึกษา</Select.Item><Select.Item value="term"
									>ปิดภาคเรียน</Select.Item
								></Select.Content
							></Select.Root
						>
					</div>
					<div class="space-y-2">
						<Label for="retention-days">เก็บต่ออีก (วัน)</Label><Input
							id="retention-days"
							type="number"
							min={1}
							max={3650}
							bind:value={draft.graceDays}
							disabled={!canManage || saving}
							required
						/>
					</div>
				</div>
				<p class="text-sm text-muted-foreground">
					วันที่เริ่มมีผล: {date(workspace.policy.effectiveFrom)} ระบบเป็นผู้กำหนดเมื่อเปิดใช้งาน ข้อมูลที่ปิดก่อนหน้านี้ไม่ถูกรวมย้อนหลัง
				</p>
				{#if canManage}<LoadingButton type="submit" loading={saving} disabled={stale || loading}
						>บันทึกระยะเวลาเก็บรายละเอียด</LoadingButton
					>{/if}
			</form>
			{#if savedMessage}<p role="status" class="text-sm">{savedMessage}</p>{/if}
			<div class="overflow-x-auto">
				<Table.Root
					><Table.Header
						><Table.Row
							><Table.Head>ภาคเรียน</Table.Head><Table.Head>ขอบเขต / วันที่ถึงกำหนด</Table.Head
							><Table.Head>รายละเอียดในขอบเขต</Table.Head><Table.Head>สถานะ</Table.Head></Table.Row
						></Table.Header
					><Table.Body
						>{#each workspace.terms as term (term.academicTermId)}<Table.Row
								><Table.Cell
									>{terms.find((item) => item.academicTermId === term.academicTermId)?.name ??
										'ภาคเรียนที่ยังไม่มีชื่อ'}
									<p class="text-xs text-muted-foreground">
										{term.levelTypes
											.map((level) =>
												level === 'primary'
													? 'ประถม'
													: level === 'secondary'
														? 'มัธยม'
														: 'ไม่ทราบระดับ'
											)
											.join(', ')}
									</p></Table.Cell
								><Table.Cell
									>{term.requiredBoundary === 'year' ? 'ปิดปีการศึกษา' : 'ปิดภาคเรียน'}
									<p class="text-xs text-muted-foreground">{date(term.eligibleAt)}</p></Table.Cell
								><Table.Cell
									><p>เช็กชื่อ {term.impact.attendanceRecords} รายการ</p>
									<p>
										หลักฐาน {term.impact.evidenceFiles} ไฟล์ · ขนาดไฟล์ที่เกี่ยวข้อง {term.impact
											.evidenceBytes} ไบต์
									</p>
									<p>พฤติกรรม {term.impact.behaviorEntries} รายการ</p></Table.Cell
								><Table.Cell
									>{#if term.job}<p>
											{term.job.physicalCleanupComplete
												? 'เก็บกวาดครบแล้ว'
												: term.job.logicalCleanupComplete
													? 'เก็บกวาดรายละเอียดแล้ว รอจัดการไฟล์จริง'
													: 'กำลังเก็บกวาด'}
										</p>
										<p class="text-xs">
											รอไฟล์ {term.job.pendingFiles} · ติดขัด {term.job.failedFiles}
										</p>{:else if term.eligible}ถึงกำหนดตามนโยบายแล้ว{:else}{#each term.blockedReasons as reason (reason)}<p
												class="text-sm"
											>
												{reasons[reason] ?? 'ยังไม่เข้าเงื่อนไข กรุณาตรวจข้อมูลล่าสุด'}
											</p>{/each}{/if}</Table.Cell
								></Table.Row
							>{/each}</Table.Body
					></Table.Root
				>
			</div>
			<p class="text-sm text-muted-foreground">
				เก็บตัวตน ประวัติปี/ห้อง สรุปการเช็กชื่อ คะแนนพฤติกรรมรวม ผลการเรียน และหลักฐานตรวจสอบไว้
				เมื่อหมดระยะเวลา รายละเอียดจะเปิดดูไม่ได้ การลบไฟล์จริงอาจยังรอดำเนินการตามสถานะข้างต้น
				ขนาดไฟล์ที่เกี่ยวข้องไม่ใช่พื้นที่ดิสก์ที่จะคืนได้แน่นอน
			</p>{/if}
	</section>{/if}
