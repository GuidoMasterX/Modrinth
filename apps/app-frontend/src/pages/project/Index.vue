<template>
	<div v-if="data">
		<Teleport to="#sidebar-teleport-target">
			<ProjectSidebarCompatibility
				v-if="!isServerProject"
				:project="data"
				:tags="{ loaders: allLoaders, gameVersions: allGameVersions }"
				:project-v3="projectV3"
				class="project-sidebar-section"
			/>
			<ProjectSidebarServerInfo
				v-if="isServerProject"
				:project-v3="projectV3"
				:tags="{ loaders: allLoaders, gameVersions: allGameVersions }"
				:required-content="serverRequiredContent"
				:recommended-version="serverRecommendedVersion"
				:supported-versions="serverSupportedVersions"
				:loaders="serverModpackLoaders"
				:ping="serverPing"
				:status-online="serverStatusOnline"
				class="project-sidebar-section"
			/>
			<ProjectSidebarLinks
				link-target="_blank"
				:project="data"
				:project-v3="projectV3"
				class="project-sidebar-section"
			/>
			<ProjectSidebarDependencies
				v-if="!isServerProject"
				:dependencies="sidebarDependencies"
				class="project-sidebar-section"
			/>
			<ProjectSidebarModpacks
				v-if="!isServerProject && data.id"
				:project-id="data.id"
				class="project-sidebar-section"
			/>
			<ProjectSidebarRepository :source-url="data.source_url" class="project-sidebar-section" />
			<ProjectSidebarTags :project="data" class="project-sidebar-section" />
			<ProjectSidebarCreators
				:organization="organization"
				:members="members"
				:org-link="(slug) => `https://modrinth.com/organization/${slug}`"
				:user-link="getUserLink"
				link-target="_blank"
				:user-link-target="null"
				class="project-sidebar-section"
			/>
			<ProjectSidebarDetails
				:project="data"
				:has-versions="versions.length > 0"
				:link-target="`_blank`"
				:hide-license="isServerProject || isCfProjectId(data.id)"
				:show-followers="isServerProject"
				class="project-sidebar-section"
			/>
		</Teleport>
		<div class="flex flex-col gap-4 p-6">
			<div
				v-if="projectInstallContext"
				class="sticky top-0 z-20 -mx-6 -mt-6 rounded-tl-[--radius-xl] border-0 border-b border-solid bg-surface-1 px-3 py-4 border-surface-5"
			>
				<BrowseInstallHeader :install-context="projectInstallContext" />
			</div>
			<InstanceIndicator v-if="instance && !projectInstallContext" :instance="instance" />
			<template v-if="data">
				<Teleport
					v-if="appSettings.featureFlags.project_background"
					to="#background-teleport-target"
				>
					<ProjectBackgroundGradient :project="data" />
				</Teleport>
				<ProjectPageHeader
					v-else
					:project="data"
					:project-v3="projectV3"
					:show-status-badge="data.status !== 'approved'"
					:show-source-badge="true"
					@contextmenu.prevent.stop="handleRightClick"
					@category="(category) => router.push(`${projectSearchUrl}?f=categories:${category}`)"
				>
					<template #actions>
						<template v-if="isServerProject">
							<Button
								v-if="serverPlaying"
								type="colored"
								color="red"
								size="xl"
								native-type="button"
								@click="handleStopServer"
							>
								<StopCircleIcon />
								{{ formatMessage(commonMessages.stopButton) }}
							</Button>
							<Button
								v-else
								type="colored"
								color="brand"
								size="xl"
								native-type="button"
								:disabled="serverInstallLoading"
								@click="handleClickPlay"
							>
								<PlayIcon />
								{{
									serverInstallLoading
										? formatMessage(commonMessages.installingLabel)
										: formatMessage(commonMessages.playButton)
								}}
							</Button>
							<IconButton
								v-tooltip="formatMessage(commonMessages.addServerToInstanceButton)"
								size="xl"
								:label="formatMessage(commonMessages.addServerToInstanceButton)"
								native-type="button"
								@click="handleAddServerToInstance"
							>
								<PlusIcon />
							</IconButton>
							<TeleportOverflowMenu
								type="quiet"
								size="xl"
								:label="formatMessage(messages.moreOptions)"
								:options="serverProjectHeaderMoreActions"
							>
								<MoreVerticalIcon />
							</TeleportOverflowMenu>
						</template>
						<template v-else>
							<Button
								v-if="showSwitchVersion && onVersionsPage"
								v-tooltip="formatMessage(messages.alreadyInstalled)"
								size="xl"
								native-type="button"
								disabled
							>
								<CheckIcon />
								{{ formatMessage(commonMessages.installedLabel) }}
							</Button>
							<Button
								v-else-if="showSwitchVersion"
								size="xl"
								native-type="button"
								@click="goToVersions"
							>
								<ArrowLeftRightIcon />
								{{ formatMessage(messages.switchVersion) }}
							</Button>
							<Button
								v-else
								v-tooltip="
									installButtonInstalled ? formatMessage(messages.alreadyInstalled) : undefined
								"
								type="colored"
								color="brand"
								size="xl"
								native-type="button"
								:disabled="installButtonDisabled"
								@click="install(null)"
							>
								<component :is="installButtonIcon" :class="installButtonIconClass" />
								{{
									installButtonInstalled
										? formatMessage(commonMessages.installedLabel)
										: installButtonValidating
											? formatMessage(commonMessages.validatingLabel)
											: installButtonLoading
												? formatMessage(commonMessages.installingLabel)
												: formatMessage(commonMessages.installButton)
								}}
							</Button>
							<Button size="xl" native-type="button" @click="switchSource">
								<ArrowLeftRightIcon />
								{{ formatMessage(messages.switchSource) }}
							</Button>
							<TeleportOverflowMenu
								type="quiet"
								size="xl"
								:label="formatMessage(messages.moreOptions)"
								:options="projectHeaderMoreActions"
							>
								<MoreVerticalIcon />
							</TeleportOverflowMenu>
						</template>
					</template>
				</ProjectPageHeader>
				<NavTabs
					:links="[
						{
							label: formatMessage(messages.descriptionTab),
							href: projectDescriptionHref,
						},
						{
							label: formatMessage(messages.versionsTab),
							href: versionsHref,
							subpages: ['version'],
							shown: projectV3?.minecraft_server == null,
						},
						{
							label: formatMessage(messages.galleryTab),
							href: projectGalleryHref,
							shown: data.gallery.length > 0,
						},
					]"
				/>
				<RouterView
					v-if="route.path.startsWith('/project')"
					:project="data"
					:versions="versions"
					:members="members"
					:instance="instance"
					:install="install"
					:installed="installed"
					:installing="installing"
					:installed-version="installedVersion"
				/>
			</template>
			<template v-else>{{ formatMessage(messages.loadError) }}</template>
		</div>
		<SelectedProjectsFloatingBar
			v-if="projectInstallContext"
			:install-context="projectInstallContext"
		/>
		<ContextMenu ref="options" :label="formatMessage(messages.projectActionsLabel)">
			<template #open_link="{ option }">
				<GlobeIcon /> {{ option.label }} <ExternalIcon />
			</template>
		</ContextMenu>
	</div>
