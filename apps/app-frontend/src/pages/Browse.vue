<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import {
	CheckIcon,
	CompassIcon,
	ExternalIcon,
	GlobeIcon,
	PlusIcon,
	SpinnerIcon,
} from '@modrinth/assets'
import type {
	BrowseInstallContentType,
	CardAction,
	CurseforgeCategory,
	ProjectType,
	Tags,
} from '@modrinth/ui'
import {
	BrowsePageLayout,
	BrowseSidebar,
	commonMessages,
	ContextMenu,
	defineMessages,
	formatProjectTypeSentence,
	getLatestMatchingInstallVersion,
	getSelectedInstallPreferences,
	getTargetInstallPreferences,
	injectNotificationManager,
	mapProvidedFiltersToCf,
	preferencesDiffer,
	provideBrowseManager,
	resolveInstallPlan,
	useBrowseSearch,
	useDebugLogger,
	useVIntl,
} from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { Ref } from 'vue'
import { computed, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import type { LocationQuery } from 'vue-router'
import { useRoute, useRouter } from 'vue-router'

import { useAppServerBrowse } from '@/composables/browse/use-app-server-browse'
import { useAppEvent } from '@/composables/use-app-event'
import { useAppSettings } from '@/composables/use-app-settings.ts'
import { useCurseforgeKey } from '@/composables/use-curseforge-key'
import {
	get_curseforge_categories,
	get_curseforge_search_results,
	get_project,
	get_search_results_v3,
	get_version_many,
} from '@/helpers/cache.js'
import { getCfVersions, isCfProjectId } from '@/helpers/curseforge-project'
import {
	get_installed_project_ids as getInstalledProjectIds,
	getInstanceIconUrl,
	list as listInstances,
} from '@/helpers/instance'
import { get as getSettings, set as setSettings } from '@/helpers/settings.ts'
import { get_categories, get_game_versions, get_loaders } from '@/helpers/tags'
import { get_instance_worlds } from '@/helpers/worlds'
import {
	instanceDetailQueryOptions,
	instanceKeys,
	instanceLinkedProjectQueryOptions,
} from '@/pages/instance/query-options'
import { type BreadcrumbDefinition, injectBreadcrumbManager } from '@/providers/breadcrumbs'
import { injectContentInstall } from '@/providers/content-install'
import { injectServerInstall } from '@/providers/server-install'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const { installingServerProjects, playServerProject, showAddServerToInstanceModal } =
	injectServerInstall()
const { install: installVersion } = injectContentInstall()
const queryClient = useQueryClient()
const debugLog = useDebugLogger('Browse')

const router = useRouter()
const route = useRoute()
const displayedBrowseRoute = shallowRef(router.currentRoute.value)
watch(
	() => router.currentRoute.value,
	(nextRoute) => {
		if (nextRoute.path.startsWith('/browse/')) {
			displayedBrowseRoute.value = nextRoute
		}
	},
	{ immediate: true },
)
const breadcrumbMessages = defineMessages({
	discoverProjectType: {
		id: 'app.browse.discover-project-type',
		defaultMessage: 'Discover {projectType}',
	},
	discoverServers: {
		id: 'app.browse.discover-servers',
		defaultMessage: 'Discover servers',
	},
})
const breadcrumbLabel = computed(() => {
	const browseRoute = displayedBrowseRoute.value
	if (browseRoute.query.from === 'worlds' || browseRoute.params.projectType === 'server') {
		return formatMessage(breadcrumbMessages.discoverServers)
	}

	return formatMessage(breadcrumbMessages.discoverProjectType, {
		projectType: formatProjectTypeSentence(
			formatMessage,
			String(browseRoute.params.projectType ?? ''),
			2,
		),
	})
})
const appSettings = useAppSettings()
const browseRouteActive = computed(() => route.path.startsWith('/browse/'))
const isFromWorlds = computed(() => displayedBrowseRoute.value.query.from === 'worlds')

const initialInstanceId = computed(() => String(route.query.i ?? ''))
const instanceQuery = useQuery(
	computed(() => ({
		...instanceDetailQueryOptions(initialInstanceId.value),
		enabled: !!initialInstanceId.value,
	})),
)
const instance = computed(() => instanceQuery.data.value ?? null)
const linkedInstanceProjectId = computed(() => instance.value?.link?.project_id ?? '')
const linkedInstanceProjectQuery = useQuery(
	computed(() => ({
		...instanceLinkedProjectQueryOptions(linkedInstanceProjectId.value),
		enabled: !!linkedInstanceProjectId.value,
	})),
)
const installedProjectIds: Ref<string[] | null> = ref(null)
const instanceHideInstalled = ref(route.query.ai === 'true')
const newlyInstalled = ref<string[]>([])
const hiddenInstanceProjectIds = ref<Set<string>>(new Set())
const hiddenInstanceProjectIdsInitialized = ref(false)
const isServerInstance = computed(
	() => linkedInstanceProjectQuery.data.value?.minecraft_server != null,
)

const breadcrumbManager = injectBreadcrumbManager()
const instanceBreadcrumbDefinition = {
	slot: 'instance',
	id: () => `instance:${String(displayedBrowseRoute.value.query.i ?? '')}`,
	label: () => instance.value?.name ?? formatMessage(commonMessages.loadingLabel),
	visual: () => ({
		type: 'image' as const,
		src: getInstanceIconUrl(instance.value?.icon_path),
		alt: instance.value?.name,
		tintBy: String(displayedBrowseRoute.value.query.i ?? ''),
	}),
	to: () => {
		const instancePath = `/instance/${encodeURIComponent(
			String(displayedBrowseRoute.value.query.i ?? ''),
		)}`
		return displayedBrowseRoute.value.query.from === 'worlds'
			? `${instancePath}/worlds`
			: instancePath
	},
} satisfies BreadcrumbDefinition
const breadcrumbDefinition = {
	slot: 'browse',
	id: () =>
		`browse:${String(displayedBrowseRoute.value.params.projectType ?? '')}:${String(
			displayedBrowseRoute.value.query.i ?? '',
		)}:${String(displayedBrowseRoute.value.query.from ?? '')}`,
	label: breadcrumbLabel,
	to: () => displayedBrowseRoute.value.fullPath,
	visual: { type: 'icon', component: CompassIcon },
} satisfies BreadcrumbDefinition

function syncBreadcrumbs() {
	if (displayedBrowseRoute.value.query.i) {
		const instanceBreadcrumb = breadcrumbManager.reset(instanceBreadcrumbDefinition)
		breadcrumbManager.push(breadcrumbDefinition, { parent: instanceBreadcrumb })
		return
	}

	breadcrumbManager.reset(breadcrumbDefinition)
}

watch(displayedBrowseRoute, syncBreadcrumbs, { immediate: true, flush: 'sync' })

debugLog('fetching tags (categories, loaders, gameVersions)')
const [categories, loaders, availableGameVersions] = await Promise.all([
	get_categories()
		.catch(handleError)
		.then(ref<Labrinth.Tags.v2.Category[]>),
	get_loaders()
		.catch(handleError)
		.then(ref<Labrinth.Tags.v2.Loader[]>),
	get_game_versions()
		.catch(handleError)
		.then(ref<Labrinth.Tags.v2.GameVersion[]>),
])

const tags: Ref<Tags> = computed(() => ({
	gameVersions: availableGameVersions.value ?? [],
	loaders: loaders.value ?? [],
	categories: categories.value ?? [],
}))

if (isFromWorlds.value && route.params.projectType !== 'server') {
	router.replace({
		path: '/browse/server',
		query: route.query,
	})
}

const allInstalledIds = computed(
	() => new Set([...newlyInstalled.value, ...(installedProjectIds.value ?? [])]),
)

function syncHiddenInstanceProjectIds() {
	hiddenInstanceProjectIds.value = new Set([
		...(installedProjectIds.value ?? []),
		...newlyInstalled.value,
	])
	hiddenInstanceProjectIdsInitialized.value = true
}

watch(
	installedProjectIds,
	(ids) => {
		if (!ids) return
		if (!hiddenInstanceProjectIdsInitialized.value) {
			syncHiddenInstanceProjectIds()
		}
	},
	{ immediate: true },
)

await initInstanceContext()

async function refreshInstalledProjectIds() {
	if (!route.query.i) {
		const instances = await queryClient
			.fetchQuery({
				queryKey: [...instanceKeys.all, 'installed-project-ids'],
				queryFn: listInstances,
				staleTime: 0,
			})
			.catch(handleError)
		if (!instances) return

		const ids = instances
			.map((gameInstance) => gameInstance.link?.project_id)
			.filter((id): id is string => !!id)
		debugLog('installedInstanceProjectIds loaded', { count: ids.length })
		installedProjectIds.value = ids
		return
	}

	if (route.query.from === 'worlds') {
		const targetInstanceId = route.query.i as string
		const worlds = await queryClient
			.fetchQuery({
				queryKey: instanceKeys.installedProjectIds(targetInstanceId, 'worlds'),
				queryFn: () => get_instance_worlds(targetInstanceId),
				staleTime: 0,
			})
			.catch(handleError)
		if (!worlds) return

		const serverProjectIds = worlds
			.filter((w) => w.type === 'server' && 'project_id' in w && w.project_id)
			.map((w) => (w as { project_id: string }).project_id)
		debugLog('installedServerProjectIds loaded', { count: serverProjectIds.length })
		installedProjectIds.value = serverProjectIds
		return
	}

	const targetInstanceId = route.query.i as string
	const ids = await queryClient
		.fetchQuery({
			queryKey: instanceKeys.installedProjectIds(targetInstanceId, 'content'),
			queryFn: () => getInstalledProjectIds(targetInstanceId),
			staleTime: 0,
		})
		.catch(handleError)
	if (!ids) return

	debugLog('installedProjectIds loaded', { count: ids.length })
	installedProjectIds.value = ids
}

async function initInstanceContext() {
	debugLog('initInstanceContext', {
		queryI: route.query.i,
		queryAi: route.query.ai,
		queryFrom: route.query.from,
	})
	await Promise.all([
		refreshInstalledProjectIds(),
		route.query.i ? instanceQuery.suspense().catch(handleError) : Promise.resolve(),
	])

	if (route.query.i) {
		debugLog('instance loaded', {
			name: instance.value?.name,
			loader: instance.value?.loader,
			gameVersion: instance.value?.game_version,
		})

		if (instance.value?.link?.project_id) {
			await linkedInstanceProjectQuery.suspense().catch(handleError)
		}
	}
}

function setBrowseHideInstalledFlag(flag: 'hide_installed_modpacks', value: boolean) {
	appSettings.featureFlags[flag] = value
	getSettings()
		.then((settings) => {
			settings.feature_flags[flag] = value
			return setSettings(settings)
		})
		.catch(handleError)
}

const hideInstalledModpacks = computed({
	get: () => appSettings.getFeatureFlag('hide_installed_modpacks'),
	set: (value: boolean) => setBrowseHideInstalledFlag('hide_installed_modpacks', value),
})

const instanceFilters = computed(() => {
	const filters = []

	if (instance.value) {
		const isVanillaShader = projectType.value === 'shader' && instance.value.loader === 'vanilla'
		const gameVersion = instance.value.game_version
		if (gameVersion && !isVanillaShader) {
			filters.push({ type: 'game_version', option: gameVersion })
		}

		const platform = instance.value.loader
		const supportedModLoaders = ['fabric', 'forge', 'quilt', 'neoforge']

		if (platform && projectType.value === 'mod' && supportedModLoaders.includes(platform)) {
			filters.push({ type: 'mod_loader', option: platform })
		}
		if (isVanillaShader) {
			filters.push({ type: 'shader_loader', option: 'vanilla' })
		}

		if (isServerInstance.value) {
			filters.push({ type: 'environment', option: 'client' })
		}
	}

	if (
		(instance.value || projectType.value === 'modpack') &&
		(projectType.value === 'modpack' ? hideInstalledModpacks.value : instanceHideInstalled.value) &&
		hiddenInstanceProjectIds.value.size > 0
	) {
		for (const id of hiddenInstanceProjectIds.value) {
			filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
		}
	}

	return filters
})

const combinedProvidedFilters = computed(() => instanceFilters.value)

const {
	serverPings,
	contextMenuRef,
	updateServerHits,
	getServerModpackContent,
	getServerCardActions,
	handleRightClick,
} = useAppServerBrowse({
	instance,
	isFromWorlds,
	allInstalledIds,
	newlyInstalled,
	installingServerProjects,
	playServerProject,
	showAddServerToInstanceModal,
	handleError,
	router,
})

const offline = ref(!navigator.onLine)
const handleOffline = () => {
	debugLog('went offline')
	offline.value = true
}
const handleOnline = () => {
	debugLog('went online')
	offline.value = false
}
window.addEventListener('offline', handleOffline)
window.addEventListener('online', handleOnline)

onBeforeUnmount(() => {
	window.removeEventListener('offline', handleOffline)
	window.removeEventListener('online', handleOnline)
})

const messages = defineMessages({
	addServersToInstance: {
		id: 'app.browse.add-servers-to-instance',
		defaultMessage: 'Adding server to instance',
	},
	projectActionsLabel: {
		id: 'app.browse.project-actions.label',
		defaultMessage: 'Project actions',
	},
	addToAnInstance: {
		id: 'app.browse.add-to-an-instance',
		defaultMessage: 'Add to an instance',
	},
	environmentProvidedByServer: {
		id: 'search.filter.locked.server-environment.title',
		defaultMessage: 'Only client-side mods can be added to the server instance',
	},
	gameVersionProvidedByInstance: {
		id: 'search.filter.locked.instance-game-version.title',
		defaultMessage: 'Game version is provided by the instance',
	},
	hideAddedServers: {
		id: 'app.browse.hide-added-servers',
		defaultMessage: 'Hide servers already added',
	},
	hideInstalledModpacks: {
		id: 'app.browse.hide-installed-modpacks',
		defaultMessage: 'Hide already installed',
	},
	installingToServer: {
		id: 'app.browse.server.installing',
		defaultMessage: 'Installing',
	},
	backToInstance: {
		id: 'app.browse.back-to-instance',
		defaultMessage: 'Back to instance',
	},
	serverInstanceContentWarning: {
		id: 'app.browse.server-instance-content-warning',
		defaultMessage:
			'Adding content may prevent you from joining this server. Any content you add will be removed when the managed server content is updated.',
	},
	modLoaderProvidedByInstance: {
		id: 'search.filter.locked.instance-loader.title',
		defaultMessage: 'Loader is provided by the instance',
	},
	modpacksProjectType: {
		id: 'app.browse.project-type.modpacks',
		defaultMessage: 'Modpacks',
	},
	modsProjectType: { id: 'app.browse.project-type.mods', defaultMessage: 'Mods' },
	resourcePacksProjectType: {
		id: 'app.browse.project-type.resource-packs',
		defaultMessage: 'Resource Packs',
	},
	dataPacksProjectType: {
		id: 'app.browse.project-type.data-packs',
		defaultMessage: 'Data Packs',
	},
	shadersProjectType: { id: 'app.browse.project-type.shaders', defaultMessage: 'Shaders' },
	serversProjectType: { id: 'app.browse.project-type.servers', defaultMessage: 'Servers' },
	providedByInstance: {
		id: 'search.filter.locked.instance',
		defaultMessage: 'Provided by the instance',
	},
	syncFilterButton: {
		id: 'search.filter.locked.instance.sync',
		defaultMessage: 'Sync with instance',
	},
})

const projectType = ref<ProjectType>(route.params.projectType as ProjectType)

function resetInstanceContext() {
	debugLog('instance context removed, resetting')
	installedProjectIds.value = null
	instanceHideInstalled.value = false
	newlyInstalled.value = []
	hiddenInstanceProjectIds.value = new Set()
	hiddenInstanceProjectIdsInitialized.value = false
	isServerInstance.value = false
	browseBreadcrumb.reset()
	void refreshInstalledProjectIds()
}

watch(
	() => route.params.projectType as ProjectType,
	async (newType) => {
		if (!browseRouteActive.value) {
			return
		}

		if (!newType || newType === projectType.value) return

		debugLog('projectType route param changed', { from: projectType.value, to: newType })
		projectType.value = newType
	},
)

watch(
	() => route.query.i,
	async (nextInstanceId, previousInstanceId) => {
		if (!route.path.startsWith('/browse') || nextInstanceId === previousInstanceId) return
		if (!nextInstanceId) {
			resetInstanceContext()
			return
		}

		installedProjectIds.value = null
		hiddenInstanceProjectIdsInitialized.value = false
		await Promise.all([instanceQuery.suspense().catch(handleError), refreshInstalledProjectIds()])
		if (instance.value?.link?.project_id) {
			await linkedInstanceProjectQuery.suspense().catch(handleError)
		}
	},
)

const selectableProjectTypes = computed(() => {
	let dataPacks = false,
		mods = false,
		modpacks = false

	if (instance.value) {
		if (
			availableGameVersions.value &&
			availableGameVersions.value.findIndex((x) => x.version === instance.value?.game_version) <=
				availableGameVersions.value.findIndex((x) => x.version === '1.13') &&
			!isServerInstance.value
		) {
			dataPacks = true
		}

		if (instance.value.loader !== 'vanilla') {
			mods = true
		}
	} else {
		dataPacks = true
		mods = true
		modpacks = true
	}

	const params: LocationQuery = {}

	if (route.query.i) params.i = route.query.i
	if (route.query.ai) params.ai = route.query.ai
	if (route.query.from) params.from = route.query.from

	const queryString = new URLSearchParams(params as Record<string, string>).toString()
	const suffix = queryString ? `?${queryString}` : ''

	if (isFromWorlds.value) {
		return [{ label: formatMessage(messages.serversProjectType), href: `/browse/server${suffix}` }]
	}

	return [
		{
			label: formatMessage(messages.modpacksProjectType),
			href: `/browse/modpack${suffix}`,
			shown: modpacks,
		},
		{ label: formatMessage(messages.modsProjectType), href: `/browse/mod${suffix}`, shown: mods },
		{
			label: formatMessage(messages.resourcePacksProjectType),
			href: `/browse/resourcepack${suffix}`,
		},
		{
			label: formatMessage(messages.dataPacksProjectType),
			href: `/browse/datapack${suffix}`,
			shown: dataPacks,
		},
		{ label: formatMessage(messages.shadersProjectType), href: `/browse/shader${suffix}` },
		{
			label: formatMessage(messages.serversProjectType),
			href: `/browse/server${suffix}`,
			shown: !instance.value,
		},
	]
})

const installContext = computed(() => {
	if (instance.value) {
		return {
			name: instance.value.name,
			loader: instance.value.loader,
			gameVersion: instance.value.game_version,
			iconSrc: getInstanceIconUrl(instance.value.icon_path),
			backUrl: `/instance/${encodeURIComponent(instance.value.id)}${isFromWorlds.value ? '/worlds' : ''}`,
			backLabel: formatMessage(messages.backToInstance),
			heading: formatMessage(
				isFromWorlds.value ? messages.addServersToInstance : commonMessages.installingContentLabel,
			),
			warning:
				isServerInstance.value && instance.value.loader !== 'vanilla' && !isFromWorlds.value
					? formatMessage(messages.serverInstanceContentWarning)
					: undefined,
		}
	}
	return null
})

const installingProjectIds = ref<Set<string>>(new Set())

function setProjectInstalling(projectId: string, installing: boolean) {
	const next = new Set(installingProjectIds.value)
	if (installing) {
		next.add(projectId)
	} else {
		next.delete(projectId)
	}
	installingProjectIds.value = next
}

function getCurrentSelectedInstallPreferences(projectTypeValue: string) {
	return getSelectedInstallPreferences({
		contentType: projectTypeValue,
		selectedFilters: searchState.currentFilters.value,
		providedFilters: combinedProvidedFilters.value,
		overriddenProvidedFilterTypes: searchState.overriddenProvidedFilterTypes.value,
	})
}

function getInstanceInstallTargetPreferences(projectTypeValue: string) {
	return getTargetInstallPreferences(
		{
			gameVersion: instance.value?.game_version,
			loader: instance.value?.loader,
		},
		projectTypeValue,
	)
}

async function getInstallProjectVersions(projectId: string) {
	if (isCfProjectId(projectId)) {
		return getCfVersions(projectId)
	}
	const project = await get_project(projectId, 'must_revalidate')
	return (await get_version_many(
		project.versions,
		'must_revalidate',
	)) as Labrinth.Versions.v2.Version[]
}

async function chooseInstanceInstallVersion(
	project: Labrinth.Search.v3.ResultSearchProject,
	projectTypeValue: string,
) {
	const targetInstance = instance.value
	if (!targetInstance) {
		return { versionId: null as string | null }
	}

	const selectedPreferences = getCurrentSelectedInstallPreferences(projectTypeValue)
	const targetPreferences = getInstanceInstallTargetPreferences(projectTypeValue)
	if (!preferencesDiffer(selectedPreferences, targetPreferences)) {
		return { versionId: null as string | null }
	}

	const selectedVersion = getLatestMatchingInstallVersion(
		await getInstallProjectVersions(project.project_id),
		selectedPreferences,
	)

	if (!selectedVersion) {
		return { versionId: null as string | null }
	}

	return { versionId: selectedVersion.id }
}

async function chooseFilterMatchingInstallVersion(
	project: Labrinth.Search.v3.ResultSearchProject,
	projectTypeValue: string,
) {
	const plan = await resolveInstallPlan({
		project: {
			project_id: project.project_id,
			title: project.title,
			icon_url: project.icon_url,
		},
		contentType: projectTypeValue as BrowseInstallContentType,
		selectedFilters: searchState.currentFilters.value,
		providedFilters: combinedProvidedFilters.value,
		overriddenProvidedFilterTypes: searchState.overriddenProvidedFilterTypes.value,
		targetPreferences: {},
		getProjectVersions: getInstallProjectVersions,
	})

	return { versionId: plan.versionId }
}

function getCardActions(
	result: Labrinth.Search.v3.ResultSearchProject,
	currentProjectType: string,
): CardAction[] {
	if (currentProjectType === 'server') {
		return getServerCardActions(result)
	}

	const projectResult = result as Labrinth.Search.v3.ResultSearchProject & {
		installed?: boolean
		installing?: boolean
	}
	const isInstalled =
		projectResult.installed || allInstalledIds.value.has(projectResult.project_id || '')
	const isInstalling = installingProjectIds.value.has(projectResult.project_id)
	const showAsInstalled = isInstalled && currentProjectType !== 'modpack'

	const isModpack = projectResult.project_types?.includes('modpack')
	const shouldUseInstallIcon = !!instance.value || isModpack

	return [
		{
			key: 'install',
			label: formatMessage(
				isInstalling
					? messages.installingToServer
					: showAsInstalled
						? commonMessages.installedLabel
						: shouldUseInstallIcon
							? commonMessages.installButton
							: messages.addToAnInstance,
			),
			icon: isInstalling ? SpinnerIcon : showAsInstalled ? CheckIcon : PlusIcon,
			iconClass: isInstalling ? 'animate-spin' : undefined,
			disabled: showAsInstalled || isInstalling,
			color: 'brand',
			type: 'outlined',
			onClick: async () => {
				setProjectInstalling(projectResult.project_id, true)
				try {
					const selectedInstall = instance.value
						? await chooseInstanceInstallVersion(projectResult, currentProjectType)
						: isModpack
							? await chooseFilterMatchingInstallVersion(projectResult, currentProjectType)
							: { versionId: null as string | null }
					if (selectedInstall === null) {
						setProjectInstalling(projectResult.project_id, false)
						return
					}
					const selectedPreferences = getCurrentSelectedInstallPreferences(currentProjectType)
					await installVersion(
						projectResult.project_id,
						selectedInstall.versionId,
						instance.value ? instance.value.id : null,
						'SearchCard',
						(versionId, installedProjectIds) => {
							setProjectInstalling(projectResult.project_id, false)
							if (versionId) {
								onSearchResultsInstalled(installedProjectIds ?? [projectResult.project_id])
							}
						},
						(profile) => {
							router.push(`/instance/${profile}`)
						},
						{
							preferredLoader: instance.value?.loader ?? selectedPreferences.loaders?.[0],
							preferredGameVersion:
								instance.value?.game_version ?? selectedPreferences.gameVersions?.[0],
						},
					)
				} catch (err) {
					setProjectInstalling(projectResult.project_id, false)
					handleError(err)
				}
			},
		},
	]
}

function onSearchResultInstalled(id: string) {
	if (!newlyInstalled.value.includes(id)) {
		newlyInstalled.value = [...newlyInstalled.value, id]
	}
}

function onSearchResultsInstalled(ids: string[]) {
	newlyInstalled.value = Array.from(new Set([...newlyInstalled.value, ...ids]))
}

async function search(requestParams: string) {
	debugLog('searching v3', requestParams)
	const isServer = projectType.value === 'server'

	const rawResults = await queryClient.fetchQuery({
		queryKey: ['search', 'v3', requestParams],
		queryFn: () =>
			get_search_results_v3(requestParams, 'must_revalidate') as Promise<{
				result: Labrinth.Search.v3.SearchResults & {
					hits: (Labrinth.Search.v3.ResultSearchProject & { installed?: boolean })[]
				}
			} | null>,
		staleTime: 30_000,
	})

	if (!rawResults) {
		return {
			projectHits: [],
			serverHits: [],
			total_hits: 0,
			per_page: 20,
		}
	}

	for (const hit of rawResults.result.hits) {
		for (const identifier of [hit.project_id, hit.slug]) {
			if (identifier) {
				queryClient.setQueryData(['projects', 'summary', identifier], hit)
			}
		}
	}

	if (isServer) {
		const hits = rawResults.result.hits ?? []
		updateServerHits(hits)
		return {
			projectHits: [],
			serverHits: hits,
			total_hits: rawResults.result.total_hits ?? 0,
			per_page: rawResults.result.hits_per_page,
		}
	}

	const hits = rawResults.result.hits.map((hit) => {
		const mapped: Labrinth.Search.v3.ResultSearchProject & { installed?: boolean } = {
			...hit,
		}

		if (instance.value || projectType.value === 'modpack') {
			mapped.installed = new Set([
				...newlyInstalled.value,
				...(installedProjectIds.value ?? []),
			]).has(hit.project_id)
		}

		return mapped
	})

	return {
		projectHits: hits,
		serverHits: [],
		total_hits: rawResults.result.total_hits,
		per_page: rawResults.result.hits_per_page,
	}
}

const lockedFilterMessages = computed(() => ({
	gameVersion: formatMessage(messages.gameVersionProvidedByInstance),
	modLoader: formatMessage(messages.modLoaderProvidedByInstance),
	environment: formatMessage(messages.environmentProvidedByServer),
	syncButton: formatMessage(messages.syncFilterButton),
	providedBy: formatMessage(messages.providedByInstance),
}))

const curseforgeApiKey = useCurseforgeKey()
const curseforgeEnabled = computed(() => !!curseforgeApiKey.value)
const curseforgeCategories = ref<CurseforgeCategory[]>([])

async function searchCurseforge(requestParams: string) {
	if (!curseforgeEnabled.value) {
		return { projectHits: [], serverHits: [], total_hits: 0, per_page: 20 }
	}
	debugLog('searching curseforge', requestParams)
	try {
		const response = (await queryClient.fetchQuery({
			queryKey: ['search', 'curseforge', requestParams],
			queryFn: () => get_curseforge_search_results(requestParams, 'must_revalidate'),
			staleTime: 30_000,
		})) as {
			projectHits: Labrinth.Search.v3.ResultSearchProject[]
			serverHits: unknown[]
			total_hits: number
			per_page: number
		} | null

		return response ?? { projectHits: [], serverHits: [], total_hits: 0, per_page: 20 }
	} catch (err) {
		console.error('Failed to search CurseForge:', err)
		handleError(err as Error)
		return { projectHits: [], serverHits: [], total_hits: 0, per_page: 20 }
	}
}

const searchState = useBrowseSearch({
	projectType,
	tags,
	active: browseRouteActive,
	providedFilters: combinedProvidedFilters,
	search,
	searchCurseforge,
	curseforgeCategories,
	persistentQueryParams: ['i', 'ai', 'from'],
	getExtraQueryParams: () => ({
		ai: instanceHideInstalled.value ? 'true' : undefined,
	}),
})

watch(
	[
		() => searchState.query.value,
		() =>
			searchState.isServerType.value
				? searchState.serverCurrentFilters.value
				: searchState.currentFilters.value,
		() => projectType.value,
	],
	() => {
		if (instance.value || projectType.value === 'modpack') {
			syncHiddenInstanceProjectIds()
		}
	},
	{ deep: true },
)

if (instance.value?.game_version) {
	const gv = instance.value.game_version
	const alreadyHasGv = searchState.serverCurrentFilters.value.some(
		(f) => f.type === 'server_game_version' && f.option === gv,
	)
	if (!alreadyHasGv) {
		searchState.serverCurrentFilters.value.push({ type: 'server_game_version', option: gv })
	}
}

void searchState.refreshSearch()

const NON_FILTER_QUERY_KEYS = new Set(['i', 'ai', 'from', 'source', 'page', 'm', 'q', 'ss', 'cs'])

function hasExplicitFilterParams(): boolean {
	return Object.keys(route.query).some((key) => !NON_FILTER_QUERY_KEYS.has(key))
}

function toPinnedTab(filter: { type: string; option: string; negative?: boolean }) {
	return { filter_type: filter.type, option: filter.option, negative: !!filter.negative }
}

function pinnedTabsKey(source: string): string {
	return `${source}:${projectType.value}`
}

let pinnedTabsSaveTimer: ReturnType<typeof setTimeout> | null = null

async function savePinnedTabs() {
	if (projectType.value === 'server') return
	try {
		const settings = await getSettings()
		const tabs = settings.pinned_browse_tabs ?? {}
		const providedModrinthTypes = new Set(combinedProvidedFilters.value.map((f) => f.type))
		const providedCfTypes = new Set(
			mapProvidedFiltersToCf(combinedProvidedFilters.value).map((f) => f.type),
		)
		tabs[pinnedTabsKey('modrinth')] = searchState.currentFilters.value
			.filter((f) => !providedModrinthTypes.has(f.type))
			.map(toPinnedTab)
		tabs[pinnedTabsKey('curseforge')] = searchState.curseforgeCurrentFilters.value
			.filter((f) => !providedCfTypes.has(f.type))
			.map(toPinnedTab)
		settings.pinned_browse_tabs = tabs
		await setSettings(settings)
	} catch (err) {
		handleError(err)
	}
}

watch(
	[
		() => searchState.currentFilters.value,
		() => searchState.curseforgeCurrentFilters.value,
		projectType,
	],
	() => {
		if (pinnedTabsSaveTimer) clearTimeout(pinnedTabsSaveTimer)
		pinnedTabsSaveTimer = setTimeout(savePinnedTabs, 800)
	},
	{ deep: true },
)

if (projectType.value !== 'server') {
	getSettings()
		.then((settings) => {
			if (hasExplicitFilterParams()) return
			const isCf = searchState.isCfSource.value
			const source = isCf ? 'curseforge' : 'modrinth'
			const pinned = settings.pinned_browse_tabs?.[pinnedTabsKey(source)] ?? []
			const providedTypes = new Set(
				(isCf
					? mapProvidedFiltersToCf(combinedProvidedFilters.value)
					: combinedProvidedFilters.value
				).map((f) => f.type),
			)
			const target = isCf
				? searchState.curseforgeCurrentFilters.value
				: searchState.currentFilters.value
			for (const tab of pinned) {
				if (providedTypes.has(tab.filter_type)) continue
				if (target.some((f) => f.type === tab.filter_type && f.option === tab.option)) continue
				target.push({
					type: tab.filter_type,
					option: tab.option,
					negative: tab.negative || undefined,
				})
			}
		})
		.catch(handleError)
}

watch(
	curseforgeEnabled,
	async (enabled, previouslyEnabled) => {
		if (!enabled) return
		if (route.query.source === 'curseforge' && projectType.value !== 'server') {
			searchState.switchSource('curseforge')
		}
		try {
			curseforgeCategories.value =
				((await get_curseforge_categories('must_revalidate')) as {
					id: number
					name: string
					iconUrl: string | null
					classId: number | null
					parentCategoryId: number | null
					isClass: boolean
				}[]) ?? []
		} catch (err) {
			handleError(err)
		}
		if (searchState.isCfSource.value && previouslyEnabled === false) {
			void searchState.refreshSearch()
		}
	},
	{ immediate: true },
)

useAppEvent('instance', async (event) => {
	if (event.event === 'created' || event.event === 'removed') {
		if (!route.query.i) {
			await refreshInstalledProjectIds()
			if (projectType.value === 'modpack') {
				if (event.event === 'removed') {
					syncHiddenInstanceProjectIds()
				}
				await searchState.refreshSearch()
			}
		}
	}

	if (instance.value && event.instance_id === instance.value.id && event.event === 'synced') {
		await refreshInstalledProjectIds()
		await searchState.refreshSearch()
	}
})

function getProjectBrowseQuery() {
	if (!browseRouteActive.value) {
		return undefined
	}
	if (!installContext.value) return undefined
	return {
		...route.query,
		b: route.fullPath,
	}
}

const advancedFiltersCollapsed = computed({
	get: () => appSettings.getFeatureFlag('advanced_filters_collapsed'),
	set: (value) => {
		appSettings.featureFlags['advanced_filters_collapsed'] = value
		getSettings()
			.then((settings) => {
				settings.feature_flags['advanced_filters_collapsed'] = value
				return setSettings(settings)
			})
			.catch(handleError)
	},
})

const dismissedPhotosensitivityFilterWarning = computed({
	get: () => appSettings.getFeatureFlag('dismissed_photosensitivity_filter_warning'),
	set: (value) => {
		appSettings.featureFlags['dismissed_photosensitivity_filter_warning'] = value
		getSettings()
			.then((settings) => {
				settings.feature_flags['dismissed_photosensitivity_filter_warning'] = value
				return setSettings(settings)
			})
			.catch(handleError)
	},
})

const filterAccordionState = ref<Record<string, boolean>>({})

watch(
	() => route.params.projectType,
	(newType) => {
		if (newType !== 'server' || !searchState.isCfSource.value) return
		searchState.switchSource('modrinth')
		if (route.query.source) {
			router.replace({
				path: route.path,
				query: { ...route.query, source: undefined },
			})
		}
	},
	{ immediate: true },
)

provideBrowseManager({
	tags,
	projectType,
	...searchState,
	advancedFiltersCollapsed,
	dismissedPhotosensitivityFilterWarning,
	getProjectLink: (result: Labrinth.Search.v3.ResultSearchProject) => ({
		path: `/project/${result.project_id ?? result.slug}`,
		query: getProjectBrowseQuery(),
	}),
	getServerProjectLink: (result: Labrinth.Search.v3.ResultSearchProject) => ({
		path: `/project/${result.slug ?? result.project_id}`,
		query: getProjectBrowseQuery(),
	}),
	selectableProjectTypes,
	showProjectTypeTabs: computed(() => true),
	variant: 'app',
	curseforgeAvailable: curseforgeEnabled,
	getCardActions,
	installContext,
	providedFilters: combinedProvidedFilters,
	hideInstalled: computed({
		get: () => {
			if (projectType.value === 'modpack') return hideInstalledModpacks.value
			return instanceHideInstalled.value
		},
		set: (val: boolean) => {
			if (projectType.value === 'modpack') {
				hideInstalledModpacks.value = val
				if (val) syncHiddenInstanceProjectIds()
				return
			}
			instanceHideInstalled.value = val
			if (val) syncHiddenInstanceProjectIds()
		},
	}),
	showHideInstalled: computed(
		() => !searchState.isCfSource.value && (projectType.value === 'modpack' || !!instance.value),
	),
	hideInstalledLabel: computed(() =>
		formatMessage(
			isFromWorlds.value
				? messages.hideAddedServers
				: projectType.value === 'modpack'
					? messages.hideInstalledModpacks
					: commonMessages.hideInstalledContentLabel,
		),
	),
	onInstalled: onSearchResultInstalled,
	filterAccordionState,
	serverPings,
	getServerModpackContent,
	onContextMenu: handleRightClick,
	offline,
	lockedFilterMessages,
})
</script>

<template>
	<div
		class="flex flex-col gap-2 p-6"
		:class="{ 'cf-source-accent': searchState.isCfSource.value }"
	>
		<BrowsePageLayout>
			<template #after>
				<ContextMenu ref="contextMenuRef" :label="formatMessage(messages.projectActionsLabel)">
					<template #open_link="{ option }">
						<GlobeIcon /> {{ option.label }} <ExternalIcon />
					</template>
				</ContextMenu>
			</template>
		</BrowsePageLayout>
		<Teleport v-if="browseRouteActive" to="#sidebar-teleport-target">
			<BrowseSidebar />
		</Teleport>
	</div>
</template>
