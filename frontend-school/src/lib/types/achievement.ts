export interface Achievement {
	id: string;
	user_id: string;
	title: string;
	description?: string;
	achievement_date: string;
	image_path?: string;
	created_by?: string;
	created_at: string;
	updated_at: string;
	user_first_name?: string;
	user_last_name?: string;
	user_profile_image_url?: string;
}

export interface CreateAchievementRequest {
	user_id?: string;
	title: string;
	description?: string;
	achievement_date: string;
	image_path?: string;
}

export interface UpdateAchievementRequest {
	title?: string;
	description?: string;
	achievement_date?: string;
	image_path?: string;
}

export interface AchievementListFilter {
	user_id?: string;
	start_date?: string;
	end_date?: string;
}
