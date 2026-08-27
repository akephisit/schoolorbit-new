export interface AcademicPrerequisite {
	key: string;
	status: 'missing' | 'warning';
	title: string;
	description: string;
	actionLabel?: string;
	href?: string;
}
