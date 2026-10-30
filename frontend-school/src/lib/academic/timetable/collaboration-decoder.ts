import type { DraftEventEnvelope } from '../../api/timetable.js';

const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu;
const dayPattern = /^(MON|TUE|WED|THU|FRI|SAT|SUN)$/u;
const collectionLimit = 100_000;
const maxEventCharacters = 64 * 1024 * 1024;
type Validator = (value: unknown) => boolean;

function object(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}
function text(value: unknown): value is string {
	return typeof value === 'string' && value.length <= 16_384;
}
function uuid(value: unknown): value is string {
	return typeof value === 'string' && uuidPattern.test(value);
}
function integer(value: unknown): value is number {
	return Number.isSafeInteger(value) && Number(value) >= 0;
}
function day(value: unknown) {
	return typeof value === 'string' && dayPattern.test(value);
}
function optional(value: unknown, validate: Validator) {
	return value === undefined || value === null || validate(value);
}
function array(value: unknown, validate: Validator, limit = collectionLimit): value is unknown[] {
	return Array.isArray(value) && value.length <= limit && value.every(validate);
}
function fields(
	value: unknown,
	ids: string[] = [],
	strings: string[] = [],
	counts: string[] = []
): value is Record<string, unknown> {
	return (
		object(value) &&
		ids.every((key) => uuid(value[key])) &&
		strings.every((key) => text(value[key])) &&
		counts.every((key) => integer(value[key]))
	);
}
function instant(value: unknown) {
	return typeof value === 'string' && value.length <= 64 && Number.isFinite(Date.parse(value));
}
function source(value: unknown) {
	if (!object(value)) return false;
	switch (value.kind) {
		case 'existing_block':
			return uuid(value.blockId) && integer(value.rowVersion);
		case 'ordinary_demand':
			return uuid(value.learningGroupId) && uuid(value.learningOfferingId);
		case 'synchronized_offering':
			return uuid(value.learningOfferingId);
		default:
			return false;
	}
}
function context(value: unknown) {
	return (
		object(value) &&
		['homeroom', 'teacher', 'learning_group', 'wholeSchool'].includes(String(value.viewMode)) &&
		optional(value.viewId, uuid) &&
		optional(value.dayOfWeek, day)
	);
}
function anchor(value: unknown) {
	if (
		!object(value) ||
		typeof value.u !== 'number' ||
		typeof value.v !== 'number' ||
		!Number.isFinite(value.u) ||
		!Number.isFinite(value.v) ||
		value.u < 0 ||
		value.u > 1 ||
		value.v < 0 ||
		value.v > 1
	)
		return false;
	switch (value.kind) {
		case 'cell':
			return day(value.dayOfWeek) && uuid(value.bellSchedulePeriodId);
		case 'block':
			return uuid(value.blockId);
		case 'tray':
			return source(value.source);
		default:
			return false;
	}
}
function target(value: unknown) {
	return (
		object(value) &&
		day(value.dayOfWeek) &&
		uuid(value.bellSchedulePeriodId) &&
		optional(value.blockId, uuid)
	);
}
function purpose(value: unknown) {
	return value === 'drag' || value === 'edit';
}
function participant(value: unknown) {
	return (
		fields(value, ['userId'], ['name', 'color']) &&
		/^#[0-9a-f]{6}$/iu.test(String(value.color)) &&
		optional(value.profileImageFileId, uuid) &&
		array(
			value.connections,
			(connection) =>
				fields(connection, ['connectionId']) &&
				optional(connection.context, context) &&
				optional(connection.anchor, anchor) &&
				optional(connection.activeReservationId, uuid),
			256
		)
	);
}
function reservation(value: unknown) {
	return (
		fields(value, ['reservationId', 'ownerConnectionId']) &&
		purpose(value.purpose) &&
		source(value.source) &&
		optional(value.target, target) &&
		instant(value.expiresAt) &&
		!('leaseToken' in value) &&
		!('candidate' in value)
	);
}
function instructor(value: unknown) {
	return fields(value, ['teacherId'], ['displayName', 'role'], ['orderIndex']);
}
function ordinaryDemand(value: unknown) {
	return (
		fields(
			value,
			['learningGroupId', 'learningOfferingId'],
			['offeringCode', 'offeringName'],
			['requiredPeriods', 'scheduledPeriods', 'remainingPeriods']
		) &&
		array(value.homeroomIds, uuid) &&
		array(value.eligibleInstructors, instructor)
	);
}
function synchronizedDemand(value: unknown) {
	return (
		fields(
			value,
			['learningOfferingId'],
			['offeringName'],
			[
				'requiredPeriods',
				'scheduledPeriods',
				'linkedGroupCount',
				'pendingGroupCount',
				'conflictGroupCount',
				'excludedGroupCount'
			]
		) &&
		optional(value.offeringCode, text) &&
		array(value.intendedHomeroomIds, uuid)
	);
}
function summary(value: unknown) {
	return fields(
		value,
		[],
		[],
		[
			'blockCount',
			'ordinaryDemandCount',
			'synchronizedDemandCount',
			'linkedGroupCount',
			'waitingGroupCount',
			'conflictGroupCount',
			'excludedGroupCount'
		]
	);
}
function sourceIssue(value: unknown) {
	return (
		fields(value, ['blockId'], ['code']) &&
		optional(value.learningGroupId, uuid) &&
		optional(value.teacherId, uuid)
	);
}
function block(value: unknown) {
	return (
		fields(
			value,
			[
				'id',
				'academicTermId',
				'academicYearId',
				'bellScheduleId',
				'bellSchedulePeriodId',
				'timetableVersionId'
			],
			['periodName', 'startTime', 'endTime'],
			['rowVersion']
		) &&
		day(value.dayOfWeek) &&
		['course', 'activity', 'structural'].includes(String(value.blockKind)) &&
		typeof value.isActive === 'boolean' &&
		optional(value.learningOfferingId, uuid) &&
		optional(value.seriesId, uuid) &&
		['offeringCode', 'offeringName', 'title', 'note'].every((key) => optional(value[key], text)) &&
		instant(value.createdAt) &&
		instant(value.updatedAt) &&
		array(
			value.groups,
			(group) =>
				fields(
					group,
					['id', 'learningGroupId', 'learningOfferingId'],
					['code', 'name'],
					['rowVersion']
				) &&
				typeof group.isActive === 'boolean' &&
				array(group.homeroomIds, uuid) &&
				array(group.homeroomNames, text) &&
				array(group.instructors, instructor) &&
				optional(group.roomId, uuid) &&
				optional(group.roomName, text) &&
				optional(group.roomCode, text)
		) &&
		array(
			value.homerooms,
			(room) =>
				fields(room, ['id', 'homeroomId'], ['code', 'name'], ['rowVersion']) &&
				typeof room.isActive === 'boolean' &&
				optional(room.roomId, uuid) &&
				optional(room.roomCode, text) &&
				optional(room.roomName, text)
		) &&
		array(
			value.teachers,
			(teacher) =>
				fields(teacher, ['id', 'teacherId'], ['displayName'], ['rowVersion']) &&
				typeof teacher.isActive === 'boolean'
		) &&
		array(
			value.syncStates,
			(sync) =>
				fields(sync, ['id', 'learningGroupId', 'learningOfferingId'], ['status'], ['rowVersion']) &&
				optional(sync.linkedBlockGroupId, uuid)
		)
	);
}
function version(value: unknown) {
	return (
		fields(
			value,
			[
				'id',
				'academicTermId',
				'academicYearId',
				'bellScheduleId',
				'deliveryVersionId',
				'createdBy'
			],
			[],
			['rowVersion']
		) &&
		['draft', 'published', 'cancelled'].includes(String(value.status)) &&
		optional(value.sourceVersionId, uuid) &&
		optional(value.publishedBy, uuid) &&
		instant(value.createdAt) &&
		instant(value.updatedAt) &&
		optional(value.effectiveFrom, text) &&
		optional(value.effectiveUntil, text) &&
		optional(value.publishedAt, instant) &&
		array(value.targets, (entry) => object(entry))
	);
}
function workspace(value: unknown) {
	return (
		object(value) &&
		version(value.version) &&
		uuid(value.latestDeliveryVersionId) &&
		summary(value.summary) &&
		optional(value.totalDraftBlockCount, integer) &&
		array(value.blocks, block) &&
		array(value.ordinaryDemands, ordinaryDemand) &&
		array(value.synchronizedDemands, synchronizedDemand) &&
		array(value.sourceIssues, sourceIssue) &&
		array(
			value.bellPeriods,
			(period) =>
				fields(period, ['id', 'bellScheduleId'], ['startTime', 'endTime'], ['orderIndex']) &&
				typeof period.isActive === 'boolean' &&
				optional(period.name, text) &&
				optional(period.applicableDays, text)
		) &&
		array(
			value.homerooms,
			(room) =>
				fields(
					room,
					['id', 'gradeLevelId'],
					['code', 'name', 'gradeLevelType', 'roomNumber'],
					['gradeLevelYear']
				) && typeof room.isActive === 'boolean'
		) &&
		array(value.rooms, (room) => fields(room, ['id'], ['code', 'name', 'status'])) &&
		array(
			value.staff,
			(staff) =>
				fields(staff, ['id'], ['displayName', 'status']) &&
				array(staff.subjectGroups, (group) => fields(group, ['id'], ['name']))
		) &&
		array(
			value.learningGroups,
			(group) =>
				fields(
					group,
					['id', 'learningOfferingId'],
					['code', 'name', 'status', 'rosterStatus', 'offeringKind', 'offeringName']
				) &&
				optional(group.offeringCode, text) &&
				array(group.homeroomIds, uuid) &&
				array(group.preferredRoomIds, uuid) &&
				array(group.eligibleInstructors, instructor)
		) &&
		array(
			value.offeringSubjectGroups,
			(entry) =>
				fields(entry, ['learningOfferingId']) && fields(entry.subjectGroup, ['id'], ['name'])
		)
	);
}
function envelope(value: unknown): value is DraftEventEnvelope {
	if (!fields(value, ['epoch'], ['type'], ['seq']) || !object(value.payload)) return false;
	const payload = value.payload;
	switch (value.type) {
		case 'DraftStateSync':
			return (
				uuid(payload.connectionId) &&
				workspace(payload.workspace) &&
				array(payload.users, participant, 10_000) &&
				array(payload.reservations, reservation, 10_000)
			);
		case 'DraftPresenceChanged':
			return participant(payload);
		case 'DraftPresenceLeft':
			return uuid(payload.connectionId);
		case 'DraftReservationsChanged':
			return array(payload.reservations, reservation, 10_000);
		case 'DraftAvailability':
			return (
				source(payload.source) &&
				optional(payload.context, context) &&
				array(
					payload.blockedCells,
					(cell) =>
						fields(cell, ['bellSchedulePeriodId', 'reservationId', 'ownerConnectionId']) &&
						day(cell.dayOfWeek) &&
						purpose(cell.purpose),
					100_000
				)
			);
		case 'DraftCommandResult':
			return (
				uuid(payload.requestId) &&
				['claim', 'target', 'renew', 'release'].includes(String(payload.command)) &&
				typeof payload.accepted === 'boolean' &&
				optional(payload.code, text) &&
				optional(payload.source, source) &&
				optional(payload.target, target) &&
				optional(payload.reservationId, uuid) &&
				optional(payload.leaseToken, uuid) &&
				optional(payload.expiresAt, instant)
			);
		case 'DraftPatched':
			return (
				uuid(payload.timetableVersionId) &&
				uuid(payload.mutationId) &&
				optional(payload.originConnectionId, uuid) &&
				array(payload.upsertedBlocks, block) &&
				array(payload.removedBlockIds, uuid) &&
				array(payload.ordinaryDemands, ordinaryDemand) &&
				array(payload.synchronizedDemands, synchronizedDemand) &&
				summary(payload.summary) &&
				array(payload.sourceIssues, sourceIssue) &&
				optional(payload.totalDraftBlockCount, integer) &&
				optional(payload.version, version)
			);
		case 'DraftReconcileRequired':
			return (
				typeof payload.reason === 'string' &&
				payload.reason.length > 0 &&
				payload.reason.length <= 96
			);
		default:
			return false;
	}
}

/** Decode into the generated contract after checking all identities used by DOM and mutations. */
export function decodeDraftEvent(data: unknown): DraftEventEnvelope | null {
	if (typeof data !== 'string' || data.length > maxEventCharacters) return null;
	try {
		const parsed: unknown = JSON.parse(data);
		return envelope(parsed) ? parsed : null;
	} catch {
		return null;
	}
}
