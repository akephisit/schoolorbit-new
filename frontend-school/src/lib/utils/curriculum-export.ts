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
