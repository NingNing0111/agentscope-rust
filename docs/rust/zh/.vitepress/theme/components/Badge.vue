<script setup lang="ts">
withDefaults(defineProps<{
  color?: 'green' | 'blue' | 'amber' | 'gray'
  size?: 'sm' | 'md'
}>(), { color: 'gray', size: 'md' })
</script>

<template>
  <span class="as-badge" :class="[`as-badge-${color}`, `as-badge-${size}`]"><slot /></span>
</template>
