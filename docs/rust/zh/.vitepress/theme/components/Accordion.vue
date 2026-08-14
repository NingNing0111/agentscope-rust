<script setup lang="ts">
import { ref, useId } from 'vue'

defineProps<{ title: string }>()
const open = ref(false)
const panelId = `as-accordion-${useId()}`
</script>

<template>
  <section class="as-accordion">
    <button
      class="as-accordion-button"
      type="button"
      :aria-expanded="open"
      :aria-controls="panelId"
      @click="open = !open"
    >
      <span>{{ title }}</span>
      <span aria-hidden="true">{{ open ? '−' : '+' }}</span>
    </button>
    <div :id="panelId" v-show="open" class="as-accordion-panel">
      <slot />
    </div>
  </section>
</template>
