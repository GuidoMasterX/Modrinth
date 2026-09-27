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
		<Button v-if="hasMore" type="quiet" class="w-fit !text-sm" @click="showMore">
			{{ formatMessage(messages.showMore) }}
		</Button>
	</div>
</template>

<script setup lang="ts">
import { useQuery } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import { defineMessages, useVIntl } from '../../composables/i18n'
import { injectModrinthClient } from '../../providers'
import { buildDependentsSearchFilters } from '../../utils/search'
import { AutoLink, Avatar, Button } from '../base'

interface CfDependent {
	id: number
	name: string
	logoUrl?: string | null
	downloads?: number
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
const CF_TOP = 5
const CF_INITIAL_PAGES = 2
const CF_EXPANDED_PAGES = 5

const props = defineProps<{
	projectId: string
}>()

const { formatMessage } = useVIntl()
const client = injectModrinthClient()

const isCf = computed(() => props.projectId.startsWith('cf-'))

const limit = ref(MR_TOP_LIMIT)
const cfPages = ref(CF_INITIAL_PAGES)
const showAllCf = ref(false)

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
				for (let page = 1; page <= cfPages.value; page++) {
					const response = await client.request<CfDependentsResponse>(`/mods/${cfId}/dependents`, {
						api: 'https://www.curseforge.com/api',
						version: 'v1',
						skipAuth: true,
						params: { page },
						headers: { 'User-Agent': 'ModrinthApp/1.0' },
					})
					const pageRows = (response.data ?? []).filter(
						(dependent) => dependent.categoryClass?.id === 4471,
					)
					dependents.push(...pageRows)
					if (pageRows.length < 20) {
						fetchedAll = true
						break
					}
				}
				dependents.sort((a, b) => (b.downloads ?? 0) - (a.downloads ?? 0))
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
	if (isCf.value && !showAllCf.value) {
		return rows.slice(0, CF_TOP)
	}
	return rows
})

const hasMore = computed(() => {
	const results = data.value
	if (!results) return false
	if (isCf.value) {
		return !showAllCf.value && (results.hasMore || results.rows.length > CF_TOP)
	}
	return results.hasMore
})

function showMore() {
	if (isCf.value) {
		if (cfPages.value < CF_EXPANDED_PAGES) {
			cfPages.value = CF_EXPANDED_PAGES
		}
		showAllCf.value = true
	} else {
		limit.value = MR_EXPANDED_LIMIT
	}
}

const messages = defineMessages({
	title: {
		id: 'project.about.modpacks.title',
		defaultMessage: 'Modpacks',
	},
	showMore: {
		id: 'project.about.modpacks.showMore',
		defaultMessage: 'Show more',
	},
})
</script>
