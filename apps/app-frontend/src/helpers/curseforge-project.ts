import type { Labrinth } from '@modrinth/api-client'

import { get_curseforge_project, get_curseforge_project_versions } from '@/helpers/cache.js'

export const CF_ID_PREFIX = 'cf-'

export interface CfSourceProject {
	source: 'curseforge'
	id: string
	slug: string | null
	title: string
	description: string
	bodyUrl: string
	iconUrl: string | null
	categories: string[]
	downloads: number
	updated: string | null
	dateCreated: string | null
	websiteUrl: string | null
	projectType: 'mod' | 'modpack' | 'resource_pack' | 'shader_pack'
}

export interface CfSourceVersionFile {
	id: string
	filename: string
	url: string
	primary: boolean
	size: number
	sha1: string | null
}

export interface CfSourceVersion {
	source: 'curseforge'
	id: string
	projectId: string
	versionNumber: string
	changelog: string | null
	gameVersions: string[]
	loaders: string[]
	datePublished: string | null
	versionType: string
	files: CfSourceVersionFile[]
	dependencies: string[]
}

const PROJECT_TYPE_MAP: Record<CfSourceProject['projectType'], string> = {
	mod: 'mod',
	modpack: 'modpack',
	resource_pack: 'resourcepack',
	shader_pack: 'shader',
}

export function isCfProjectId(id: string | null | undefined): boolean {
	return !!id && id.startsWith(CF_ID_PREFIX)
}

export function parseCfId(id: string): number {
	return Number(id.slice(CF_ID_PREFIX.length))
}

export function toCfProjectId(cfId: number | string): string {
	return `${CF_ID_PREFIX}${cfId}`
}

type CfProject = Labrinth.Projects.v2.Project & {
	body_url: string
	website_url: string
}

export function cfProjectToProject(cf: CfSourceProject): CfProject {
	const projectType = PROJECT_TYPE_MAP[cf.projectType] ?? 'mod'
	return {
		id: toCfProjectId(cf.id),
		slug: cf.slug ?? toCfProjectId(cf.id),
		project_type: projectType,
		team: '',
		organization: null,
		title: cf.title,
		description: cf.description,
		body: cf.description,
		body_url: cf.bodyUrl,
		website_url: cf.websiteUrl ?? '',
		icon_url: cf.iconUrl,
		raw_icon_url: cf.iconUrl,
		categories: cf.categories,
		additional_categories: [],
		loaders: [],
		game_versions: [],
		downloads: cf.downloads,
		follows: 0,
		updated: cf.updated ?? '',
		published: cf.dateCreated ?? '',
		status: 'approved',
		client_side: 'unknown',
		server_side: 'unknown',
		license: { id: '', name: '', url: null },
		issues_url: '',
		source_url: '',
		wiki_url: '',
		discord_url: '',
		donation_urls: [],
		gallery: [],
		color: null,
		versions: [],
	} as unknown as CfProject
}

type CfVersion = Labrinth.Versions.v2.Version

export function cfVersionToVersion(cf: CfSourceVersion): CfVersion {
	return {
		id: toCfProjectId(cf.id),
		project_id: toCfProjectId(cf.projectId),
		author_id: null,
		organization: null,
		name: cf.versionNumber,
		version_number: cf.versionNumber,
		changelog: cf.changelog ?? '',
		date_published: cf.datePublished ?? '',
		downloads: 0,
		version_type: cf.versionType,
		status: 'listed',
		requested_status: null,
		featured: cf.files.some((file) => file.primary),
		game_versions: cf.gameVersions,
		loaders: cf.loaders,
		dependencies: cf.dependencies.map((dependency) => ({
			version_id: null,
			project_id: toCfProjectId(dependency),
			file_name: null,
			dependency_type: 'optional',
		})),
		files: cf.files.map((file) => ({
			hashes: file.sha1 ? { sha1: file.sha1 } : {},
			url: file.url,
			filename: file.filename,
			primary: file.primary,
			size: file.size,
			file_type: null,
		})),
	} as unknown as CfVersion
}

export async function getCfProject(requestedId: string): Promise<CfProject | null> {
	const cf = await get_curseforge_project(parseCfId(requestedId))
	return cf ? cfProjectToProject(cf as CfSourceProject) : null
}

export async function getCfVersions(requestedId: string): Promise<CfVersion[]> {
	const cfVersions = await get_curseforge_project_versions(parseCfId(requestedId))
	return ((cfVersions ?? []) as CfSourceVersion[]).map(cfVersionToVersion)
}
