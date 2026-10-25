<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import type { ComicPlatform } from "../api/comics";
import ComicSearchBar from "../components/ComicSearchBar.vue";
import { useSearchStore } from "../stores/search";

const router = useRouter();
const search = useSearchStore();

// Keep the last query when coming back from the results page, but only the
// form values: results are rendered exclusively by the search route.
const platform = ref<ComicPlatform>(search.platform);
const query = ref(search.query);

function selectPlatform(next: ComicPlatform) {
  platform.value = next;
}

function submit() {
  if (!query.value.trim()) {
    return;
  }

  search.beginSearch(platform.value, query.value);
  void router.push({ name: "search" });
}
</script>

<template>
  <main class="home">
    <section class="hero">
      <ComicSearchBar
        v-model:query="query"
        :platform="platform"
        :loading="false"
        @select-platform="selectPlatform"
        @search="submit"
      />
    </section>
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

@media (max-width: 560px) {
  .home {
    padding-top: 1rem;
  }

  .hero {
    min-height: 45vh;
  }
}
</style>
