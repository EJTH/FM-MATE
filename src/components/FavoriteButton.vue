<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    favorited: boolean;
    label?: string;
  }>(),
  { label: "" },
);

const emit = defineEmits<{
  "update:favorited": [value: boolean];
}>();

const toggleFavorite = () => {
  emit("update:favorited", !props.favorited);
};
</script>

<template>
  <button
    class="favorite-button"
    :class="{ 'is-favorite': props.favorited }"
    type="button"
    :aria-pressed="props.favorited"
    :aria-label="
      props.favorited
        ? `Remove ${props.label} from favorites`
        : `Add ${props.label} to favorites`
    "
    :title="props.favorited ? 'Remove favorite' : 'Add favorite'"
    @click.stop="toggleFavorite"
  >
    {{ props.favorited ? "♥" : "♡" }}
  </button>
</template>
