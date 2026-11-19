<script setup lang="ts">
import { archiveState } from "../services/firmwareArchive";

const props = defineProps<{
  selected: string;
  disabled?: boolean;
}>();
const emit = defineEmits<{ change: [commit: string] }>();

function change(event: Event) {
  const select = event.target as HTMLSelectElement;
  emit("change", select.value);
  select.value = props.selected;
}
</script>

<template>
  <label class="archive-control">
    <span>Archive date</span>
    <select
      :value="selected"
      :disabled="disabled || archiveState.loading"
      @change="change"
    >
      <option value="">Latest catalog</option>
      <option
        v-if="selected && !archiveState.commits.some((commit) => commit.sha === selected)"
        :value="selected"
      >
        Saved archive ({{ selected.slice(0, 7) }})
      </option>
      <option
        v-for="commit in archiveState.commits"
        :key="commit.sha"
        :value="commit.sha"
      >
        {{ new Date(commit.date).toLocaleDateString() }} ({{ commit.sha.slice(0, 7) }})
      </option>
    </select>
  </label>
</template>
