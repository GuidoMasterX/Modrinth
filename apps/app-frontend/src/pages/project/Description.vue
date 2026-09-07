<template>
	<Card>
		<!-- eslint-disable-next-line vue/no-v-html -->
		<div
			v-if="descriptionHtml"
			class="markdown-body [&_img]:max-w-full [&_img]:h-auto [&_video]:max-w-full [&_iframe]:max-w-full [&_pre]:overflow-x-auto"
			v-html="descriptionHtml"
		/>
		<ProjectPageDescription v-else :description="project.body" />
	</Card>
</template>

<script setup>
import { Card, ProjectPageDescription } from '@modrinth/ui'
import DOMPurify from 'dompurify'
import { computed, ref, watch } from 'vue'

import { get_curseforge_description } from '@/helpers/cache.js'

const props = defineProps({
	project: {
		type: Object,
		default: () => {},
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