</template>

<script setup>
import {
	ArrowLeftRightIcon,
	BookmarkIcon,
	CheckIcon,
	ClipboardCopyIcon,
	DownloadIcon,
	ExternalIcon,
	GlobeIcon,
	HeartIcon,
	MoreVerticalIcon,
	PlayIcon,
	PlusIcon,
	ReportIcon,
	SpinnerIcon,
	StopCircleIcon,
} from '@modrinth/assets'
import {
	BrowseInstallHeader,
	Button,
	commonMessages,
	ContextMenu,
	defineMessages,
	IconButton,
	injectNotificationManager,
	NavTabs,
	ProjectBackgroundGradient,
	ProjectPageHeader,
	ProjectSidebarCompatibility,
	ProjectSidebarCreators,
	ProjectSidebarDependencies,
	ProjectSidebarDetails,
	ProjectSidebarLinks,
	ProjectSidebarModpacks,
	ProjectSidebarRepository,
	ProjectSidebarServerInfo,
	ProjectSidebarTags,
	SelectedProjectsFloatingBar,
	TeleportOverflowMenu,
	useVIntl,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { openUrl } from '@tauri-apps/plugin-opener'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'
import { computed, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import InstanceIndicator from '@/components/ui/InstanceIndicator.vue'
import {
	fetchCachedServerStatus,
	getFreshCachedServerStatus,
} from '@/composables/instances/use-server-status-query'
import { useAppEvent } from '@/composables/use-app-event'
import { useAppSettings } from '@/composables/use-app-settings.ts'
import { useCurseforgeKey } from '@/composables/use-curseforge-key'
import {
	get_curseforge_project_many,
	get_curseforge_search_results,
	get_organization,
	get_project,
	get_project_many,
	get_project_v3,
	get_search_results_v3,
	get_team,
	get_version,
	get_version_many,
} from '@/helpers/cache.js'
import {
	CF_ID_PREFIX,
	cfProjectUrl,
	getCfProject,
	getCfVersions,
	isCfProjectId,
	parseCfId,
} from '@/helpers/curseforge-project'
import {
	get as getInstance,
	get_install_candidates,
	get_projects as getInstanceProjects,
	getInstanceIconUrl,
	kill,
	list as listInstances,
} from '@/helpers/instance'
import { get_by_instance_id } from '@/helpers/process'
import { get_categories, get_game_versions, get_loaders } from '@/helpers/tags'
import { getServerAddress } from '@/helpers/worlds'
import { provideBreadcrumbParent, useBreadcrumb } from '@/providers/breadcrumbs'
import { injectContentInstall } from '@/providers/content-install'
import { injectServerInstall } from '@/providers/server-install'

dayjs.extend(relativeTime)

const { handleError } = injectNotificationManager()
const { install: installVersion } = injectContentInstall()
const route = useRoute()
const router = useRouter()
const displayedProjectRoute = shallowRef(router.currentRoute.value)
watch(
	() => router.currentRoute.value,
	(nextRoute) => {
		if (nextRoute.path.startsWith('/project/')) {
			displayedProjectRoute.value = nextRoute
		}
	},
	{ immediate: true },
)
const projectBreadcrumbTo = computed(() => {
	const currentRoute = displayedProjectRoute.value
	if (currentRoute.name === 'Version') {
		return {
			name: 'Versions',
			params: { id: currentRoute.params.id },
			query: currentRoute.query,
		}
	}

	return currentRoute.fullPath
})
const queryClient = useQueryClient()
const appSettings = useAppSettings()
const { formatMessage } = useVIntl()

const messages = defineMessages({
	moreOptions: { id: 'app.project.more-options', defaultMessage: 'More options' },
	projectActionsLabel: { id: 'app.project.actions.label', defaultMessage: 'Project actions' },
	descriptionTab: { id: 'app.project.tab.description', defaultMessage: 'Description' },
	versionsTab: { id: 'app.project.tab.versions', defaultMessage: 'Versions' },
	galleryTab: { id: 'app.project.tab.gallery', defaultMessage: 'Gallery' },
	loadError: {
		id: 'app.project.load-error',
		defaultMessage: 'Project data could not be loaded.',
	},
	comingSoon: { id: 'app.project.coming-soon', defaultMessage: 'Coming soon' },
	viewOnModrinth: {
		id: 'app.project.view-on-modrinth',
		defaultMessage: 'View on Modrinth',
	},
	viewOnCurseforge: {
		id: 'app.project.view-on-curseforge',
		defaultMessage: 'View on CurseForge',
	},
	switchSourceNotFound: {
		id: 'app.project.switch-source-not-found',
		defaultMessage: 'No matching project found on the other source.',
	},
	curseforgeNotConfigured: {
		id: 'app.project.switch-source.curseforge-not-configured',
		defaultMessage:
			'A CurseForge API key is not configured. Set one in Settings to view CurseForge projects.',
	},
	backToBrowse: {
		id: 'app.project.install-context.back-to-browse',
		defaultMessage: 'Back to discover',
	},
	backToInstance: {
		id: 'app.project.install-context.back-to-instance',
		defaultMessage: 'Back to instance',
	},
	alreadyInstalled: {
		id: 'app.project.install-button.already-installed',
		defaultMessage: 'This project is already installed',
	},
	switchVersion: {
		id: 'app.project.install-button.switch-version',
		defaultMessage: 'Switch version',
	},
	switchSource: {
		id: 'app.project.switch-source',
		defaultMessage: 'Switch source',
	},
})

const { installingServerProjects, playServerProject, showAddServerToInstanceModal } =
	injectServerInstall()
const installing = ref(false)
const data = shallowRef(null)

function getProjectBreadcrumbSummary(projectId) {
	const identifier = Array.isArray(projectId) ? projectId[0] : projectId
	if (typeof identifier !== 'string' || !identifier) return undefined

	return queryClient.getQueryData(['projects', 'summary', identifier])
}

function getProjectBreadcrumbLabel(projectId) {
	const summary = getProjectBreadcrumbSummary(projectId)
	return summary?.name ?? summary?.title ?? formatMessage(commonMessages.loadingLabel)
}

const projectBreadcrumbLabel = ref(getProjectBreadcrumbLabel(route.params.id))
const projectBreadcrumb = useBreadcrumb({
	slot: 'project',
	id: () => `project:${String(displayedProjectRoute.value.params.id ?? '')}`,
	label: projectBreadcrumbLabel,
	visual: () => {
		const identifier = String(displayedProjectRoute.value.params.id ?? '')
		const loadedProject =
			data.value?.id === identifier || data.value?.slug === identifier ? data.value : undefined
		const project = loadedProject ?? getProjectBreadcrumbSummary(identifier)
		return {
			type: 'image',
			src: project?.icon_url,
			alt: projectBreadcrumbLabel.value,
			tintBy: identifier,
		}
	},
	to: projectBreadcrumbTo,
})
provideBreadcrumbParent(projectBreadcrumb)

const versions = shallowRef([])
const members = shallowRef([])
const categories = shallowRef([])
const organization = shallowRef(null)
const instance = ref(null)
const instanceProjects = ref(null)

const installed = ref(false)
const installedVersion = ref(null)
const isServerProject = ref(false)
const projectV3 = shallowRef(null)
const serverRequiredContent = shallowRef(null)
const serverRecommendedVersion = shallowRef(null)
const serverSupportedVersions = shallowRef([])
const serverModpackLoaders = shallowRef([])
const serverPing = ref(undefined)
const serverStatusOnline = ref(false)
const serverInstancePath = ref(null)
const serverPlaying = ref(false)

/** @typedef {{ dependency_type: string, project_id: string, title: string, icon_url?: string | null }} SidebarDependency */
const sidebarDependencies = shallowRef([])

watch([data, versions], async () => {
	sidebarDependencies.value = []
	const deps = (versions.value[0]?.dependencies ?? []).filter(
		(dep) => dep.project_id && dep.dependency_type !== 'embedded',
	)
	const projectId = data.value?.id
	if (!deps.length || !projectId) return
	const depIds = deps.map((dep) => dep.project_id)
	const [mrIds, cfIds] = [
		depIds.filter((id) => !id.startsWith('cf-')),
		depIds.filter((id) => id.startsWith('cf-')),
	]
	const [mrProjects, cfProjects] = await Promise.all([
		mrIds.length ? get_project_many(mrIds, 'must_revalidate').catch(() => null) : null,
		cfIds.length
			? get_curseforge_project_many(
					cfIds.map((id) => id.slice(CF_ID_PREFIX.length)),
					'must_revalidate',
				).catch(() => null)
			: null,
	])
	if (data.value?.id !== projectId) return
	const resolved = new Map()
	for (const project of mrProjects ?? []) {
		resolved.set(project.id, project)
	}
	for (const project of cfProjects ?? []) {
		resolved.set(`${CF_ID_PREFIX}${project.id}`, project)
	}
	sidebarDependencies.value = deps.map((dep) => {
		const project = resolved.get(dep.project_id)
		return {
			dependency_type: dep.dependency_type,
			project_id: dep.project_id,
			title: project?.title ?? dep.file_name ?? dep.project_id,
			icon_url: project?.icon_url ?? null,
		}
	})
})

const instanceFilters = computed(() => {
	if (!instance.value) {
		return {}
	}

	const loaders = []
	if (data.value.project_type === 'mod') {
		if (instance.value.loader !== 'vanilla') {
			loaders.push(instance.value.loader)
		}
		if (instance.value.loader === 'vanilla' || data.value.loaders.includes('datapack')) {
			loaders.push('datapack')
		}
	}

	return { l: loaders, g: instance.value.game_version }
})

function buildProjectHref(path, extraQuery = {}) {
	const params = new URLSearchParams()
	for (const [key, val] of Object.entries({ ...route.query, ...extraQuery })) {
		if (Array.isArray(val)) {
			for (const v of val) params.append(key, v)
		} else if (val) {
			params.append(key, String(val))
		}
	}
	const qs = params.toString()
	return qs ? `${path}?${qs}` : path
}

function buildBrowseHref(path) {
	const params = new URLSearchParams()
	for (const [key, val] of Object.entries(route.query)) {
		if (key === 'b') continue
		if (Array.isArray(val)) {
			for (const v of val) params.append(key, v)
		} else if (val) {
			params.append(key, String(val))
		}
	}
	const qs = params.toString()
	return qs ? `${path}?${qs}` : path
}

const projectDescriptionHref = computed(() => buildProjectHref(`/project/${route.params.id}`))
const versionsHref = computed(() =>
	buildProjectHref(`/project/${route.params.id}/versions`, instanceFilters.value),
)
const projectGalleryHref = computed(() => buildProjectHref(`/project/${route.params.id}/gallery`))

const projectBrowseBackUrl = computed(() => {
	const browsePath = route.query.b
	if (typeof browsePath === 'string' && browsePath.startsWith('/browse/')) return browsePath
	const instanceId = route.query.i
	if (typeof instanceId === 'string' && instanceId) {
		return `/instance/${encodeURIComponent(instanceId)}`
	}
	const type = data.value?.project_type ? `${data.value.project_type}` : 'mod'
	return buildBrowseHref(`/browse/${type}`)
})
const projectBackLabel = computed(() =>
	typeof route.query.i === 'string' && typeof route.query.b !== 'string'
		? formatMessage(messages.backToInstance)
		: formatMessage(messages.backToBrowse),
)

const projectInstallContext = computed(() => {
	if (instance.value) {
		return {
			name: instance.value.name,
			loader: instance.value.loader,
			gameVersion: instance.value.game_version,
			iconSrc: getInstanceIconUrl(instance.value.icon_path),
			backUrl: projectBrowseBackUrl.value,
			backLabel: projectBackLabel.value,
			heading: formatMessage(commonMessages.installingContentLabel),
		}
	}

	return null
})

const installButtonLoading = computed(() => installing.value)
const installButtonValidating = computed(
	() => installing.value && data.value?.project_type !== 'modpack',
)
const installButtonInstalled = computed(() => installed.value)
const installButtonDisabled = computed(
	() => installButtonInstalled.value || installButtonLoading.value,
)
const serverInstallLoading = computed(
	() => !!data.value && installingServerProjects.value.includes(data.value.id),
)
const installButtonIcon = computed(() => {
	if (installButtonLoading.value && !installButtonInstalled.value) return SpinnerIcon
	if (!installButtonInstalled.value) return DownloadIcon
	return CheckIcon
})
const installButtonIconClass = computed(() =>
	installButtonLoading.value && !installButtonInstalled.value ? 'animate-spin' : undefined,
)
const serverProjectHeaderMoreActions = computed(() => [
	{
		id: 'open-in-browser',
		label: formatMessage(
			isCfProjectId(data.value?.id)
				? commonMessages.openInBrowserButton
				: commonMessages.openInModrinthButton,
		),
		icon: ExternalIcon,
		action: openProjectInBrowser,
	},
	{
		type: 'divider',
	},
	{
		id: 'report',
		label: formatMessage(commonMessages.reportButton),
		icon: ReportIcon,
		tone: 'red',
		action: reportProject,
	},
])
const projectHeaderMoreActions = computed(() => [
	{
		id: 'follow',
		label: formatMessage(commonMessages.followButton),
		icon: HeartIcon,
		disabled: true,
		tooltip: formatMessage(messages.comingSoon),
		action: () => {},
	},
	{
		id: 'save',
		label: formatMessage(commonMessages.saveButton),
		icon: BookmarkIcon,
		disabled: true,
		tooltip: formatMessage(messages.comingSoon),
		action: () => {},
	},
	{
		id: 'open-in-browser',
		label: formatMessage(
			isCfProjectId(data.value?.id)
				? commonMessages.openInBrowserButton
				: commonMessages.openInModrinthButton,
		),
		icon: ExternalIcon,
		action: openProjectInBrowser,
	},
	{
		id: 'switch-source',
		label: formatMessage(
			isCfProjectId(data.value?.id) ? messages.viewOnModrinth : messages.viewOnCurseforge,
		),
		icon: ArrowLeftRightIcon,
		action: switchSource,
	},
	{
		type: 'divider',
	},
	{
		id: 'report',
		label: formatMessage(commonMessages.reportButton),
		icon: ReportIcon,
		tone: 'red',
		action: reportProject,
	},
])
const projectSearchUrl = computed(
	() => `/browse/${isServerProject.value ? 'server' : data.value?.project_type}`,
)

const showSwitchVersion = computed(() => !!instance.value && installed.value)
const onVersionsPage = computed(() => route.name === 'Versions')

function goToVersions() {
	router.push(versionsHref.value)
}

const [allLoaders, allGameVersions] = await Promise.all([
	get_loaders().catch(handleError).then(ref),
	get_game_versions().catch(handleError).then(ref),
])

async function handleClickPlay() {
	if (!isServerProject.value) return
	await playServerProject(data.value.id).catch(handleError)
	await updateServerPlayState()
}

async function updateServerPlayState() {
	if (!isServerProject.value || !data.value) return
	const packs = await listInstances()
	const inst = packs.find((p) => p.link?.project_id === data.value.id)
	if (inst) {
		serverInstancePath.value = inst.id
		const processes = await get_by_instance_id(inst.id).catch(() => [])
		serverPlaying.value = Array.isArray(processes) && processes.length > 0
	} else {
		serverInstancePath.value = null
		serverPlaying.value = false
	}
}

async function handleStopServer() {
	if (!serverInstancePath.value) return
	await kill(serverInstancePath.value).catch(() => {})
	serverPlaying.value = false
}

function handleAddServerToInstance() {
	const address = getServerAddress(projectV3.value?.minecraft_java_server)
	if (!address || !data.value) return
	showAddServerToInstanceModal(data.value.title, address)
}

function openProjectInBrowser() {
	if (!data.value) return
	if (isCfProjectId(data.value.id)) {
		void openUrl(cfProjectUrl(data.value))
		return
	}
	const type = isServerProject.value ? 'project' : data.value.project_type
	void openUrl(`https://modrinth.com/${type}/${data.value.slug}`)
}

function getUserLink(username, member) {
	if (member?.user?.id?.startsWith('cf-user-')) {
		return `https://www.curseforge.com/members/${encodeURIComponent(member.user.username)}`
	}
	return `/user/${encodeURIComponent(username)}`
}

const CF_MODRINTH_TYPE_CLASS_IDS = {
	mod: 6,
	modpack: 4471,
	resourcepack: 12,
	shader: 6552,
	datapack: 6945,
}

const curseforgeApiKey = useCurseforgeKey()

function normalizeName(name) {
	return (name ?? '').toLowerCase().replace(/[^a-z0-9]/g, '')
}

function findBestMatch(hits, title, getNames) {
	const normalizedTitle = normalizeName(title)
	const names = (hit) => (getNames(hit) ?? []).filter(Boolean).map(normalizeName).filter(Boolean)
	const byDownloads = (a, b) => (b.downloads ?? 0) - (a.downloads ?? 0)
	const exact = hits.find((hit) =>
		getNames(hit)?.some((n) => n.toLowerCase() === title.toLowerCase()),
	)
	if (exact) return exact
	const normalized = hits
		.filter((hit) => names(hit).some((n) => n === normalizedTitle))
		.sort(byDownloads)
	if (normalized.length > 0) return normalized[0]
	const contained = hits
		.filter((hit) =>
			names(hit).some((n) => n.includes(normalizedTitle) || normalizedTitle.includes(n)),
		)
		.sort(byDownloads)
	return contained[0] ?? null
}

function modrinthProjectType(project) {
	const type = project.project_type
	if (CF_MODRINTH_TYPE_CLASS_IDS[type]) return type
	return (
		{
			resource_pack: 'resourcepack',
			shader_pack: 'shader',
			data_pack: 'datapack',
		}[type] ?? null
	)
}

async function switchSource() {
	const project = data.value
	if (!project) return
	if (isCfProjectId(project.id)) {
		let results = null
		try {
			const type = modrinthProjectType(project)
			const facets = type
				? `&facets=${encodeURIComponent(JSON.stringify([[`project_types:${type}`]]))}`
				: ''
			results = await get_search_results_v3(
				`?query=${encodeURIComponent(project.title)}&limit=50${facets}`,
				'must_revalidate',
			)
		} catch (err) {
			handleError(err)
			return
		}
		const match = findBestMatch(results?.result?.hits ?? [], project.title, (hit) => [
			hit.name,
			hit.slug,
		])
		if (match) {
			await router.push(`/project/${match.project_id}`)
		} else {
			handleError(formatMessage(messages.switchSourceNotFound))
		}
	} else {
		if (!curseforgeApiKey.value) {
			handleError(formatMessage(messages.curseforgeNotConfigured))
			return
		}
		// CF slug search is authoritative; CF relevance searchFilter is not
		if (project.slug) {
			let slugResults = null
			try {
				slugResults = await get_curseforge_search_results(
					`?gameId=432&slug=${encodeURIComponent(project.slug)}&pageSize=1`,
					'must_revalidate',
				)
			} catch (err) {
				handleError(err)
				return
			}
			const slugHit = slugResults?.projectHits?.[0]
			if (slugHit) {
				await router.push(`/project/${slugHit.project_id}`)
				return
			}
		}
		const classId = CF_MODRINTH_TYPE_CLASS_IDS[modrinthProjectType(project)]
		let results = null
		try {
			results = await get_curseforge_search_results(
				`?gameId=432&searchFilter=${encodeURIComponent(project.title)}&pageSize=50${
					classId ? `&classId=${classId}` : ''
				}`,
				'must_revalidate',
			)
		} catch (err) {
			handleError(err)
			return
		}
		const match = findBestMatch(results?.projectHits ?? [], project.title, (hit) => [
			hit.name,
			hit.slug,
		])
		if (match) {
			await router.push(`/project/${match.project_id}`)
		} else {
			handleError(formatMessage(messages.switchSourceNotFound))
		}
	}
}

function reportProject() {
	if (!data.value) return
	if (isCfProjectId(data.value.id)) {
		void openUrl(cfProjectUrl(data.value))
		return
	}
	void openUrl(`https://modrinth.com/report?item=project&itemID=${data.value.id}`)
}

async function fetchProjectData() {
	const requestedId = String(route.params.id ?? '')
	projectBreadcrumbLabel.value = getProjectBreadcrumbLabel(requestedId)
	if (isCfProjectId(requestedId)) {
		await fetchCfProjectData(requestedId)
		return
	}
	const [project, projectV3Result] = await Promise.all([
		get_project(requestedId, 'must_revalidate').catch(handleError),
		get_project_v3(requestedId, 'must_revalidate').catch(handleError),
	])
	if (String(route.params.id ?? '') !== requestedId) {
		return
	}

	projectV3.value = projectV3Result

	if (!project) {
		handleError('Error loading project')
		return
	}

	data.value = project
	projectBreadcrumbLabel.value = project.title
	;[versions.value, members.value, categories.value, instance.value, instanceProjects.value] =
		await Promise.all([
			get_version_many(project.versions, 'must_revalidate').catch(handleError),
			get_team(project.team).catch(handleError),
			get_categories().catch(handleError),
			route.query.i ? getInstance(route.query.i).catch(handleError) : Promise.resolve(),
			route.query.i ? getInstanceProjects(route.query.i).catch(handleError) : Promise.resolve(),
		])
	if (String(route.params.id ?? '') !== requestedId) {
		return
	}

	for (const member of members.value ?? []) {
		for (const identifier of [member.user.id, member.user.username]) {
			if (identifier) {
				queryClient.setQueryData(['users', 'summary', identifier], member.user)
			}
		}
	}

	versions.value = versions.value.sort((a, b) => dayjs(b.date_published) - dayjs(a.date_published))

	const installedFile = instanceProjects.value
		? Object.values(instanceProjects.value).find(
				(x) => x.metadata && x.metadata.project_id === data.value.id,
			)
		: undefined
	installed.value = !!installedFile
	if (!installed.value) {
		installed.value = await checkCrossSourceInstalled(project, requestedId)
	}
	installedVersion.value = installedFile?.metadata.version_id ?? null

	if (project.organization) {
		organization.value = await get_organization(project.organization).catch(handleError)
	} else {
		organization.value = null
	}
	if (String(route.params.id ?? '') !== requestedId) {
		return
	}

	isServerProject.value = projectV3.value?.minecraft_server != null
	serverStatusOnline.value = !!projectV3.value?.minecraft_java_server?.ping?.data

	fetchDeferredServerData(project)
}

async function fetchCfProjectData(requestedId) {
	const [project, cfVersions] = await Promise.all([
		getCfProject(requestedId).catch(handleError),
		getCfVersions(requestedId).catch(handleError),
	])
	if (String(route.params.id ?? '') !== requestedId) return

	if (!project) {
		handleError('Error loading project')
		return
	}

	data.value = project
	projectV3.value = null
	projectBreadcrumbLabel.value = project.title
	const cfMembers = project.cf_members ?? []
	;[versions.value, members.value, categories.value, instance.value, instanceProjects.value] =
		await Promise.all([
			Promise.resolve(cfVersions.sort((a, b) => dayjs(b.date_published) - dayjs(a.date_published))),
			Promise.resolve(cfMembers),
			Promise.resolve([]),
			route.query.i ? getInstance(route.query.i).catch(handleError) : Promise.resolve(),
			route.query.i ? getInstanceProjects(route.query.i).catch(handleError) : Promise.resolve(),
		])
	if (String(route.params.id ?? '') !== requestedId) return

	const installedFile = instanceProjects.value
		? Object.values(instanceProjects.value).find(
				(x) => x.metadata && x.metadata.cf_project_id === parseCfId(requestedId),
			)
		: undefined
	installed.value = !!installedFile
	if (!installed.value) {
		installed.value = await checkCrossSourceInstalled(project, requestedId)
	}
	installedVersion.value = installedFile?.metadata.cf_version_id
		? `cf-${installedFile.metadata.cf_version_id}`
		: null

	organization.value = null
	isServerProject.value = false
	serverStatusOnline.value = false
}

async function checkCrossSourceInstalled(project, requestedId) {
	if (!route.query.i || !project) return false
	try {
		const candidates = await get_install_candidates(
			requestedId,
			project.project_type,
			[],
			project.slug,
			project.title,
		)
		return candidates.some((candidate) => candidate.id === route.query.i && candidate.installed)
	} catch {
		return false
	}
}

function fetchDeferredServerData(project) {
	const serverAddress = projectV3.value?.minecraft_java_server?.address
	if (serverAddress) {
		const cachedStatus = getFreshCachedServerStatus(queryClient, serverAddress)
		if (cachedStatus) {
			serverPing.value = cachedStatus.ping
			serverStatusOnline.value = true
		} else {
			serverPing.value = undefined
		}

		fetchCachedServerStatus(queryClient, serverAddress)
			.then((status) => {
				if (projectV3.value?.minecraft_java_server?.address !== serverAddress) return
				serverPing.value = status.ping
				serverStatusOnline.value = true
			})
			.catch((error) => {
				console.error(`Failed to ping server ${serverAddress}:`, error)
			})
	}

	const content = projectV3.value?.minecraft_java_server?.content
	if (content?.kind === 'modpack' && content.version_id) {
		get_version(content.version_id, 'bypass')
			.catch(handleError)
			.then(async (modpackVersion) => {
				if (!modpackVersion) return
				serverRecommendedVersion.value = modpackVersion.game_versions?.[0] ?? null
				serverModpackLoaders.value = modpackVersion.mrpack_loaders ?? []
				if (modpackVersion.project_id) {
					const modpackProject = await get_project_v3(
						modpackVersion.project_id,
						'must_revalidate',
					).catch(handleError)
					if (modpackProject) {
						const primaryFile =
							modpackVersion.files?.find((f) => f.primary) ?? modpackVersion.files?.[0]

						serverRequiredContent.value = {
							name: modpackProject.name,
							versionNumber: modpackVersion.version_number ?? '',
							icon: modpackProject.icon_url,
							onclickName:
								modpackProject.id !== project.id
									? () => router.push(`/project/${modpackProject.id}`)
									: undefined,
							onclickVersion:
								modpackProject.id !== project.id
									? () => router.push(`/project/${modpackProject.id}/version/${modpackVersion.id}`)
									: undefined,
							onclickDownload: primaryFile?.url ? () => openUrl(primaryFile.url) : undefined,
							showCustomModpackTooltip: modpackProject.id === project.id,
						}
					}
				}
			})
	} else if (content?.kind === 'vanilla') {
		serverRecommendedVersion.value = content.recommended_game_version ?? null
		const supported = content.supported_game_versions ?? []
		serverSupportedVersions.value = supported.filter((v) => !!v)
	}

	updateServerPlayState()
}

await fetchProjectData()

useAppEvent('process', (e) => {
	if (
		e.event === 'finished' &&
		serverInstancePath.value &&
		e.instance_id === serverInstancePath.value
	) {
		serverPlaying.value = false
	}
})

watch(
	() => route.params.id,
	async () => {
		if (route.params.id && route.path.startsWith('/project')) {
			await fetchProjectData()
		}
	},
)

async function install(version) {
	installing.value = true
	await installVersion(
		data.value.id,
		version,
		instance.value ? instance.value.id : null,
		'ProjectPage',
		(version, installedProjectIds) => {
			installing.value = false

			const installedIds = installedProjectIds ?? [data.value.id]
			if (instance.value && version && installedIds.includes(data.value.id)) {
				installed.value = true
				installedVersion.value = version
			}
		},
		(profile) => {
			router.push(`/instance/${profile}`)
		},
	).catch(handleError)
}

const options = ref(null)
const handleRightClick = (event) => {
	const project = data.value
	options.value.open(event, [
		{
			id: 'install',
			label: formatMessage(commonMessages.installButton),
			icon: DownloadIcon,
			action: () => install(null),
		},
		{ type: 'divider' },
		{
			id: 'open-in-browser',
			label: formatMessage(
				isCfProjectId(data.value?.id)
					? commonMessages.openInBrowserButton
					: commonMessages.openInModrinthButton,
			),
			icon: ExternalIcon,
			action: openProjectInBrowser,
		},
		{
			id: 'copy_link',
			label: formatMessage(commonMessages.copyLinkButton),
			icon: ClipboardCopyIcon,
			action: () => copyProjectLink(project),
		},
		{
			id: 'switch-source',
			label: formatMessage(
				isCfProjectId(data.value?.id) ? messages.viewOnModrinth : messages.viewOnCurseforge,
			),
			icon: ArrowLeftRightIcon,
			action: switchSource,
		},
	])
}
const getProjectLink = (project) =>
	isCfProjectId(project.id)
		? cfProjectUrl(project)
		: `https://modrinth.com/${project.project_type}/${project.slug}`
const copyProjectLink = (project) => navigator.clipboard.writeText(getProjectLink(project))
</script>

<style scoped lang="scss">
.root-container {
	display: flex;
	flex-direction: row;
	min-height: 100%;
}

.project-sidebar {
	position: fixed;
	width: calc(300px + 1.5rem);
	min-height: calc(100vh - 3.25rem);
	height: fit-content;
	max-height: calc(100vh - 3.25rem);
	padding: 1rem 0.5rem 1rem 1rem;
	overflow-y: auto;
	-ms-overflow-style: none;
	scrollbar-width: none;

	&::-webkit-scrollbar {
		width: 0;
		background: transparent;
	}
}

.sidebar-card {
	display: flex;
	flex-direction: column;
	gap: 1rem;
}

.content-container {
	display: flex;
	flex-direction: column;
	width: 100%;
	padding: 1rem;
	margin-left: calc(300px + 1rem);
}

.button-group {
	display: flex;
	flex-wrap: wrap;
	flex-direction: row;
	gap: 0.5rem;
}

.stats {
	display: flex;
	flex-direction: column;
	flex-wrap: wrap;
	gap: var(--gap-md);

	.stat {
		display: flex;
		flex-direction: row;
		align-items: center;
		width: fit-content;
		gap: var(--gap-xs);
		--stat-strong-size: 1.25rem;

		strong {
			font-size: var(--stat-strong-size);
		}

		p {
			margin: 0;
		}

		svg {
			min-height: var(--stat-strong-size);
			min-width: var(--stat-strong-size);
		}
	}

	.date {
		margin-top: auto;
	}
}

.tabs {
	display: flex;
	flex-direction: row;
	gap: 1rem;
	margin-bottom: var(--gap-md);
	justify-content: space-between;

	.tab {
		display: flex;
		flex-direction: row;
		align-items: center;
		border-radius: var(--border-radius);
		cursor: pointer;
		transition: background-color 0.2s ease-in-out;

		&:hover {
			background-color: var(--color-raised-bg);
		}

		&.router-view-active {
			background-color: var(--color-raised-bg);
		}
	}
}

.links {
	a {
		display: inline-flex;
		align-items: center;
		border-radius: 1rem;
		color: var(--color-text);

		svg,
		img {
			height: 1rem;
			width: 1rem;
		}

		span {
			margin-left: 0.25rem;
			text-decoration: underline;
			line-height: 2rem;
		}

		&:focus-visible,
		&:hover {
			svg,
			img,
			span {
				color: var(--color-heading);
			}
		}

		&:active {
			svg,
			img,
			span {
				color: var(--color-text-dark);
			}
		}

		&:not(:last-child)::after {
			content: '•';
			margin: 0 0.25rem;
		}
	}
}

.install-loading {
	scale: 0.2;
	height: 1rem;
	width: 1rem;
	margin-right: -1rem;

	:deep(svg) {
		color: var(--color-contrast);
	}
}

.project-sidebar-section {
	@apply p-4 flex flex-col gap-2 border-0 border-b-[1px] border-[--brand-gradient-border] border-solid;
}
</style>
