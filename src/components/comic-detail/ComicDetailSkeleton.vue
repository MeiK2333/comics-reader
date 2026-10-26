<script setup lang="ts">
import { useAppI18n } from "../../i18n";

const { t } = useAppI18n();
</script>

<template>
  <div
    class="detail-skeleton"
    role="status"
    aria-busy="true"
    :aria-label="t('detail.loading')"
  >
    <div class="detail-skeleton__hero" aria-hidden="true">
      <NSkeleton class="detail-skeleton__cover" />
      <div class="detail-skeleton__summary">
        <NSkeleton round width="5rem" height="1.5rem" />
        <NSkeleton
          class="detail-skeleton__title"
          :sharp="false"
        />
        <NSkeleton
          class="detail-skeleton__subtitle"
          :sharp="false"
        />
        <div class="detail-skeleton__chips">
          <NSkeleton
            v-for="index in 4"
            :key="index"
            round
            width="4rem"
            height="1.5rem"
          />
        </div>
      </div>
    </div>

    <div class="detail-skeleton__body" aria-hidden="true">
      <section class="detail-skeleton__section">
        <NSkeleton
          class="detail-skeleton__heading"
          :sharp="false"
        />
        <NSkeleton width="100%" height="1rem" :sharp="false" />
        <NSkeleton width="92%" height="1rem" :sharp="false" />
        <NSkeleton width="68%" height="1rem" :sharp="false" />
      </section>

      <section class="detail-skeleton__section">
        <NSkeleton
          class="detail-skeleton__heading"
          :sharp="false"
        />
        <div class="detail-skeleton__chips">
          <NSkeleton
            v-for="index in 5"
            :key="index"
            round
            width="4.5rem"
            height="1.5rem"
          />
        </div>
      </section>

      <section class="detail-skeleton__section">
        <NSkeleton
          class="detail-skeleton__heading"
          :sharp="false"
        />
        <div class="detail-skeleton__chapter-grid">
          <NSkeleton
            v-for="index in 8"
            :key="index"
            class="detail-skeleton__chapter"
          />
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.detail-skeleton__hero {
  display: grid;
  grid-template-columns: minmax(180px, 240px) minmax(0, 1fr);
  gap: 1.5rem;
  align-items: start;
}

.detail-skeleton__cover {
  display: block;
  width: 100%;
  height: auto;
  aspect-ratio: 3 / 4;
  border-radius: 12px;
}

.detail-skeleton__summary {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.7rem;
  min-width: 0;
  padding-top: 0.1rem;
}

.detail-skeleton__title {
  width: 72%;
  height: 2rem;
  margin-top: 0.15rem;
}

.detail-skeleton__subtitle {
  width: 42%;
  height: 1rem;
}

.detail-skeleton__chips {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
}

.detail-skeleton__body {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  margin-top: 1.75rem;
}

.detail-skeleton__section {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.detail-skeleton__heading {
  width: 6rem;
  height: 1rem;
}

.detail-skeleton__chapter-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(8.5rem, 1fr));
  gap: 0.4rem;
}

.detail-skeleton__chapter {
  width: 100%;
  height: 34px;
}

@media (max-width: 640px) {
  .detail-skeleton__hero {
    grid-template-columns: 1fr;
    gap: 1rem;
  }

  .detail-skeleton__cover {
    width: min(220px, 60%);
    margin: 0 auto;
    aspect-ratio: 3 / 4;
  }

  .detail-skeleton__summary {
    padding-top: 0;
  }
}
</style>