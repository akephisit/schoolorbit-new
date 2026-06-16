type AcademicYearLike = {
	id: string;
	year?: number | null;
	name?: string | null;
};

type GradeLevelLike = {
	id: string;
	short_name?: string | null;
	name?: string | null;
};

type StudyPlanVersionLike = {
	id: string;
	study_plan_id: string;
	study_plan_name_th?: string | null;
	version_name: string;
	start_academic_year_id: string;
	end_academic_year_id?: string | null;
	is_active?: boolean | null;
};

type StudyPlanSubjectLike = {
	id: string;
	grade_level_id: string;
	term: string;
	subject_code?: string | null;
	subject_name_th?: string | null;
	subject_name_en?: string | null;
	subject_type?: string | null;
	subject_credit?: number | null;
	subject_hours?: number | null;
};

type StudyPlanActivityLike = {
	id: string;
	grade_level_id: string;
	term?: string | null;
	catalog_name?: string | null;
	catalog_activity_type?: string | null;
	catalog_periods_per_week?: number | null;
	catalog_scheduling_mode?: string | null;
};

type SemesterLike = {
	id: string;
	term: string;
	name?: string | null;
};

type ClassroomLike = {
	id: string;
	name: string;
	grade_level_name?: string | null;
};

type ClassroomCourseLike = {
	id: string;
	classroom_id: string;
	academic_semester_id: string;
	subject_code?: string | null;
	subject_name_th?: string | null;
	subject_name_en?: string | null;
	subject_type?: string | null;
	subject_credit?: number | null;
	subject_hours?: number | null;
	instructor_name?: string | null;
};

type ClassroomActivityLike = {
	slot_id: string;
	classroom_id: string;
	semester_id: string;
	name: string;
	activity_type: string;
	periods_per_week: number;
	scheduling_mode: string;
};

type CourseInstructorLike = {
	classroom_course_id: string;
	role: 'primary' | 'secondary';
	instructor_name?: string | null;
};

type WorksheetReportCell = string | number;
type WorksheetReportRow = WorksheetReportCell[];

export type PlannedCurriculumExportRow = {
	studyPlan: string;
	version: string;
	startYear: string;
	endYear: string;
	gradeLevel: string;
	term: string;
	itemKind: 'รายวิชา' | 'กิจกรรม';
	code: string;
	name: string;
	itemType: string;
	credits: number | '';
	hours: number | '';
	periodsPerWeek: number | '';
	schedulingMode: string;
};

export type PlannedCurriculumSummaryRow = {
	studyPlan: string;
	version: string;
	startYear: string;
	endYear: string;
	subjectCount: number;
	activityCount: number;
};

export type ActualCurriculumExportRow = {
	academicYear: string;
	term: string;
	semester: string;
	classroom: string;
	gradeLevel: string;
	itemKind: 'รายวิชา' | 'กิจกรรม';
	code: string;
	name: string;
	itemType: string;
	credits: number | '';
	hours: number | '';
	periodsPerWeek: number | '';
	instructor: string;
	schedulingMode: string;
};

export type ActualCurriculumSummaryRow = {
	academicYear: string;
	term: string;
	classroom: string;
	gradeLevel: string;
	courseCount: number;
	activityCount: number;
};

export type ActualSubjectActivityRow = {
	academicYear: string;
	term: string;
	itemKind: 'รายวิชา' | 'กิจกรรม';
	codeOrActivityType: string;
	name: string;
	itemType: string;
	credits: number | '';
	hours: number | '';
	periodsPerWeek: number | '';
	classroomCount: number;
	classrooms: string;
	instructors: string;
	classroomDetails: string;
};

function academicYearNumber(years: AcademicYearLike[], id: string | null | undefined) {
	return years.find((year) => year.id === id)?.year ?? null;
}

function academicYearName(years: AcademicYearLike[], id: string | null | undefined) {
	if (!id) return '';
	const year = years.find((item) => item.id === id);
	return year?.name ?? (year?.year ? String(year.year) : '');
}

function gradeLevelName(levels: GradeLevelLike[], id: string | null | undefined) {
	if (!id) return '';
	const level = levels.find((item) => item.id === id);
	return level?.short_name ?? level?.name ?? '';
}

function uniqueSorted(values: string[]) {
	return [...new Set(values.filter(Boolean))].sort((a, b) => a.localeCompare(b, 'th'));
}

