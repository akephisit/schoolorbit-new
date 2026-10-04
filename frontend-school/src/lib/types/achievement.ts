import type { components } from '#lib/api/generated/school-api.js';

type Schemas = components['schemas'];

export type Achievement = Schemas['Achievement'];
export type CreateAchievementRequest = Schemas['CreateAchievementRequest'];
export type UpdateAchievementRequest = Schemas['UpdateAchievementRequest'];
export type AchievementListFilter = Schemas['AchievementListFilter'];
