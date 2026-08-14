<script setup lang="ts">
import { VPLink } from 'vitepress/theme'

withDefaults(defineProps<{
  title: string
  icon?: string
  href?: string
  cta?: string
}>(), { icon: undefined, href: undefined, cta: undefined })
</script>

<template>
  <VPLink v-if="href" class="as-card as-card-link" :href="href">
    <span v-if="icon" class="as-card-icon" aria-hidden="true">{{ icon }}</span>
    <strong class="as-card-title">{{ title }}</strong>
    <div class="as-card-content"><slot /></div>
    <span v-if="cta" class="as-card-cta">{{ cta }} →</span>
  </VPLink>
  <div v-else class="as-card">
    <span v-if="icon" class="as-card-icon" aria-hidden="true">{{ icon }}</span>
    <strong class="as-card-title">{{ title }}</strong>
    <div class="as-card-content"><slot /></div>
    <span v-if="cta" class="as-card-cta">{{ cta }}</span>
  </div>
</template>