function uniqueInOrder(values: string[]) {
	const seen = new Set<string>();
	return values.filter((value) => {
		if (!value || seen.has(value)) return false;
		seen.add(value);
		return true;
	});
}

function roleLabel(role: string) {
	return role === 'primary' ? 'หลัก' : 'ร่วม';
}

function termReportLabel(term: string) {
	return term ? `ภาคเรียนที่ ${term}` : 'ไม่ระบุภาคเรียน';
}

function classroomDetailRows(detail: string): WorksheetReportRow[] {
	return detail
		.split('\n')
		.map((line) => line.trim())
		.filter(Boolean)
		.map((line) => {
			const separatorIndex = line.indexOf(':');
			if (separatorIndex === -1) return [line];
			return [line.slice(0, separatorIndex).trim(), line.slice(separatorIndex + 1).trim()];
		});
}

function courseInstructorDetails(
	course: ClassroomCourseLike,
	courseInstructorsByCourseId: Record<string, CourseInstructorLike[]> | undefined
) {
	const instructors = courseInstructorsByCourseId?.[course.id] ?? [];
	if (instructors.length === 0) {
		return {
			names: course.instructor_name ? [course.instructor_name] : [],
			detail: course.instructor_name ?? ''
		};
	}

	const details = instructors
		.map((instructor) =>
			instructor.instructor_name
				? `${instructor.instructor_name} (${roleLabel(instructor.role)})`
				: ''
		)
		.filter(Boolean);

	return {
		names: uniqueInOrder(instructors.map((instructor) => instructor.instructor_name ?? '')),
		detail: details.join(', ')
	};
}

export function filterEffectiveStudyPlanVersions<T extends StudyPlanVersionLike>(
	versions: T[],
	years: AcademicYearLike[],
	selectedYearId: string
): T[] {
	const selectedYear = academicYearNumber(years, selectedYearId);
	if (selectedYear === null) return [];

	return versions.filter((version) => {
		if (version.is_active === false) return false;

		const startYear = academicYearNumber(years, version.start_academic_year_id);
		if (startYear === null || startYear > selectedYear) return false;

		const endYear = academicYearNumber(years, version.end_academic_year_id);
		return endYear === null || endYear >= selectedYear;
	});
}

export function buildEffectiveStudyPlanRows(input: {
	version: StudyPlanVersionLike;
	academicYears: AcademicYearLike[];
	gradeLevels: GradeLevelLike[];
	subjects: StudyPlanSubjectLike[];
	activities: StudyPlanActivityLike[];
}): PlannedCurriculumExportRow[] {
	const base = {
		studyPlan: input.version.study_plan_name_th ?? '',
		version: input.version.version_name,
		startYear: academicYearName(input.academicYears, input.version.start_academic_year_id),
		endYear: academicYearName(input.academicYears, input.version.end_academic_year_id)
	};

	const subjectRows: PlannedCurriculumExportRow[] = input.subjects.map((subject) => ({
		...base,
		gradeLevel: gradeLevelName(input.gradeLevels, subject.grade_level_id),
		term: subject.term,
		itemKind: 'รายวิชา',
		code: subject.subject_code ?? '',
		name: subject.subject_name_th ?? subject.subject_name_en ?? '',
		itemType: subject.subject_type ?? '',
		credits: subject.subject_credit ?? '',
		hours: subject.subject_hours ?? '',
		periodsPerWeek: '',
		schedulingMode: ''
	}));

	const activityRows: PlannedCurriculumExportRow[] = input.activities.map((activity) => ({
		...base,
		gradeLevel: gradeLevelName(input.gradeLevels, activity.grade_level_id),
		term: activity.term ?? 'ทุกเทอม',
		itemKind: 'กิจกรรม',
		code: '',
		name: activity.catalog_name ?? '',
		itemType: activity.catalog_activity_type ?? '',
		credits: '',
		hours: '',
		periodsPerWeek: activity.catalog_periods_per_week ?? '',
		schedulingMode: activity.catalog_scheduling_mode ?? ''
	}));

	return [...subjectRows, ...activityRows];
}

export function buildPlannedCurriculumSummaryRows(input: {
	rowsByVersion: Array<{
		version: StudyPlanVersionLike;
		academicYears: AcademicYearLike[];
		subjectCount: number;
		activityCount: number;
	}>;
}): PlannedCurriculumSummaryRow[] {
	return input.rowsByVersion.map((item) => ({
		studyPlan: item.version.study_plan_name_th ?? '',
		version: item.version.version_name,
		startYear: academicYearName(item.academicYears, item.version.start_academic_year_id),
		endYear: academicYearName(item.academicYears, item.version.end_academic_year_id),
		subjectCount: item.subjectCount,
		activityCount: item.activityCount
	}));
}

