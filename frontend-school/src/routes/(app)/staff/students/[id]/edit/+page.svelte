<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { PageProps } from './$types';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { withStudentRegisterContext } from '#lib/navigation/student-register.js';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import {
		getStudent,
		updateStudent,
		addParentToStudent,
		removeParentFromStudent,
		type StudentRegister
	} from '#lib/api/students.js';
	import {
		studentRegisterDraft,
		studentRegisterReadScope,
		studentRegisterErrors,
		updateStudentRegisterInput,
		studentParentDraft,
		studentParentErrors
	} from '#lib/forms/student-register.js';
	import StudentRegisterForm from '#lib/components/student/registry/StudentRegisterForm.svelte';
	import StudentParentFields from '#lib/components/student/registry/StudentParentFields.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { authStore } from '#lib/stores/auth.js';
	import { Plus, Trash2, RefreshCw } from '@lucide/svelte';
	let { data }: PageProps = $props();
	const readScope = $derived(
		studentRegisterReadScope({
			school: $can.has(PERMISSIONS.STUDENT_READ_SCHOOL),
			assigned: $can.has(PERMISSIONS.STUDENT_READ_ASSIGNED),
			own: $can.has(PERMISSIONS.STUDENT_READ_OWN),
			piiSchool: $can.has(PERMISSIONS.STUDENT_PII_READ_SCHOOL),
			piiAssigned: $can.has(PERMISSIONS.STUDENT_PII_READ_ASSIGNED),
			piiOwn: $can.has(PERMISSIONS.STUDENT_PII_READ_OWN)
		})
	);

	const canReadStudent = $derived(
		$can.hasAny(
			PERMISSIONS.STUDENT_READ_SCHOOL,
			PERMISSIONS.STUDENT_READ_ASSIGNED,
			PERMISSIONS.STUDENT_READ_OWN
		)
	);
	const canUpdateStudent = $derived($can.has(PERMISSIONS.STUDENT_UPDATE_ALL));
	const canReadStudentPii = $derived(
		$can.hasAny(
			PERMISSIONS.STUDENT_PII_READ_SCHOOL,
			PERMISSIONS.STUDENT_PII_READ_ASSIGNED,
			PERMISSIONS.STUDENT_PII_READ_OWN
		)
	);
	let student = $state.raw<StudentRegister | null>(null);
	let draft = $state(studentRegisterDraft());
	let errors = $state<Record<string, string>>({});
	let loading = $state(true),
		saving = $state(false),
		formError = $state(''),
		loadError = $state('');
	let parentOpen = $state(false),
		parentBusy = $state(false),
		parentError = $state('');
	let parentDraft = $state(studentParentDraft()),
		parentErrors = $state<Record<string, string>>({});
	let removingParent = $state<string | null>(null);
	let disposed = false,
		revision = 0,
		owner = '',
		parentRevision = 0,
		draftInitialized = false;
	const request = new LatestRequest();
	$effect.pre(() => {
		const nextOwner = `${data.studentId}:${$authStore.user?.id ?? ''}:${readScope}:${canUpdateStudent}`;
		untrack(() => {
			if (owner !== nextOwner) {
				owner = nextOwner;
				revision++;
				request.abort();
				student = null;
				loading = true;
				loadError = '';
				draft = studentRegisterDraft();
				draftInitialized = false;
				errors = {};
				formError = '';
				saving = false;
				parentOpen = false;
				parentBusy = false;
				parentDraft = studentParentDraft();
				parentErrors = {};
				removingParent = null;
				parentError = '';
				parentRevision++;
			}
		});
	});
	$effect.pre(() => {
		const source = data.student;
		untrack(() => {
			const ticket = request.begin(),
				session = authStore.sessionEpoch,
				scope = readScope;
			loading = true;
			loadError = '';
			void source.then((result) => {
				if (
					session !== authStore.sessionEpoch ||
					scope !== readScope ||
					!request.isCurrent(ticket.revision) ||
					!canReadStudent
				)
					return;
				loading = false;
				if (result.ok) {
					student = result.data;
					if (student && !draftInitialized) {
						draft = studentRegisterDraft(student);
						draftInitialized = true;
					}
				} else loadError = result.error;
			});
		});
		return () => request.abort();
	});
	$effect.pre(() => {
		const open = parentOpen;
		untrack(() => {
			if (!open) {
				parentRevision++;
				parentDraft = studentParentDraft();
				parentErrors = {};
				parentError = '';
			}
		});
	});
	onDestroy(() => {
		disposed = true;
		revision++;
		parentRevision++;
		request.abort();
		draft = studentRegisterDraft();
		parentDraft = studentParentDraft();
	});
	async function reloadProfile() {
		if (!canReadStudent || saving || parentBusy) return;
		const ticket = request.begin(),
			session = authStore.sessionEpoch,
			scope = readScope;
		loading = true;
		loadError = '';
		const result = await captureRouteLoad(
			getStudent(data.studentId, null, { signal: ticket.signal }),
			'โหลดข้อมูลนักเรียนไม่สำเร็จ'
		);
		if (
			!request.isCurrent(ticket.revision) ||
			session !== authStore.sessionEpoch ||
			scope !== readScope ||
			!canReadStudent
		)
			return;
		loading = false;
		if (result.ok) {
			student = result.data;
			if (student && !draftInitialized) {
				draft = studentRegisterDraft(student);
				draftInitialized = true;
			}
		} else loadError = result.error;
	}
	function mutation() {
		const currentRevision = ++revision,
			session = authStore.sessionEpoch,
			scope = readScope,
			person = data.studentId;
		request.abort();
		loading = false;
		return () =>
			!disposed &&
			revision === currentRevision &&
			session === authStore.sessionEpoch &&
			scope === readScope &&
			person === data.studentId &&
			canReadStudent &&
			canUpdateStudent;
	}
	async function save() {
		if (!student || !canReadStudent || !canUpdateStudent || saving || parentBusy) return;
		errors = studentRegisterErrors(draft, 'edit');
		if (Object.keys(errors).length) {
			formError = 'ตรวจสอบข้อมูลที่ระบุด้านล่าง';
			return;
		}
		const current = mutation(),
			destination = withStudentRegisterContext(
				resolve('/(app)/staff/students/[id]', { id: data.studentId }),
				data.context
			);
		saving = true;
		formError = '';
		try {
			await updateStudent(data.studentId, updateStudentRegisterInput(draft));
			if (current())
				await goto(destination, {
					invalidate: [`school:student-register-profile:${data.studentId}`]
				});
		} catch (error) {
			if (current()) formError = error instanceof Error ? error.message : 'บันทึกไม่สำเร็จ';
		} finally {
			if (current()) saving = false;
		}
	}
	async function addParent() {
		if (!student || !canUpdateStudent || !canReadStudent || saving || parentBusy || !parentOpen)
			return;
		parentErrors = studentParentErrors(parentDraft);
		if (Object.keys(parentErrors).length) return;
		const current = mutation(),
			draftRevision = parentRevision;
		parentBusy = true;
		parentError = '';
		try {
			await addParentToStudent(data.studentId, {
				...parentDraft,
				title: parentDraft.title.trim() || undefined,
				first_name: parentDraft.first_name.trim(),
				last_name: parentDraft.last_name.trim(),
				phone: parentDraft.phone.trim(),
				relationship: parentDraft.relationship.trim(),
				email: parentDraft.email.trim() || undefined,
				national_id: canReadStudentPii ? parentDraft.national_id.trim() || undefined : undefined
			});
			if (!current()) return;
			const result = await getStudent(data.studentId);
			if (!current()) return;
			student = result;
			if (draftRevision === parentRevision) parentOpen = false;
		} catch (error) {
			if (current() && draftRevision === parentRevision)
				parentError = error instanceof Error ? error.message : 'เพิ่มผู้ปกครองไม่สำเร็จ';
		} finally {
			if (current()) parentBusy = false;
		}
	}
	async function removeParent() {
		if (!removingParent || !canUpdateStudent || !canReadStudent || saving || parentBusy) return;
		const current = mutation(),
			id = removingParent;
		parentBusy = true;
		formError = '';
		try {
			await removeParentFromStudent(data.studentId, id);
			if (!current()) return;
			if (student)
				student = { ...student, parents: student.parents.filter((parent) => parent.id !== id) };
			removingParent = null;
		} catch (error) {
			if (current()) formError = error instanceof Error ? error.message : 'นำผู้ปกครองออกไม่สำเร็จ';
		} finally {
			if (current()) parentBusy = false;
		}
	}
