<template>
	<div v-if="modpacks.length > 0" class="flex flex-col gap-3">
		<h2 class="text-lg m-0">{{ formatMessage(messages.title) }}</h2>
		<div class="flex flex-col gap-3">
			<AutoLink
				v-for="modpack in modpacks"
				:key="modpack.project_id"
				:to="`/project/${modpack.project_id}`"
				class="flex w-fit items-center gap-2 text-primary leading-[1.2] hover:underline"
			>
				<Avatar :src="modpack.icon_url" size="24px" class="shrink-0" no-shadow />
				<span class="font-semibold">{{ modpack.name }}</span>
			</AutoLink>
		</div>
		<button
			v-if="showToggle"
			class="flex bg-transparent text-secondary border-none cursor-pointer !w-full items-center gap-2 truncate rounded-xl px-2 py-1 text-sm font-semibold transition-all hover:text-contrast focus-visible:text-contrast active:scale-[0.98]"
			@click="toggleExpand"
		>
			<DropdownIcon class="h-4 w-4 transition-transform" :class="{ 'rotate-180': expanded }" />
			<span class="truncate text-sm">
				{{ expanded ? formatMessage(messages.showFewer) : formatMessage(messages.showMore) }}
			</span>
		</button>
	</div>
</template>

<script setup lang="ts">
import { DropdownIcon } from '@modrinth/assets'
import { useQuery } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import { defineMessages, useVIntl } from '../../composables/i18n'
import { injectModrinthClient } from '../../providers'
import { buildDependentsSearchFilters } from '../../utils/search'
import { AutoLink, Avatar } from '../base'

interface CfDependent {
	id: number
	name: string
	logoUrl?: string | null
	downloads?: number
	updateDate?: number
	categoryClass?: { id?: number } | null
}

interface CfDependentsResponse {
	data?: CfDependent[]
}

interface ModpackRow {
	project_id: string
	name: string
	icon_url?: string | null
}

interface ModpackResults {
	rows: ModpackRow[]
	hasMore: boolean
}

const MR_TOP_LIMIT = 5
const MR_EXPANDED_LIMIT = 50
const CF_CLASS_MODPACK = 4471
const CF_TOP = 5
const CF_PAGE_SIZE = 100
const CF_INITIAL_PAGES = 3
const CF_EXPANDED_PAGES = 10

const props = defineProps<{
	projectId: string
}>()

const { formatMessage } = useVIntl()
const client = injectModrinthClient()

const isCf = computed(() => props.projectId.startsWith('cf-'))

const limit = ref(MR_TOP_LIMIT)
const cfPages = ref(CF_INITIAL_PAGES)
const expanded = ref(false)

const { data } = useQuery({
	queryKey: computed(
		() =>
			[
				'project',
				props.projectId,
				'modpack-dependents',
				isCf.value ? `cf-${cfPages.value}` : limit.value,
			] as const,
	),
	queryFn: async (): Promise<ModpackResults> => {
		try {
			if (isCf.value) {
				const cfId = props.projectId.slice('cf-'.length)
				const dependents: CfDependent[] = []
				let fetchedAll = false
				// The website API paginates with a 0-based `page` and `size`
				// (default 20, max 146); entries are recency-ordered, so more
				// pages mean a more accurate downloads ranking.
				for (let page = 0; page < cfPages.value; page++) {
					const response = await client.request<CfDependentsResponse>(`/mods/${cfId}/dependents`, {
						api: 'https://www.curseforge.com/api',
						version: 'v1',
						skipAuth: true,
						params: { page, size: CF_PAGE_SIZE },
						headers: { 'User-Agent': 'ModrinthApp/1.0' },
					})
					const rawRows = response.data ?? []
					dependents.push(
						...rawRows.filter((dependent) => dependent.categoryClass?.id === CF_CLASS_MODPACK),
					)
					if (rawRows.length < CF_PAGE_SIZE) {
						fetchedAll = true
						break
					}
				}
				dependents.sort(
					(a, b) =>
						(b.downloads ?? 0) - (a.downloads ?? 0) ||
						(b.updateDate ?? 0) - (a.updateDate ?? 0) ||
						a.name.localeCompare(b.name) ||
						a.id - b.id,
				)
				return {
					rows: dependents.map((dependent) => ({
						project_id: `cf-${dependent.id}`,
						name: dependent.name,
						icon_url: dependent.logoUrl ?? null,
					})),
					hasMore: !fetchedAll && cfPages.value < CF_EXPANDED_PAGES,
				}
			}
			const results = await client.labrinth.projects_v3.search({
				limit: limit.value,
				index: 'downloads',
				filters: buildDependentsSearchFilters(['modpack'], [props.projectId]),
			})
			return {
				rows: results.hits.map((hit) => ({
					project_id: hit.project_id,
					name: hit.name,
					icon_url: hit.icon_url ?? null,
				})),
				hasMore: (results.total_hits ?? 0) > limit.value,
			}
		} catch {
			return { rows: [], hasMore: false }
		}
	},
	staleTime: 1000 * 60 * 30,
})

const modpacks = computed(() => {
	const rows = data.value?.rows ?? []
	if (isCf.value && !expanded.value) {
		return rows.slice(0, CF_TOP)
	}
	return rows
})

const hasMore = computed(() => {
	const results = data.value
	if (!results) return false
	if (isCf.value) {
		return !expanded.value && (results.hasMore || results.rows.length > CF_TOP)
	}
	return results.hasMore
})

const showToggle = computed(() => hasMore.value || expanded.value)

function toggleExpand() {
	if (!expanded.value) {
		if (isCf.value) {
			if (cfPages.value < CF_EXPANDED_PAGES) {
				cfPages.value = CF_EXPANDED_PAGES
			}
		} else {
			limit.value = MR_EXPANDED_LIMIT
		}
		expanded.value = true
	} else {
		expanded.value = false
	}
}

const messages = defineMessages({
	title: {
		id: 'project.about.modpacks.title',
		defaultMessage: 'Modpacks',
	},
	showMore: {
		id: 'search.filter.option.show_more',
		defaultMessage: 'Show more',
	},
	showFewer: {
		id: 'search.filter.option.show_fewer',
		defaultMessage: 'Show fewer',
	},
})
</script>
