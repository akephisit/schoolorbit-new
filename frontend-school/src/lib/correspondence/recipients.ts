import type { CorrespondenceOptions, ProcessRecipient } from '#lib/api/correspondence.js';
export function recipientKey(item: ProcessRecipient): string {
	return `${item.kind}:${item.userId ?? ''}:${item.organizationUnitId ?? ''}:${item.positionCode ?? ''}`;
}
export function recipientLabel(item: ProcessRecipient, options: CorrespondenceOptions): string {
	if (item.kind === 'user')
		return options.users.find((user) => user.id === item.userId)?.displayName ?? 'บุคลากรที่เลือก';
	const unit =
		options.organizations.find((unit) => unit.id === item.organizationUnitId)?.name ??
		'หน่วยงานที่เลือก';
	return item.kind === 'organization_unit'
		? `หน่วยงาน ${unit}`
		: `${unit} · ${options.positions.find((position) => position.organizationUnitId === item.organizationUnitId && position.code === item.positionCode)?.name ?? 'ตำแหน่งที่เลือก'}`;
}
/** Presentation lookups only; group membership remains resolved by the server when sending. */
export function mergeRecipientLookups(
	current: CorrespondenceOptions,
	added: CorrespondenceOptions
): CorrespondenceOptions {
	function merge<T>(items: T[], incoming: T[], key: (item: T) => string): T[] {
		const result = new Map(items.map((item) => [key(item), item]));
		for (const item of incoming) result.set(key(item), item);
		return [...result.values()];
	}
	return {
		...current,
		users: merge(current.users, added.users, (item) => item.id),
		organizations: merge(current.organizations, added.organizations, (item) => item.id),
		positions: merge(
			current.positions,
			added.positions,
			(item) => `${item.organizationUnitId}:${item.code}`
		)
	};
}
