interface RequestOptions {
	signal?: AbortSignal;
}

function indexChildren<Parent extends { id: string }, Child>(
	parents: Parent[],
	children: Child[],
	parentId: (child: Child) => string
): Map<string, Child[]> {
	const indexed = new Map(parents.map((parent) => [parent.id, [] as Child[]]));
	for (const child of children) {
		const id = parentId(child);
		const siblings = indexed.get(id);
		if (siblings) siblings.push(child);
		else indexed.set(id, [child]);
	}
	return indexed;
}

export async function loadTimetableCollections<
	Offering extends { id: string },
	Group extends { learningOfferingId: string },
	Homeroom
>(
	deps: {
		listLearningOfferings: (termId: string, options?: RequestOptions) => Promise<Offering[]>;
		listLearningGroupsForTerm: (termId: string, options?: RequestOptions) => Promise<Group[]>;
		listHomerooms: (yearId: string, options?: RequestOptions) => Promise<Homeroom[]>;
	},
	termId: string,
	yearId: string,
	signal: AbortSignal
) {
	const options = { signal };
	const [offerings, groups, homerooms] = await Promise.all([
		deps.listLearningOfferings(termId, options),
		deps.listLearningGroupsForTerm(termId, options),
		deps.listHomerooms(yearId, options)
	]);

	return {
		offerings,
		groups,
		homerooms,
		groupsByOfferingId: indexChildren(offerings, groups, (group) => group.learningOfferingId)
	};
}

export async function loadStudentYearCollections<
	StudentYear extends { id: string },
	Placement extends { studentAcademicYearId: string },
	Homeroom,
	Student,
	GradeLevel,
	StudyProgram
>(
	deps: {
		listStudentAcademicYears: (yearId: string, options?: RequestOptions) => Promise<StudentYear[]>;
		listPlacementsForAcademicYear: (
			yearId: string,
			options?: RequestOptions
		) => Promise<Placement[]>;
		listHomerooms: (yearId: string, options?: RequestOptions) => Promise<Homeroom[]>;
		listStudentOptions: (search: string, options?: RequestOptions) => Promise<Student[]>;
		listGradeLevelOptions: (yearId: string, options?: RequestOptions) => Promise<GradeLevel[]>;
		listStudyProgramOptionsForAcademicYear: (
			yearId: string,
			options?: RequestOptions
		) => Promise<StudyProgram[]>;
	},
	yearId: string,
	signal: AbortSignal
) {
	const options = { signal };
	const [studentYears, placements, homerooms, students, gradeLevels, programs] = await Promise.all([
		deps.listStudentAcademicYears(yearId, options),
		deps.listPlacementsForAcademicYear(yearId, options),
		deps.listHomerooms(yearId, options),
		deps.listStudentOptions('', options),
		deps.listGradeLevelOptions(yearId, options),
		deps.listStudyProgramOptionsForAcademicYear(yearId, options)
	]);

	return {
		studentYears,
		placements,
		homerooms,
		students,
		gradeLevels,
		programs,
		placementsByStudentYearId: indexChildren(
			studentYears,
			placements,
			(placement) => placement.studentAcademicYearId
		)
	};
}

export async function loadHomeroomCollections<
	Homeroom extends { id: string },
	Advisor extends { homeroomId: string },
	GradeLevel,
	StudyProgram,
	Staff
>(
	deps: {
		listHomerooms: (yearId: string, options?: RequestOptions) => Promise<Homeroom[]>;
		listHomeroomAdvisorsForAcademicYear: (
			yearId: string,
			options?: RequestOptions
		) => Promise<Advisor[]>;
		listGradeLevelOptions: (yearId: string, options?: RequestOptions) => Promise<GradeLevel[]>;
		listStudyProgramOptionsForAcademicYear: (
			yearId: string,
			options?: RequestOptions
		) => Promise<StudyProgram[]>;
		listStaffOptions: (options?: RequestOptions) => Promise<Staff[]>;
	},
	yearId: string,
	signal: AbortSignal
) {
	const options = { signal };
	const [homerooms, advisors, gradeLevels, programs, staff] = await Promise.all([
		deps.listHomerooms(yearId, options),
		deps.listHomeroomAdvisorsForAcademicYear(yearId, options),
		deps.listGradeLevelOptions(yearId, options),
		deps.listStudyProgramOptionsForAcademicYear(yearId, options),
		deps.listStaffOptions(options)
	]);

	return {
		homerooms,
		advisors,
		gradeLevels,
		programs,
		staff,
		advisorsByHomeroomId: indexChildren(homerooms, advisors, (advisor) => advisor.homeroomId)
	};
}
