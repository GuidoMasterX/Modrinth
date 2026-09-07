import { ref } from 'vue'

import { get as getSettings } from '@/helpers/settings'

const curseforgeApiKey = ref<string | null>(null)
let loaded = false

export function useCurseforgeKey() {
	if (!loaded) {
		loaded = true
		getSettings()
			.then((settings) => {
				curseforgeApiKey.value = settings.curseforge_api_key
			})
			.catch(() => {
				loaded = false
			})
	}
	return curseforgeApiKey
}

export function setCurseforgeKey(value: string | null) {
	curseforgeApiKey.value = value
}
