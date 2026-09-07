<template>
	<Card>
		<!-- eslint-disable-next-line vue/no-v-html -->
		<div v-if="descriptionHtml" v-html="descriptionHtml" />
		<ProjectPageDescription v-else :description="project.body" />
		<ButtonLink :href="cfProjectUrl(project)" target="_blank" class="mt-4">
			<ExternalIcon />
			<span>{{ formatMessage(messages.readOnCurseForge) }}</span>
		</ButtonLink>
	</Card>
</template>

<script setup>
import { ExternalIcon } from '@modrinth/assets'
import { ButtonLink, Card, defineMessages, ProjectPageDescription, useVIntl } from '@modrinth/ui'
import DOMPurify from 'dompurify'
import { computed, ref, watch } from 'vue'

import { get_curseforge_description } from '@/helpers/cache.js'
import { cfProjectUrl } from '@/helpers/curseforge-project'

const props = defineProps({
	project: {
		type: Object,
		default: () => {},
	},
})

const { formatMessage } = useVIntl()

const messages = defineMessages({
	readOnCurseForge: {
		id: 'app.project.description.read-on-curseforge',
		defaultMessage: 'Read more on CurseForge',
	},
})

const rawDescription = ref(null)
let descriptionRequest = 0

watch(
	() => props.project.id,
	async (id) => {
		rawDescription.value = null
		const request = ++descriptionRequest
		if (!id?.startsWith('cf-')) return
		try {
			const html = await get_curseforge_description(id)
			if (request === descriptionRequest) {
				rawDescription.value = html
			}
		} catch {
			// description unavailable - fall back to the summary
		}
	},
	{ immediate: true },
)

const descriptionHtml = computed(() =>
	rawDescription.value ? DOMPurify.sanitize(rawDescription.value) : null,
)
</script>

<script>
export default {
	name: 'Description',
}
</script>