export function buildActualCurriculumRows(input: {
	yearName: string;
	semesters: SemesterLike[];
	classrooms: ClassroomLike[];
	courses: ClassroomCourseLike[];
	activities: ClassroomActivityLike[];
}): ActualCurriculumExportRow[] {
	const semesterById = new Map(input.semesters.map((semester) => [semester.id, semester]));
	const classroomById = new Map(input.classrooms.map((classroom) => [classroom.id, classroom]));

	const courseRows: ActualCurriculumExportRow[] = input.courses.map((course) => {
		const semester = semesterById.get(course.academic_semester_id);
		const classroom = classroomById.get(course.classroom_id);
		return {
			academicYear: input.yearName,
			term: semester?.term ?? '',
			semester: semester?.name ?? '',
			classroom: classroom?.name ?? '',
			gradeLevel: classroom?.grade_level_name ?? '',
			itemKind: 'รายวิชา',
			code: course.subject_code ?? '',
			name: course.subject_name_th ?? course.subject_name_en ?? '',
			itemType: course.subject_type ?? '',
			credits: course.subject_credit ?? '',
			hours: course.subject_hours ?? '',
			periodsPerWeek: '',
			instructor: course.instructor_name ?? '',
			schedulingMode: ''
		};
	});

	const activityRows: ActualCurriculumExportRow[] = input.activities.map((activity) => {
		const semester = semesterById.get(activity.semester_id);
		const classroom = classroomById.get(activity.classroom_id);
		return {
			academicYear: input.yearName,
			term: semester?.term ?? '',
			semester: semester?.name ?? '',
			classroom: classroom?.name ?? '',
			gradeLevel: classroom?.grade_level_name ?? '',
			itemKind: 'กิจกรรม',
			code: '',
			name: activity.name,
			itemType: activity.activity_type,
			credits: '',
			hours: '',
			periodsPerWeek: activity.periods_per_week,
			instructor: '',
			schedulingMode: activity.scheduling_mode
		};
	});

	return [...courseRows, ...activityRows].sort((a, b) => {
		const classroomCompare = a.classroom.localeCompare(b.classroom, 'th');
		if (classroomCompare !== 0) return classroomCompare;
		const termCompare = a.term.localeCompare(b.term, 'th');
		if (termCompare !== 0) return termCompare;
		return a.name.localeCompare(b.name, 'th');
	});
}

export function summarizeActualCurriculum(
	rows: ActualCurriculumExportRow[]
): ActualCurriculumSummaryRow[] {
	const summary = new Map<string, ActualCurriculumSummaryRow>();

	for (const row of rows) {
		const key = `${row.academicYear}\u0000${row.term}\u0000${row.classroom}`;
		const current = summary.get(key) ?? {
			academicYear: row.academicYear,
			term: row.term,
			classroom: row.classroom,
			gradeLevel: row.gradeLevel,
			courseCount: 0,
			activityCount: 0
		};

		if (row.itemKind === 'รายวิชา') current.courseCount += 1;
		else current.activityCount += 1;
		summary.set(key, current);
	}

	return [...summary.values()].sort((a, b) => {
		const classroomCompare = a.classroom.localeCompare(b.classroom, 'th');
		if (classroomCompare !== 0) return classroomCompare;
		return a.term.localeCompare(b.term, 'th');
	});
}

