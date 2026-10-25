<script setup lang="ts">
import { onMounted, ref } from "vue";
import { storeToRefs } from "pinia";
import { useRouter } from "vue-router";
import type { ComicPlatform } from "../api/comics";
import ComicSearchBar from "../components/ComicSearchBar.vue";
import ComicSearchResults from "../components/ComicSearchResults.vue";
import HomeIcon from "../components/HomeIcon.vue";
import { useAppI18n } from "../i18n";
import { useSearchStore } from "../stores/search";

const router = useRouter();
const { t } = useAppI18n();
const search = useSearchStore();
const { page, loading, response, error } = storeToRefs(search);

// The search bar starts from the active session. Editing it only changes the
// draft; the store and results update once the user submits or picks a
// platform, matching the old in-place search behavior.
const draftPlatform = ref<ComicPlatform>(search.platform);
const draftQuery = ref(search.query);

const homeReady = ref(false);
onMounted(() => {
  // The trailing home control is inserted after the initial render so it can
  // expand into the pill and push the search button left.
  homeReady.value = true;
});

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
  if (!homeReady.value) {
    return;
  }
  // Collapse first, then navigate: this reverses the entry animation before
  // the home page (which has no trailing control) takes over.
  homeReady.value = false;
}

function finishGoHome() {
  // This route has no URL state to preserve and is only an in-session
  // results view, so the home control always returns to the home page.
  void router.push({ name: "home" });
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
          <Transition name="home-pop" @after-leave="finishGoHome">
            <span v-if="homeReady" class="home-control">
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
          </Transition>
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
  overflow: hidden;
}

.home-button {
  flex: none;
  transform-origin: center;
  transition: transform 220ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.home-pop-enter-from,
.home-pop-leave-to {
  width: 0;
  opacity: 0;
}

.home-pop-enter-from .home-button,
.home-pop-leave-to .home-button {
  transform: scale(0.75);
}

.home-pop-enter-active,
.home-pop-leave-active {
  transition:
    width 220ms cubic-bezier(0.2, 0.8, 0.2, 1),
    opacity 140ms ease;
  overflow: hidden;
}

.home-pop-enter-to,
.home-pop-leave-from {
  width: 40px;
  opacity: 1;
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

@media (prefers-reduced-motion: reduce) {
  .home-control,
  .home-button {
    transition-duration: 0.01ms;
  }

  .home-pop-enter-active,
  .home-pop-leave-active {
    transition-duration: 0.01ms;
  }
}
</style>
