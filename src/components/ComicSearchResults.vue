<script setup lang="ts">
import { computed } from "vue";
import type { ComicPlatform, ComicSearchResult } from "../api/comics";
import { useErrorMessage } from "../composables/useErrorMessage";
import type { AppError } from "../errors";
import { useAppI18n } from "../i18n";
import ComicResultCard from "./ComicResultCard.vue";

const props = defineProps<{
  platform: ComicPlatform;
  loading: boolean;
  page: number;
  response: ComicSearchResult | null;
  error: AppError | null;
}>();

const emit = defineEmits<{
  "update:page": [page: number];
}>();

const { t } = useAppI18n();
const errorText = useErrorMessage();

const errorMessage = computed(() =>
  props.error ? errorText(props.error) : "",
);

const pageCount = computed(() => {
  const current = props.response;
  if (!current) {
    return 1;
  }
  return Math.max(
    1,
    Math.ceil(current.total / Math.max(1, current.page_size)),
  );
});

function changePage(next: number) {
  if (next !== props.page) {
    emit("update:page", next);
  }
}
</script>

<template>
  <section class="results">
    <NAlert v-if="errorMessage" type="error" :bordered="false" class="status">
      {{ errorMessage }}
    </NAlert>

    <NSpin v-else-if="response" :show="loading">
      <div class="results-content">
        <div class="results-header">
          {{ t("home.search.total", { total: response.total }) }}
        </div>

        <NEmpty
          v-if="response.result.length === 0"
          class="status"
          :description="t('home.search.empty')"
        />

        <template v-else>
          <div class="result-grid">
            <ComicResultCard
              v-for="item in response.result"
              :key="item.id"
              :platform="platform"
              :item="item"
            />
          </div>

          <!-- Fewer slots keep the control row inside the result grid on
               narrow windows instead of overflowing the content margins. -->
          <NPagination
            v-if="pageCount > 1"
            class="pagination"
            :page="page"
            :page-count="pageCount"
            :page-slot="7"
            @update:page="changePage"
          />
        </template>
      </div>
    </NSpin>

    <div v-else class="status">
      <NSpin size="large" />
    </div>
  </section>
</template>

<style scoped>
.results {
  margin-top: 1.25rem;
}

.results-content {
  min-height: 8rem;
}

.results-header {
  margin-bottom: 0.75rem;
  font-size: 0.9rem;
  opacity: 0.7;
}

.result-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: 1rem;
}

.pagination {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
}

/* On narrow windows, stretch the control row to the grid's outer edges so the
   pagination keeps the same side margin as the result cards. */
@media (max-width: 480px) {
  .pagination {
    justify-content: space-between;
  }
}

.status {
  display: flex;
  justify-content: center;
  padding: 2rem 0;
}
</style>
