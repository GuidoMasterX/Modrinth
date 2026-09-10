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

const props = defineProps<{
	projectId: string
}>()

const { formatMessage } = useVIntl()
const { labrinth } = injectModrinthClient()

const { data } = useQuery({
	queryKey: computed(() => ['project', props.projectId, 'modpack-dependents'] as const),
	queryFn: async () => {
		const results = await labrinth.projects_v3.search({
			limit: 10,
			index: 'downloads',
			filters: buildDependentsSearchFilters(['modpack'], [props.projectId]),
		})
		return results.hits
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
