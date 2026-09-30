export function menuPreloadPolicy(item: { path: string }): 'hover' | 'tap' | 'off' {
	const path = item.path.split('?')[0];
	if (
		path === '/staff/academic/exam-schedules' ||
		path === '/staff/exams' ||
		path === '/staff/roles' ||
		path === '/staff/students' ||
		path === '/staff/manage' ||
		path === '/staff/academic/supervision' ||
		path === '/staff/academic/supervision/requests' ||
		path === '/staff/academic/supervision/evaluate' ||
		path === '/staff/academic/supervision/approvals' ||
		path === '/staff/academic/supervision/cycles' ||
		path === '/staff/academic/supervision/templates'
	)
		return 'off';
	return path.startsWith('/staff/achievements') ||
		path.startsWith('/staff/certificates') ||
		path.startsWith('/staff/certificate-requests') ||
		path === '/student' ||
		path === '/student/profile' ||
		path === '/student/certificates' ||
		path === '/staff/facility/buildings' ||
		path === '/staff/features' ||
		path === '/staff/school-settings' ||
		path === '/staff/school-fonts' ||
		path === '/staff/calendar' ||
		path === '/staff/work' ||
		path === '/staff/work/manage' ||
		path === '/staff/profile' ||
		path === '/staff/organization' ||
		path === '/staff/academic/supervision/overview' ||
		path === '/staff/academic/timetable' ||
		path === '/staff/academic/timetable/today' ||
		path === '/staff/academic/assessments' ||
		path === '/staff/academic/gradebook' ||
		path === '/staff/academic/promotion' ||
		path === '/staff/academic/promotion/policies' ||
		path === '/staff/academic/term-lifecycle' ||
		path === '/staff/academic/year-lifecycle' ||
		path === '/staff/academic/results' ||
		path === '/staff/academic/results/aggregates' ||
		path === '/staff/academic/results/annual' ||
		path === '/staff/academic/result-locks' ||
		path === '/staff/academic/result-corrections' ||
		path === '/staff/academic/question-bank'
		? 'tap'
		: 'hover';
}
