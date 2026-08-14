<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(defineProps<{ cols?: number }>(), { cols: 1 })
const style = computed(() => ({ '--as-card-cols': props.cols }))
</script>

<template>
  <div class="as-card-group" :style="style"><slot /></div>
</template>
