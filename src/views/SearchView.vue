<script setup lang="ts">
import { ref } from "vue";
import { storeToRefs } from "pinia";
import type { ComicPlatform } from "../api/comics";
import ComicSearchBar from "../components/ComicSearchBar.vue";
import ComicSearchResults from "../components/ComicSearchResults.vue";
import HomeIcon from "../components/HomeIcon.vue";
import { useBackNavigation } from "../composables/useBackNavigation";
import { useAppI18n } from "../i18n";
import { useSearchStore } from "../stores/search";

const { goBack } = useBackNavigation();
const { t } = useAppI18n();
const search = useSearchStore();
const { page, loading, response, error } = storeToRefs(search);

// The search bar starts from the active session. Editing it only changes the
// draft; the store and results update once the user submits or picks a
// platform, matching the old in-place search behavior.
const draftPlatform = ref<ComicPlatform>(search.platform);
const draftQuery = ref(search.query);

function submit() {
  const text = draftQuery.value.trim();
  if (!text) {
    return;
  }

  draftQuery.value = text;
  search.beginSearch(draftPlatform.value, text);
}

function selectPlatform(next: ComicPlatform) {
  draftPlatform.value = next;
  const text = draftQuery.value.trim();
  if (!text) {
    return;
  }

  draftQuery.value = text;
  search.beginSearch(next, text);
}

function goHome() {
  // Search is a child of Home. Pop rather than push so the browser stack does
  // not keep an empty results page under the home screen.
  goBack({ name: "home" });
}
</script>

<template>
  <main class="search-page">
    <header class="search-header">
      <ComicSearchBar
        v-model:query="draftQuery"
        :platform="draftPlatform"
        :loading="loading"
        @select-platform="selectPlatform"
        @search="submit"
      >
        <template #trailing>
          <span class="home-control">
            <NButton
              attr-type="button"
              type="primary"
              quaternary
              circle
              size="large"
              class="home-button"
              :aria-label="t('search.home')"
              @click="goHome"
            >
              <template #icon>
                <HomeIcon />
              </template>
            </NButton>
          </span>
        </template>
      </ComicSearchBar>
    </header>

    <ComicSearchResults
      :loading="loading"
      :page="page"
      :response="response"
      :error="error"
      @update:page="search.changePage"
    />
  </main>
</template>

<style scoped>
.search-page {
  /* Match the home page width so moving between the two does not shift the
     search bar or result grid. */
  width: min(1100px, calc(100% - 2.5rem));
  margin: 0 auto;
  padding: 1rem 1.25rem 1.5rem;
}

.search-header {
  max-width: 760px;
  margin: 0 auto;
}

.home-control {
  display: flex;
  flex: none;
  view-transition-name: search-home;
}

.home-button {
  flex: none;
}

.search-header :deep(.search-form) {
  width: 100%;
  margin-top: 0;
}

@media (max-width: 560px) {
  .search-page {
    padding-top: 0.75rem;
  }
}
</style>
