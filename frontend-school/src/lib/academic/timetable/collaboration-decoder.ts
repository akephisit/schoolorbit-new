import type { DraftEventEnvelope } from '../../api/timetable.js';

const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu;
const dayPattern = /^(MON|TUE|WED|THU|FRI|SAT|SUN)$/u;
const collectionLimit = 100_000;
const maxEventBytes = 64 * 1024 * 1024;

/** Match the transport budget without allocating a second complete encoded snapshot. */
function withinEventBudget(value: string): boolean {
	if (value.length > maxEventBytes) return false;
	let bytes = 0;
	for (let index = 0; index < value.length; index++) {
		const code = value.charCodeAt(index);
		if (code < 0x80) bytes++;
		else if (code < 0x800) bytes += 2;
		else if (
			code >= 0xd800 &&
			code <= 0xdbff &&
			index + 1 < value.length &&
			value.charCodeAt(index + 1) >= 0xdc00 &&
			value.charCodeAt(index + 1) <= 0xdfff
		) {
			bytes += 4;
			index++;
		} else bytes += 3;
		if (bytes > maxEventBytes) return false;
	}
	return true;
}
type Validator = (value: unknown) => boolean;

function object(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}
function keys(value: Record<string, unknown>, allowed: string[]): boolean {
	return Object.keys(value).every((key) => allowed.includes(key));
}
function text(value: unknown): value is string {
	return typeof value === 'string';
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
	return typeof value === 'string' && Number.isFinite(Date.parse(value));
}
function source(value: unknown) {
	if (!object(value)) return false;
	switch (value.kind) {
		case 'existing_block':
			return (
				keys(value, ['kind', 'blockId', 'rowVersion']) &&
				uuid(value.blockId) &&
				integer(value.rowVersion)
			);
		case 'ordinary_demand':
			return (
				keys(value, ['kind', 'learningGroupId', 'learningOfferingId']) &&
				uuid(value.learningGroupId) &&
				uuid(value.learningOfferingId)
			);
		case 'synchronized_offering':
			return keys(value, ['kind', 'learningOfferingId']) && uuid(value.learningOfferingId);
		default:
			return false;
	}
}
function context(value: unknown) {
	return (
		object(value) &&
		keys(value, ['viewMode', 'viewId', 'dayOfWeek']) &&
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
			return (
				keys(value, ['kind', 'dayOfWeek', 'bellSchedulePeriodId', 'u', 'v']) &&
				day(value.dayOfWeek) &&
				uuid(value.bellSchedulePeriodId)
			);
		case 'block':
			return keys(value, ['kind', 'blockId', 'u', 'v']) && uuid(value.blockId);
		case 'tray':
			return keys(value, ['kind', 'source', 'u', 'v']) && source(value.source);
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
		keys(value, ['userId', 'name', 'color', 'profileImageFileId', 'connections']) &&
		/^#[0-9a-f]{6}$/iu.test(String(value.color)) &&
		optional(value.profileImageFileId, uuid) &&
		array(
			value.connections,
			(connection) =>
				fields(connection, ['connectionId']) &&
				keys(connection, ['connectionId', 'context', 'anchor', 'activeReservationId']) &&
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
		keys(value, [
			'reservationId',
			'ownerConnectionId',
			'purpose',
			'source',
			'target',
			'expiresAt'
		]) &&
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
			['id', 'academicTermId', 'academicYearId', 'bellScheduleId', 'deliveryVersionId'],
			[],
			['rowVersion']
		) &&
		['draft', 'published', 'cancelled'].includes(String(value.status)) &&
		(value.createdBy === null || uuid(value.createdBy)) &&
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
function uniqueIds(value: unknown): boolean {
	return array(value, uuid) && new Set(value).size === value.length;
}
function uniqueRows(value: unknown, validate: Validator, identity = 'id'): boolean {
	if (!array(value, validate)) return false;
	const ids = new Set<string>();
	return value.every((row) => {
		const id = object(row) ? row[identity] : undefined;
		if (!uuid(id) || ids.has(id)) return false;
		ids.add(id);
		return true;
	});
}
function bellPeriod(value: unknown) {
	return (
		fields(value, ['id', 'bellScheduleId'], ['startTime', 'endTime'], ['orderIndex']) &&
		typeof value.isActive === 'boolean' &&
		optional(value.name, text) &&
		optional(value.applicableDays, text)
	);
}
function homeroom(value: unknown) {
	return (
		fields(value, ['id', 'gradeLevelId'], ['code', 'name', 'gradeLevelType'], ['gradeLevelYear']) &&
		(value.roomNumber === null || text(value.roomNumber)) &&
		typeof value.isActive === 'boolean'
	);
}
function room(value: unknown) {
	return fields(value, ['id'], ['name', 'status']) && (value.code === null || text(value.code));
}
function subjectGroup(value: unknown) {
	return (
		fields(value, ['id'], ['name']) &&
		(value.displayOrder === null || Number.isSafeInteger(value.displayOrder))
	);
}
function staff(value: unknown) {
	return (
		fields(value, ['id'], ['displayName', 'status']) && array(value.subjectGroups, subjectGroup)
	);
}
function offeringSubjectGroup(value: unknown) {
	return fields(value, ['learningOfferingId']) && subjectGroup(value.subjectGroup);
}
function lookupChanges(value: unknown) {
	return (
		object(value) &&
		uniqueRows(value.upsertedStaff, staff) &&
		uniqueIds(value.removedStaffIds) &&
		optional(value.staffOrder, uniqueIds) &&
		uniqueRows(value.upsertedHomerooms, homeroom) &&
		uniqueIds(value.removedHomeroomIds) &&
		optional(value.homeroomOrder, uniqueIds) &&
		uniqueRows(value.upsertedRooms, room) &&
		uniqueIds(value.removedRoomIds) &&
		optional(value.roomOrder, uniqueIds) &&
		uniqueRows(value.upsertedBellPeriods, bellPeriod) &&
		uniqueIds(value.removedBellPeriodIds) &&
		optional(value.bellPeriodOrder, uniqueIds) &&
		uniqueRows(value.upsertedOfferingSubjectGroups, offeringSubjectGroup, 'learningOfferingId') &&
		uniqueIds(value.removedSubjectGroupOfferingIds) &&
		optional(value.subjectGroupOfferingOrder, uniqueIds)
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
		uniqueRows(value.bellPeriods, bellPeriod) &&
		uniqueRows(value.homerooms, homeroom) &&
		uniqueRows(value.rooms, room) &&
		uniqueRows(value.staff, staff) &&
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
		uniqueRows(value.offeringSubjectGroups, offeringSubjectGroup, 'learningOfferingId')
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
				optional(
					payload.roomOptions,
					(value) =>
						object(value) &&
						keys(value, ['reservationId', 'availableRoomIds']) &&
						uuid(value.reservationId) &&
						uniqueIds(value.availableRoomIds)
				) &&
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
				optional(payload.version, version) &&
				optional(payload.lookupChanges, lookupChanges)
			);
		case 'DraftReconcileRequired':
			return typeof payload.reason === 'string' && payload.reason.length > 0;
		default:
			return false;
	}
}

/** Decode into the generated contract after checking all identities used by DOM and mutations. */
export function decodeDraftEvent(data: unknown): DraftEventEnvelope | null {
	if (typeof data !== 'string' || !withinEventBudget(data)) return null;
	try {
		const parsed: unknown = JSON.parse(data);
		return envelope(parsed) ? parsed : null;
	} catch {
		return null;
	}
}
