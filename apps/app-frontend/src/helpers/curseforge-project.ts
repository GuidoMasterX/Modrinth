import type { Labrinth } from '@modrinth/api-client'

import { get_curseforge_project, get_curseforge_project_versions } from '@/helpers/cache.js'

export const CF_ID_PREFIX = 'cf-'

export function isCfProjectId(id: string | null | undefined): boolean {
	return !!id && id.startsWith(CF_ID_PREFIX)
}

export function parseCfId(id: string): number {
	return Number(id.slice(CF_ID_PREFIX.length))
}

export function toCfProjectId(cfId: number | string): string {
	return `${CF_ID_PREFIX}${cfId}`
}

export async function getCfProject(
	requestedId: string,
): Promise<Labrinth.Projects.v2.Project | null> {
	return get_curseforge_project(requestedId.slice(CF_ID_PREFIX.length))
}

export async function getCfVersions(requestedId: string): Promise<Labrinth.Versions.v2.Version[]> {
	return (await get_curseforge_project_versions(requestedId.slice(CF_ID_PREFIX.length))) ?? []
}

export function cfProjectUrl(project: Pick<Labrinth.Projects.v2.Project, 'slug' | 'id'>): string {
	const slugOrId = project.slug ?? project.id.slice(CF_ID_PREFIX.length)
	return `https://www.curseforge.com/projects/${slugOrId}`
}
