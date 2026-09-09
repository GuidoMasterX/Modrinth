import { createSharedComposable, useMagicKeys } from '@vueuse/core'

export const useShiftKey = createSharedComposable(() => useMagicKeys().shift)