</script>

<PageShell
	title="แก้ไขทะเบียนนักเรียน"
	description={student
		? `${student.title ?? ''}${student.first_name} ${student.last_name}`
		: 'ข้อมูลประจำตัวและสถานะทะเบียนกลาง'}
	backHref={data.detailHref}
	backPreload={false}
>
	{#snippet actions()}<Button
			variant="outline"
			onclick={reloadProfile}
			disabled={loading || saving || parentBusy || !canReadStudent}
			><RefreshCw aria-hidden="true" />รีเฟรช</Button
		>{/snippet}
	<section data-testid="student-profile" aria-busy={loading}>
		{#if !canReadStudent}<PageState variant="permission" title="ไม่มีสิทธิ์ดูข้อมูลนักเรียน" />
		{:else if !canUpdateStudent}<PageState
				variant="permission"
				title="อ่านได้อย่างเดียว"
				description="บัญชีนี้ยังไม่มีสิทธิ์แก้ไขข้อมูลนักเรียน"
				href={data.detailHref}
				actionLabel="ดูข้อมูลนักเรียน"
			/>
		{:else if loading && !student}<div role="status" aria-label="กำลังโหลดข้อมูลนักเรียน">
				<PageSkeleton variant="form" rows={6} />
			</div>
		{:else if loadError && !student}<PageState
				variant="error"
				title="โหลดข้อมูลนักเรียนไม่สำเร็จ"
				description={loadError}
				actionLabel="ลองอีกครั้ง"
				onaction={reloadProfile}
			/>
		{:else if student}
			{#if loadError}<PageState
					variant="error"
					title="อัปเดตข้อมูลนักเรียนไม่สำเร็จ"
					description={loadError}
					actionLabel="ลองอีกครั้ง"
					onaction={reloadProfile}
				/>{/if}
			{#if loading}<p role="status">กำลังอัปเดตข้อมูลนักเรียน...</p>{/if}
			<StudentRegisterForm
				mode="edit"
				bind:value={draft}
				{errors}
				{saving}
				disabled={parentBusy}
				{formError}
				cancelHref={data.detailHref}
				onsubmit={save}
			>
				{#snippet guardians()}
					<Card.Root
						><Card.Header
							><div class="flex flex-wrap items-center justify-between gap-3">
								<Card.Title><h2>ผู้ปกครอง</h2></Card.Title><Button
									type="button"
									variant="outline"
									onclick={() => (parentOpen = true)}
									disabled={saving || parentBusy}><Plus aria-hidden="true" />เพิ่มผู้ปกครอง</Button
								>
							</div></Card.Header
						>
						<Card.Content class="space-y-3"
							>{#each student?.parents ?? [] as parent (parent.id)}<div
									class="flex items-start justify-between gap-3 rounded-lg border p-4"
								>
									<div class="min-w-0 space-y-1">
										<p class="break-words font-medium">{parent.first_name} {parent.last_name}</p>
										<p class="break-words text-sm text-muted-foreground">
											{parent.relationship}{parent.is_primary
												? ' · ผู้ปกครองหลัก'
												: ''}{parent.phone ? ` · ${parent.phone}` : ''}
										</p>
									</div>
									<Button
										type="button"
										variant="ghost"
										size="icon"
										aria-label="ลบผู้ปกครอง"
										onclick={() => (removingParent = parent.id)}
										disabled={saving || parentBusy}><Trash2 aria-hidden="true" /></Button
									>
								</div>{:else}<p class="text-sm text-muted-foreground">
									ยังไม่มีข้อมูลผู้ปกครอง
								</p>{/each}</Card.Content
						></Card.Root
					>
				{/snippet}
			</StudentRegisterForm>
		{:else}<PageState title="ไม่พบนักเรียน" />{/if}
	</section>
</PageShell>
{#if canReadStudent && canUpdateStudent}
	<Dialog.Root bind:open={parentOpen}
		><Dialog.Content
			><Dialog.Header
				><Dialog.Title>เพิ่มผู้ปกครอง</Dialog.Title><Dialog.Description
					>เชื่อมโยงผู้ปกครองกับนักเรียน หากเบอร์โทรซ้ำระบบจะใช้บัญชีเดิม</Dialog.Description
				></Dialog.Header
			>
			<form
				class="space-y-4"
				novalidate
				onsubmit={(event) => {
					event.preventDefault();
					void addParent();
				}}
			>
				<StudentParentFields
					bind:value={parentDraft}
					idPrefix="student-parent"
					errors={parentErrors}
					disabled={parentBusy}
					allowNationalId={canReadStudentPii}
				/>{#if parentError}<p role="alert" class="text-sm text-destructive">
						{parentError}
					</p>{/if}<Dialog.Footer
					><Button
						type="button"
						variant="outline"
						onclick={() => (parentOpen = false)}
						disabled={parentBusy}>ยกเลิก</Button
					><LoadingButton type="submit" loading={parentBusy} loadingLabel="กำลังเพิ่ม..."
						>บันทึก</LoadingButton
					></Dialog.Footer
				>
			</form></Dialog.Content
		></Dialog.Root
	>
	<AlertDialog.Root
		open={Boolean(removingParent)}
		onOpenChange={(open) => {
			if (!open && !parentBusy) removingParent = null;
		}}
		><AlertDialog.Content
			><AlertDialog.Header
				><AlertDialog.Title>นำผู้ปกครองออกจากนักเรียน?</AlertDialog.Title><AlertDialog.Description
					>ยกเลิกความเชื่อมโยงนี้ โดยไม่ลบบัญชีผู้ปกครอง</AlertDialog.Description
				></AlertDialog.Header
			><AlertDialog.Footer
				><AlertDialog.Cancel disabled={parentBusy}>ยกเลิก</AlertDialog.Cancel><Button
					variant="destructive"
					onclick={removeParent}
					disabled={parentBusy}>{parentBusy ? 'กำลังนำออก...' : 'นำออก'}</Button
				></AlertDialog.Footer
			></AlertDialog.Content
		></AlertDialog.Root
	>
{/if}
