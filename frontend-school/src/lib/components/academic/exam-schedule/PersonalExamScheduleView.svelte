<script lang="ts">
	import type { PersonalExamScheduleRound, PersonalExamSessionView } from '$lib/api/examSchedule';
	import { PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '$lib/components/ui/table';

	interface SessionDateGroup {
		examDate: string;
		sessions: PersonalExamSessionView[];
	}

	interface Props {
		rounds: PersonalExamScheduleRound[];
		showSeatNumber?: boolean;
	}

	let { rounds, showSeatNumber = true }: Props = $props();

	const dateFormatter = new Intl.DateTimeFormat('th-TH', {
		day: 'numeric',
		month: 'short',
		year: 'numeric'
	});

	function formatDate(value: string): string {
		const date = new Date(`${value}T00:00:00`);
		return Number.isNaN(date.getTime()) ? value : dateFormatter.format(date);
	}

	function formatTime(value: string): string {
		if (!value) return '-';
		const timePart = value.includes('T') ? value.split('T')[1] : value;
		return timePart.slice(0, 5);
	}

	function roomLabel(session: PersonalExamSessionView): string {
		return [session.buildingName, session.roomName].filter(Boolean).join(' / ') || '-';
	}

	function personalExamSessionKey(session: PersonalExamSessionView): string {
		return [
			session.examDate,
			session.startsAt,
			session.endsAt,
			session.classroomName,
			session.subjectName,
			session.assessmentCategoryName,
			session.buildingName ?? '',
			session.roomName
		].join('|');
	}

	function groupSessionsByDate(sessions: PersonalExamSessionView[]): SessionDateGroup[] {
		const groups = new Map<string, PersonalExamSessionView[]>();
		for (const session of sessions) {
			const dateSessions = groups.get(session.examDate) ?? [];
			dateSessions.push(session);
			groups.set(session.examDate, dateSessions);
		}

		return Array.from(groups.entries())
			.sort(([leftDate], [rightDate]) => leftDate.localeCompare(rightDate))
			.map(([examDate, dateSessions]) => ({
				examDate,
				sessions: dateSessions.toSorted(
					(left, right) =>
						left.startsAt.localeCompare(right.startsAt) ||
						left.subjectName.localeCompare(right.subjectName)
				)
			}));
	}
</script>

{#if rounds.length === 0}
	<PageState title="ยังไม่มีตารางสอบ" description="ไม่มีตารางสอบที่เผยแพร่ในขณะนี้" />
{:else}
	<div class="space-y-4">
		{#each rounds as round (round.roundId)}
			<section class="space-y-3">
				<div class="flex flex-wrap items-center gap-2">
					<h2 class="text-base font-semibold">{round.roundName}</h2>
					<Badge variant="secondary">{round.sessions.length} วิชา</Badge>
				</div>

				{#if round.sessions.length === 0}
					<PageState
						title="ยังไม่มีรายการสอบ"
						description="ไม่มีตารางสอบที่เผยแพร่สำหรับรอบสอบนี้"
					/>
				{:else}
					<div class="space-y-3">
						{#each groupSessionsByDate(round.sessions) as dateGroup (dateGroup.examDate)}
							<section class="rounded-md border">
								<div class="border-b bg-muted/30 px-3 py-2 text-sm font-medium">
									{formatDate(dateGroup.examDate)}
								</div>
								<Table class="min-w-[860px]">
									<TableHeader>
										<TableRow>
											<TableHead class="w-36">วันที่</TableHead>
											<TableHead class="w-32">เวลา</TableHead>
											<TableHead>วิชา</TableHead>
											<TableHead class="w-44">ประเภทประเมิน</TableHead>
											<TableHead class="w-36">ห้องเรียน</TableHead>
											<TableHead class="w-44">อาคาร / ห้องสอบ</TableHead>
											{#if showSeatNumber}
												<TableHead class="w-28 text-center">เลขที่นั่ง</TableHead>
											{/if}
										</TableRow>
									</TableHeader>
									<TableBody>
										{#each dateGroup.sessions as session (personalExamSessionKey(session))}
											<TableRow>
												<TableCell class="text-sm text-muted-foreground">
													{formatDate(session.examDate)}
												</TableCell>
												<TableCell class="font-mono text-sm">
													{formatTime(session.startsAt)}-{formatTime(session.endsAt)}
												</TableCell>
												<TableCell class="font-medium whitespace-normal">
													{session.subjectName || '-'}
												</TableCell>
												<TableCell class="text-sm whitespace-normal">
													{session.assessmentCategoryName || '-'}
												</TableCell>
												<TableCell class="text-sm">{session.classroomName || '-'}</TableCell>
												<TableCell class="text-sm whitespace-normal">{roomLabel(session)}</TableCell
												>
												{#if showSeatNumber}
													<TableCell class="text-center">
														<Badge variant="outline">{session.seatNumber || '-'}</Badge>
													</TableCell>
												{/if}
											</TableRow>
										{/each}
									</TableBody>
								</Table>
							</section>
						{/each}
					</div>
				{/if}
			</section>
		{/each}
	</div>
{/if}
