<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  /** Battery percentage, 0–100. */
  level: number;
  /** Localized accessible label, e.g. "Battery 85%". */
  label: string;
}>();

const fillWidth = computed(() =>
  Math.max(1, Math.min(14, Math.round((props.level / 100) * 14))),
);
</script>

<template>
  <span class="reader-battery" role="img" :aria-label="label" :title="label">
    <svg
      class="reader-battery__icon"
      viewBox="0 0 24 13"
      aria-hidden="true"
      focusable="false"
    >
      <rect
        x="1"
        y="1"
        width="18"
        height="11"
        rx="2.5"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
      />
      <rect
        x="3"
        y="3"
        :width="fillWidth"
        height="7"
        rx="1"
        fill="currentColor"
      />
      <rect x="20" y="4" width="2" height="5" rx="1" fill="currentColor" />
    </svg>
    <span class="reader-battery__value">{{ level }}%</span>
  </span>
</template>

<style scoped>
.reader-battery {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 0.25rem;
  font-size: 0.78rem;
  font-variant-numeric: tabular-nums;
  line-height: 1;
}

.reader-battery__icon {
  display: block;
  width: 22px;
  height: 12px;
}

.reader-battery__value {
  opacity: 0.8;
}
</style>
