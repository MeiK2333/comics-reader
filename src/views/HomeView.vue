<script setup lang="ts">
import ComicSearchBar from "../components/ComicSearchBar.vue";
import ComicSearchResults from "../components/ComicSearchResults.vue";
import { useComicSearch } from "../composables/useComicSearch";

const {
  platform,
  query,
  page,
  loading,
  response,
  error,
  hasSearched,
  search,
  selectPlatform,
  changePage,
} = useComicSearch();
</script>

<template>
  <main class="home" :class="{ 'has-results': hasSearched }">
    <section class="hero">
      <ComicSearchBar
        v-model:query="query"
        :platform="platform"
        :loading="loading"
        @select-platform="selectPlatform"
        @search="search"
      />
    </section>

    <ComicSearchResults
      v-if="hasSearched"
      :loading="loading"
      :page="page"
      :response="response"
      :error="error"
      @update:page="changePage"
    />
  </main>
</template>

<style scoped>
.home {
  /* Account for the horizontal padding so a narrow window does not add a
     page-level horizontal scrollbar. */
  width: min(1100px, calc(100% - 2.5rem));
  margin: 0 auto;
  padding: 2rem 1.25rem 4rem;
}

.hero {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  min-height: 55vh;
  text-align: center;
}

.home.has-results {
  padding-top: 1rem;
  /* Keep the pagination close to the window edge instead of leaving a large
     empty band under the last result row. */
  padding-bottom: 1.5rem;
}

.home.has-results .hero {
  align-items: flex-start;
  min-height: 0;
  gap: 0.5rem;
  padding-top: 0.25rem;
  text-align: left;
}

@media (max-width: 560px) {
  .home {
    padding-top: 1rem;
  }

  .hero {
    min-height: 45vh;
  }
}
</style>