export function buildActualSubjectActivityRows(input: {
	yearName: string;
	semesters: SemesterLike[];
	classrooms: ClassroomLike[];
	courses: ClassroomCourseLike[];
	activities: ClassroomActivityLike[];
	courseInstructorsByCourseId?: Record<string, CourseInstructorLike[]>;
}): ActualSubjectActivityRow[] {
	const semesterById = new Map(input.semesters.map((semester) => [semester.id, semester]));
	const classroomById = new Map(input.classrooms.map((classroom) => [classroom.id, classroom]));
	const grouped = new Map<
		string,
		{
			rowBase: Omit<
				ActualSubjectActivityRow,
				'classroomCount' | 'classrooms' | 'instructors' | 'classroomDetails'
			>;
			classrooms: string[];
			instructors: string[];
			classroomDetails: string[];
		}
	>();

	for (const course of input.courses) {
		const semester = semesterById.get(course.academic_semester_id);
		const classroom = classroomById.get(course.classroom_id);
		const term = semester?.term ?? '';
		const code = course.subject_code ?? '';
		const key = `course\u0000${term}\u0000${code}`;
		const instructorDetails = courseInstructorDetails(course, input.courseInstructorsByCourseId);
		const current = grouped.get(key) ?? {
			rowBase: {
				academicYear: input.yearName,
				term,
				itemKind: 'รายวิชา',
				codeOrActivityType: code,
				name: course.subject_name_th ?? course.subject_name_en ?? '',
				itemType: course.subject_type ?? '',
				credits: course.subject_credit ?? '',
				hours: course.subject_hours ?? '',
				periodsPerWeek: ''
			},
			classrooms: [],
			instructors: [],
			classroomDetails: []
		};

		const classroomName = classroom?.name ?? '';
		if (classroomName) current.classrooms.push(classroomName);
		current.instructors.push(...instructorDetails.names);
		current.classroomDetails.push(
			instructorDetails.detail ? `${classroomName}: ${instructorDetails.detail}` : classroomName
		);
		grouped.set(key, current);
	}

	for (const activity of input.activities) {
		const semester = semesterById.get(activity.semester_id);
		const classroom = classroomById.get(activity.classroom_id);
		const term = semester?.term ?? '';
		const key = `activity\u0000${term}\u0000${activity.activity_type}\u0000${activity.name}`;
		const current = grouped.get(key) ?? {
			rowBase: {
				academicYear: input.yearName,
				term,
				itemKind: 'กิจกรรม',
				codeOrActivityType: activity.activity_type,
				name: activity.name,
				itemType: activity.activity_type,
				credits: '',
				hours: '',
				periodsPerWeek: activity.periods_per_week
			},
			classrooms: [],
			instructors: [],
			classroomDetails: []
		};

		const classroomName = classroom?.name ?? '';
		if (classroomName) current.classrooms.push(classroomName);
		current.classroomDetails.push(classroomName);
		grouped.set(key, current);
	}

	return [...grouped.values()]
		.map((item) => {
			const classrooms = uniqueSorted(item.classrooms);
			return {
				...item.rowBase,
				classroomCount: classrooms.length,
				classrooms: classrooms.join(', '),
				instructors: uniqueInOrder(item.instructors).join(', '),
				classroomDetails: uniqueSorted(item.classroomDetails).join('\n')
			};
		})
		.sort((a, b) => {
			const termCompare = a.term.localeCompare(b.term, 'th');
			if (termCompare !== 0) return termCompare;
			const kindOrder = { รายวิชา: 0, กิจกรรม: 1 };
			const kindCompare = kindOrder[a.itemKind] - kindOrder[b.itemKind];
			if (kindCompare !== 0) return kindCompare;
			return a.name.localeCompare(b.name, 'th');
		});
}

export function plannedRowsForWorksheet(rows: PlannedCurriculumExportRow[]) {
	return rows.map((row) => ({
		แผนการเรียน: row.studyPlan,
		เวอร์ชัน: row.version,
		ปีเริ่มต้น: row.startYear,
		ปีสิ้นสุด: row.endYear,
		ระดับชั้น: row.gradeLevel,
		ภาคเรียน: row.term,
		ประเภทข้อมูล: row.itemKind,
		รหัส: row.code,
		ชื่อ: row.name,
		ประเภท: row.itemType,
		หน่วยกิต: row.credits,
		ชั่วโมงต่อเทอม: row.hours,
		คาบต่อสัปดาห์: row.periodsPerWeek,
		รูปแบบการจัด: row.schedulingMode
	}));
}

export function plannedSummaryForWorksheet(rows: PlannedCurriculumSummaryRow[]) {
	return rows.map((row) => ({
		แผนการเรียน: row.studyPlan,
		เวอร์ชัน: row.version,
		ปีเริ่มต้น: row.startYear,
		ปีสิ้นสุด: row.endYear,
		จำนวนรายวิชา: row.subjectCount,
		จำนวนกิจกรรม: row.activityCount
	}));
}

