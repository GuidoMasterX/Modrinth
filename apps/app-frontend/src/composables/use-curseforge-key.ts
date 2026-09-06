import { ref } from 'vue'

import { getSettings } from '@/helpers/settings'

const curseforgeApiKey = ref<string | null>(null)
let loaded = false

export function useCurseforgeKey() {
	if (!loaded) {
		loaded = true
		getSettings()
			.then((settings) => {
				curseforgeApiKey.value = settings.curseforge_api_key
			})
			.catch(() => {})
	}
	return curseforgeApiKey
}

export function setCurseforgeKey(value: string | null) {
	curseforgeApiKey.value = value
}
