import type {
	TimetableBlock,
	TimetableBlockPlacementCandidate,
	TimetableBlockPlacementSource,
	TimetableBlockWorkspace,
	TimetableTargetKind
} from '../../api/timetable';

export interface TimetableOptimisticPlacementSource {
	source: TimetableBlockPlacementSource;
	candidate: TimetableBlockPlacementCandidate;
}

function periodDetails(
	workspace: TimetableBlockWorkspace,
	bellSchedulePeriodId: string
): Pick<TimetableBlock, 'periodName' | 'startTime' | 'endTime'> {
	const period = workspace.bellPeriods.find((item) => item.id === bellSchedulePeriodId);
	if (!period) throw new Error('ไม่พบคาบเวลาปลายทาง');
	return {
		periodName: period.name ?? `คาบที่ ${period.orderIndex}`,
		startTime: period.startTime,
		endTime: period.endTime
	};
}

export function moveTimetableBlockOptimistically(
	workspace: TimetableBlockWorkspace,
	block: TimetableBlock,
	dayOfWeek: string,
	bellSchedulePeriodId: string
): TimetableBlock {
	return {
		...block,
		dayOfWeek,
		bellSchedulePeriodId,
		...periodDetails(workspace, bellSchedulePeriodId)
	};
}

export function swapTimetableBlocksOptimistically(
	workspace: TimetableBlockWorkspace,
	blockA: TimetableBlock,
	blockB: TimetableBlock
): [TimetableBlock, TimetableBlock] {
	return [
		moveTimetableBlockOptimistically(
			workspace,
			blockA,
			blockB.dayOfWeek,
			blockB.bellSchedulePeriodId
		),
		moveTimetableBlockOptimistically(
			workspace,
			blockB,
			blockA.dayOfWeek,
			blockA.bellSchedulePeriodId
		)
	];
}

export function createOptimisticTimetableBlock(
	workspace: TimetableBlockWorkspace,
	dragSource: TimetableOptimisticPlacementSource,
	dayOfWeek: string,
	bellSchedulePeriodId: string,
	id: string,
	timestamp = new Date().toISOString()
): TimetableBlock {
	if (dragSource.source.kind === 'existing_block') {
		throw new Error('คาบที่มีอยู่แล้วต้องใช้การย้ายแทนการสร้าง');
	}
	const room = workspace.rooms.find((item) => item.id === dragSource.candidate.roomId);
	const base = {
		id,
		timetableVersionId: workspace.version.id,
		academicTermId: workspace.version.academicTermId,
		academicYearId: workspace.version.academicYearId,
		bellScheduleId: workspace.version.bellScheduleId,
		bellSchedulePeriodId,
		blockKind: dragSource.candidate.blockKind,
		learningOfferingId: dragSource.candidate.learningOfferingId,
		structuralKind: null,
		seriesId: null,
		dayOfWeek,
		...periodDetails(workspace, bellSchedulePeriodId),
		title: null,
		note: null,
		isActive: true,
		syncStates: [],
		rowVersion: 0,
		createdAt: timestamp,
		updatedAt: timestamp
	} satisfies Omit<
		TimetableBlock,
		'offeringCode' | 'offeringName' | 'schedulingMode' | 'groups' | 'homerooms' | 'teachers'
	>;

	if (dragSource.source.kind === 'ordinary_demand') {
		const source = dragSource.source;
		const demand = workspace.ordinaryDemands.find(
			(item) => item.learningGroupId === source.learningGroupId
		);
		const group = workspace.learningGroups.find((item) => item.id === source.learningGroupId);
		if (!demand || !group) throw new Error('ไม่พบกลุ่มเรียนที่กำลังจัดตาราง');
		const instructorIds = dragSource.candidate.instructorIds ?? [];
		return {
			...base,
			offeringCode: demand.offeringCode,
			offeringName: demand.offeringName,
			schedulingMode: dragSource.candidate.blockKind === 'activity' ? 'independent' : null,
			groups: [
				{
					id: `${id}:group`,
					learningGroupId: group.id,
					learningOfferingId: group.learningOfferingId,
					code: group.code,
					name: group.name,
					homeroomIds: group.homeroomIds,
					instructors: instructorIds.map((teacherId, index) => {
						const eligible = group.eligibleInstructors.find(
							(teacher) => teacher.teacherId === teacherId
						);
						return {
							teacherId,
							displayName:
								eligible?.displayName ??
								workspace.staff.find((teacher) => teacher.id === teacherId)?.displayName ??
								'ครูผู้สอน',
							role: eligible?.role ?? (index === 0 ? 'primary' : 'secondary'),
							orderIndex: eligible?.orderIndex ?? index
						};
					}),
					roomId: dragSource.candidate.roomId,
					roomCode: room?.code ?? null,
					rowVersion: 0,
					isActive: true,
					syncStatus: null
				}
			],
			homerooms: [],
			teachers: []
		};
	}

	const source = dragSource.source;
	const demand = workspace.synchronizedDemands.find(
		(item) => item.learningOfferingId === source.learningOfferingId
	);
	if (!demand) throw new Error('ไม่พบกิจกรรมพร้อมกันที่กำลังจัดตาราง');
	const homeroomIds = dragSource.candidate.homeroomIds ?? [];
	const teacherIds = dragSource.candidate.teacherIds ?? [];
	return {
		...base,
		offeringCode: demand.offeringCode,
		offeringName: demand.offeringName,
		schedulingMode: 'synchronized',
		groups: [],
		homerooms: homeroomIds.flatMap((homeroomId, index) => {
			const homeroom = workspace.homerooms.find((item) => item.id === homeroomId);
			return homeroom
				? [
						{
							id: `${id}:homeroom:${index}`,
							homeroomId,
							code: homeroom.code,
							name: homeroom.name,
							roomId: dragSource.candidate.roomId,
							roomCode: room?.code ?? null,
							rowVersion: 0,
							isActive: true
						}
					]
				: [];
		}),
		teachers: teacherIds.map((teacherId, index) => ({
			id: `${id}:teacher:${index}`,
			teacherId,
			displayName:
				workspace.staff.find((teacher) => teacher.id === teacherId)?.displayName ?? 'ครูผู้สอน',
			rowVersion: 0,
			isActive: true
		}))
	};
}

export function removeTimetableTargetOptimistically(
	block: TimetableBlock,
	targetKind: TimetableTargetKind,
	targetId: string
): TimetableBlock {
	if (targetKind === 'group') {
		return { ...block, groups: block.groups.filter((group) => group.id !== targetId) };
	}
	if (targetKind === 'homeroom') {
		return {
			...block,
			homerooms: block.homerooms.filter((homeroom) => homeroom.id !== targetId)
		};
	}
	return { ...block, teachers: block.teachers.filter((teacher) => teacher.id !== targetId) };
}