export function actualRowsForWorksheet(rows: ActualCurriculumExportRow[]) {
	return rows.map((row) => ({
		ปีการศึกษา: row.academicYear,
		ภาคเรียน: row.term,
		ชื่อภาคเรียน: row.semester,
		ห้องเรียน: row.classroom,
		ระดับชั้น: row.gradeLevel,
		ประเภทข้อมูล: row.itemKind,
		รหัส: row.code,
		ชื่อ: row.name,
		ประเภท: row.itemType,
		หน่วยกิต: row.credits,
		ชั่วโมงต่อเทอม: row.hours,
		คาบต่อสัปดาห์: row.periodsPerWeek,
		ครูผู้สอน: row.instructor,
		รูปแบบการจัด: row.schedulingMode
	}));
}

export function actualSummaryForWorksheet(rows: ActualCurriculumSummaryRow[]) {
	return rows.map((row) => ({
		ปีการศึกษา: row.academicYear,
		ภาคเรียน: row.term,
		ห้องเรียน: row.classroom,
		ระดับชั้น: row.gradeLevel,
		จำนวนรายวิชา: row.courseCount,
		จำนวนกิจกรรม: row.activityCount
	}));
}

export function actualSubjectActivityForWorksheet(rows: ActualSubjectActivityRow[]) {
	return rows.map((row) => ({
		ปีการศึกษา: row.academicYear,
		ภาคเรียน: row.term,
		ประเภทข้อมูล: row.itemKind,
		'รหัส/ประเภทกิจกรรม': row.codeOrActivityType,
		ชื่อ: row.name,
		'ประเภทวิชา/กิจกรรม': row.itemType,
		หน่วยกิต: row.credits,
		ชั่วโมงต่อเทอม: row.hours,
		คาบต่อสัปดาห์: row.periodsPerWeek,
		จำนวนห้อง: row.classroomCount,
		'ห้องที่เรียน/เข้าร่วม': row.classrooms,
		ครูผู้สอนทั้งหมด: row.instructors,
		'รายละเอียดห้อง/ครู': row.classroomDetails
	}));
}

export function actualSubjectActivityReportForWorksheet(
	rows: ActualSubjectActivityRow[]
): WorksheetReportRow[] {
	if (rows.length === 0) return [['ไม่มีข้อมูลรายวิชาหรือกิจกรรม']];

	const report: WorksheetReportRow[] = [[rows[0].academicYear], []];
	const terms = uniqueInOrder(rows.map((row) => row.term));

	for (const term of terms) {
		if (report.length > 2) report.push([]);
		report.push([termReportLabel(term)]);

		const courseRows = rows.filter((row) => row.term === term && row.itemKind === 'รายวิชา');
		const activityRows = rows.filter((row) => row.term === term && row.itemKind === 'กิจกรรม');

		if (courseRows.length > 0) {
			report.push(['รายวิชา']);
			report.push([
				'รหัส/ชื่อวิชา',
				'ประเภท',
				'หน่วยกิต',
				'ชั่วโมงต่อเทอม',
				'จำนวนห้อง',
				'ครูผู้สอนทั้งหมด'
			]);

			for (const [index, row] of courseRows.entries()) {
				report.push([
					[row.codeOrActivityType, row.name].filter(Boolean).join(' '),
					row.itemType,
					row.credits,
					row.hours,
					row.classroomCount,
					row.instructors
				]);
				if (row.classrooms) report.push(['เรียนทั้งหมด', row.classrooms]);

				const detailRows = classroomDetailRows(row.classroomDetails);
				if (detailRows.length > 0) {
					report.push(['ห้องเรียน', 'ครูผู้สอน']);
					report.push(...detailRows);
				}

				if (index < courseRows.length - 1) report.push([]);
			}
		}

		if (activityRows.length > 0) {
			if (courseRows.length > 0) report.push([]);
			report.push(['กิจกรรม']);
			report.push(['ชื่อกิจกรรม', 'ประเภทกิจกรรม', 'คาบต่อสัปดาห์', 'จำนวนห้อง']);

			for (const [index, row] of activityRows.entries()) {
				report.push([
					row.name,
					row.itemType || row.codeOrActivityType,
					row.periodsPerWeek,
					row.classroomCount
				]);
				if (row.classrooms) report.push(['เข้าร่วมทั้งหมด', row.classrooms]);

				const detailRows = classroomDetailRows(row.classroomDetails).map(([classroom]) => [
					classroom
				]);
				if (detailRows.length > 0) {
					report.push(['ห้องเรียน']);
					report.push(...detailRows);
				}

				if (index < activityRows.length - 1) report.push([]);
			}
		}
	}

	return report;
}
