<script setup lang="ts">
withDefaults(defineProps<{
  label: string
  detail?: string
  variant?: "standard" | "primary"
  disabled?: boolean
}>(), { variant: "standard", disabled: false })
</script>

<template>
  <button class="action-button" :class="`action-button--${variant}`" :disabled="disabled" type="button">
    <span v-if="$slots.icon" class="action-button__icon" aria-hidden="true"><slot name="icon" /></span>
    <span class="action-button__text">
      <span class="action-button__label">{{ label }}</span>
      <span v-if="detail" class="action-button__detail">{{ detail }}</span>
    </span>
    <span v-if="variant === 'standard'" class="action-button__led" aria-hidden="true" />
  </button>
</template>
