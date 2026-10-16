<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { authStore } from '#lib/stores/auth.js';
	import { createStudent } from '#lib/api/students.js';
	import {
		studentRegisterDraft,
		studentRegisterErrors,
		createStudentRegisterInput
	} from '#lib/forms/student-register.js';
	import { withStudentRegisterContext } from '#lib/navigation/student-register.js';
	import StudentRegisterForm from '#lib/components/student/registry/StudentRegisterForm.svelte';
	let { data }: PageProps = $props();
	const canCreateStudent = $derived($can.has(PERMISSIONS.STUDENT_CREATE_ALL));
	const canHandleStudentPii = $derived(canCreateStudent);
	let draft = $state(studentRegisterDraft());
	let errors = $state<Record<string, string>>({});
	let formError = $state('');
	let saving = $state(false);
	let disposed = false,
		revision = 0,
		owner = '';
	$effect.pre(() => {
		const nextOwner = `${$authStore.user?.id ?? ''}:${canCreateStudent}`;
		untrack(() => {
			if (owner && owner !== nextOwner) {
				revision++;
				draft = studentRegisterDraft();
				errors = {};
				formError = '';
				saving = false;
			}
			owner = nextOwner;
		});
	});
	onDestroy(() => {
		disposed = true;
		revision++;
		draft = studentRegisterDraft();
	});
	async function save() {
		if (!canCreateStudent || saving) return;
		errors = studentRegisterErrors(draft, 'create');
		if (Object.keys(errors).length) {
			formError = 'ตรวจสอบข้อมูลที่ระบุด้านล่าง';
			return;
		}
		const currentRevision = ++revision,
			session = authStore.sessionEpoch;
		const context = data.context;
		const current = () =>
			!disposed &&
			revision === currentRevision &&
			session === authStore.sessionEpoch &&
			canCreateStudent;
		saving = true;
		formError = '';
		try {
			const result = await createStudent(createStudentRegisterInput(draft, canHandleStudentPii));
			if (!current()) return;
			draft = studentRegisterDraft();
			await goto(
				withStudentRegisterContext(
					resolve('/(app)/staff/students/[id]', { id: result.id }),
					context
				)
			);
		} catch (error) {
			if (current()) formError = error instanceof Error ? error.message : 'เพิ่มนักเรียนไม่สำเร็จ';
		} finally {
			if (current()) saving = false;
		}
	}
</script>

<PageShell
	title="เพิ่มทะเบียนนักเรียน"
	description="บันทึกข้อมูลประจำตัวและสถานะทะเบียนกลาง"
	backHref={data.context.returnTo}
	backPreload={false}
>
	{#if !canCreateStudent}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์เพิ่มนักเรียน"
			description="ต้องมีสิทธิ์สร้างนักเรียนจึงจะบันทึกข้อมูลใหม่ได้"
		/>
	{:else}
		<StudentRegisterForm
			mode="create"
			bind:value={draft}
			{errors}
			{saving}
			{formError}
			allowNationalId={canHandleStudentPii}
			cancelHref={data.context.returnTo}
			onsubmit={save}
		/>
	{/if}
</PageShell>
