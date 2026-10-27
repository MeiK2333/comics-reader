<script setup lang="ts">
defineProps<{
  backLabel: string;
  loading: boolean;
}>();

const emit = defineEmits<{
  back: [];
}>();
</script>

<template>
  <header class="detail-topbar">
    <NButton
      attr-type="button"
      quaternary
      circle
      size="large"
      class="back-button"
      :aria-label="backLabel"
      :title="backLabel"
      @click="emit('back')"
    >
      <template #icon>
        <svg
          class="back-icon"
          viewBox="0 0 24 24"
          aria-hidden="true"
          focusable="false"
        >
          <path
            d="M15 5l-7 7 7 7"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </template>
    </NButton>

    <slot />

    <div v-if="loading" class="detail-topbar-skeleton" aria-hidden="true">
      <NSkeleton round width="96px" height="34px" />
      <NSkeleton round width="80px" height="34px" />
    </div>
  </header>
</template>

<style scoped>
.detail-topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  margin-bottom: 1rem;
}

.back-button {
  flex: none;
  margin-left: -0.35rem;
}

.back-icon {
  display: block;
  width: 20px;
  height: 20px;
}

.detail-topbar-skeleton {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.5rem;
  flex-wrap: wrap;
}
</style>
