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
	</div>
</template>

<script setup lang="ts">
import { useQuery } from '@tanstack/vue-query'
import { computed } from 'vue'

import { defineMessages, useVIntl } from '../../composables/i18n'
import { injectModrinthClient } from '../../providers'
import { buildDependentsSearchFilters } from '../../utils/search'
import { AutoLink, Avatar } from '../base'

interface CfDependent {
	id: number
	name: string
	logoUrl?: string | null
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

const props = defineProps<{
	projectId: string
}>()

const { formatMessage } = useVIntl()
const client = injectModrinthClient()

const isCf = computed(() => props.projectId.startsWith('cf-'))

const { data } = useQuery({
	queryKey: computed(() => ['project', props.projectId, 'modpack-dependents'] as const),
	queryFn: async (): Promise<ModpackRow[]> => {
		try {
			if (isCf.value) {
				const cfId = props.projectId.slice('cf-'.length)
				const response = await client.request<CfDependentsResponse>(`/mods/${cfId}/dependents`, {
					api: 'https://www.curseforge.com/api',
					version: 'v1',
					skipAuth: true,
					params: { pageSize: 50, index: 0 },
					headers: { 'User-Agent': 'ModrinthApp/1.0' },
				})
				return (response.data ?? [])
					.filter((dependent) => dependent.categoryClass?.id === 4471)
					.map((dependent) => ({
						project_id: `cf-${dependent.id}`,
						name: dependent.name,
						icon_url: dependent.logoUrl ?? null,
					}))
			}
			const results = await client.labrinth.projects_v3.search({
				limit: 10,
				index: 'downloads',
				filters: buildDependentsSearchFilters(['modpack'], [props.projectId]),
			})
			return results.hits.map((hit) => ({
				project_id: hit.project_id,
				name: hit.name,
				icon_url: hit.icon_url ?? null,
			}))
		} catch {
			return []
		}
	},
	staleTime: 1000 * 60 * 30,
})

const modpacks = computed(() => data.value ?? [])

const messages = defineMessages({
	title: {
		id: 'project.about.modpacks.title',
		defaultMessage: 'Included in modpacks',
	},
})
</script>
