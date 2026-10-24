<script setup lang="ts">
import { computed, h, ref } from "vue";
import type { DropdownOption } from "naive-ui";
import {
  searchComics,
  type ComicPlatform,
  type ComicSearchItem,
  type ComicSearchResult,
} from "../api/comics";
import manhuaguiIcon from "../assets/platforms/manhuagui.svg";
import jmIcon from "../assets/platforms/jm.png";
import nhentaiIcon from "../assets/platforms/nhentai.svg";
import ComicThumb from "../components/ComicThumb.vue";
import SearchIcon from "../components/SearchIcon.vue";
import { useErrorMessage } from "../composables/useErrorMessage";
import { toAppError } from "../errors";
import type { AppError } from "../errors";
import { useAppI18n } from "../i18n";
import type { MessageKey } from "../i18n";

const { t } = useAppI18n();
const errorText = useErrorMessage();

const platforms: ReadonlyArray<{
  icon: string;
  nameKey: MessageKey;
  value: ComicPlatform;
}> = [
  {
    icon: manhuaguiIcon,
    nameKey: "home.search.platforms.manhuagui",
    value: "manhuagui",
  },
  { icon: jmIcon, nameKey: "home.search.platforms.jm", value: "jm" },
  {
    icon: nhentaiIcon,
    nameKey: "home.search.platforms.nhentai",
    value: "nhentai",
  },
];

const platformOptions = computed<DropdownOption[]>(() =>
  platforms.map((option) => ({
    key: option.value,
    label: t(option.nameKey),
    icon: () => {
      const nhentai = option.value === "nhentai";
      return h("img", {
        src: option.icon,
        alt: "",
        style: {
          width: "20px",
          height: "20px",
          borderRadius: "4px",
          objectFit: "contain",
          boxSizing: "border-box",
          background: nhentai ? "#0c0c0c" : "transparent",
          padding: nhentai ? "3px 2px" : "0",
        },
      });
    },
  })),
);

const platform = ref<ComicPlatform>("manhuagui");
const selectedPlatform = computed(
  () =>
    platforms.find((option) => option.value === platform.value) ?? platforms[0],
);
const platformTriggerLabel = computed(
  () => `${t("home.search.platform")}: ${t(selectedPlatform.value.nameKey)}`,
);
const query = ref("");
const page = ref(1);
const loading = ref(false);
const response = ref<ComicSearchResult | null>(null);
const error = ref<AppError | null>(null);
let requestId = 0;

const hasSearched = computed(
  () => loading.value || response.value !== null || error.value !== null,
);

const pageCount = computed(() => {
  const current = response.value;
  if (!current) {
    return 1;
  }
  return Math.max(
    1,
    Math.ceil(current.total / Math.max(1, current.page_size)),
  );
});

const errorMessage = computed(() =>
  error.value ? errorText(error.value) : "",
);

async function runSearch(targetPage: number) {
  const text = query.value.trim();
  if (!text || loading.value) {
    return;
  }

  const id = ++requestId;
  loading.value = true;
  error.value = null;
  page.value = targetPage;

  try {
    const result = await searchComics(platform.value, text, targetPage);
    if (id === requestId) {
      response.value = result;
    }
  } catch (cause) {
    if (id === requestId) {
      response.value = null;
      error.value = toAppError(cause);
    }
  } finally {
    if (id === requestId) {
      loading.value = false;
    }
  }
}

function search() {
  if (!query.value.trim() || loading.value) {
    return;
  }
  void runSearch(1);
}

function selectPlatform(next: ComicPlatform) {
  if (platform.value === next) {
    return;
  }

  // Ignore any in-flight response from the previous platform.
  platform.value = next;
  requestId++;
  response.value = null;
  error.value = null;
  page.value = 1;
  loading.value = false;
}

function selectPlatformFromDropdown(key: string | number) {
  const next = platforms.find((option) => option.value === String(key));
  if (next) {
    selectPlatform(next.value);
  }
}

function changePage(next: number) {
  if (next === page.value) {
    return;
  }
  void runSearch(next);
}

function metaFor(item: ComicSearchItem): string[] {
  const extra = item.extra;
  const lines: string[] = [];

  if (platform.value === "manhuagui") {
    const authors = Array.isArray(extra.authors)
      ? extra.authors.filter(
          (author): author is string => typeof author === "string",
        )
      : [];
    if (authors.length > 0) {
      lines.push(authors.join(", "));
    }
    if (typeof extra.latestChapter === "string" && extra.latestChapter) {
      lines.push(extra.latestChapter);
    }
    return lines;
  }

  if (platform.value === "jm") {
    const tags = Array.isArray(extra.tags)
      ? extra.tags.filter(
          (tag): tag is string => typeof tag === "string",
        )
      : [];
    if (tags.length > 0) {
      lines.push(tags.slice(0, 4).join(" · "));
    }
    return lines;
  }

  if (typeof extra.numPages === "number") {
    lines.push(t("home.search.pages", { count: extra.numPages }));
  }
  if (typeof extra.numFavorites === "number") {
    lines.push(t("home.search.favorites", { count: extra.numFavorites }));
  }
  return lines;
}
</script>

<template>
  <main class="home" :class="{ 'has-results': hasSearched }">
    <section class="hero">
      <form class="search-form" @submit.prevent="search">
        <NDropdown
          trigger="click"
          :options="platformOptions"
          placement="bottom-start"
          @select="selectPlatformFromDropdown"
        >
          <NButton
            attr-type="button"
            quaternary
            circle
            size="large"
            class="platform-trigger"
            :aria-label="platformTriggerLabel"
          >
            <template #icon>
              <img
                class="platform-trigger-icon"
                :class="{
                  'platform-trigger-icon--nhentai':
                    selectedPlatform.value === 'nhentai',
                }"
                :src="selectedPlatform.icon"
                alt=""
                draggable="false"
              />
            </template>
          </NButton>
        </NDropdown>
        <NInput
          v-model:value="query"
          class="search-input"
          size="large"
          :bordered="false"
          placeholder=""
          :input-props="{ 'aria-label': t('home.search.submit') }"
          :disabled="loading"
          clearable
          autofocus
        />
        <NButton
          attr-type="submit"
          type="primary"
          quaternary
          circle
          size="large"
          class="search-action"
          :loading="loading"
          :disabled="!query.trim()"
          :aria-label="t('home.search.submit')"
        >
          <template #icon>
            <SearchIcon />
          </template>
        </NButton>
      </form>
    </section>

    <section v-if="hasSearched" class="results">
      <NAlert
        v-if="errorMessage"
        type="error"
        :bordered="false"
        class="status"
      >
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
              <NCard
                v-for="item in response.result"
                :key="item.id"
                class="result-card"
                size="small"
                hoverable
              >
                <template #cover>
                  <ComicThumb
                    :platform="platform"
                    :thumb="item.thumb"
                    :title="item.title"
                  />
                </template>
                <h2 class="result-title" :title="item.title">
                  {{ item.title }}
                </h2>
                <p
                  v-for="(line, index) in metaFor(item)"
                  :key="index"
                  class="result-meta"
                >
                  {{ line }}
                </p>
              </NCard>
            </div>

            <NPagination
              v-if="pageCount > 1"
              class="pagination"
              :page="page"
              :page-count="pageCount"
              @update:page="changePage"
            />
          </template>
        </div>
      </NSpin>

      <div v-else class="status">
        <NSpin size="large" />
      </div>
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

.home.has-results {
  padding-top: 1rem;
}

.home.has-results .hero {
  align-items: stretch;
  min-height: 0;
  gap: 0.5rem;
  padding-top: 0.25rem;
  text-align: left;
}

.home.has-results .search-form {
  align-self: flex-start;
}

.platform-trigger {
  flex: none;
}

.platform-trigger-icon {
  display: block;
  width: 30px;
  height: 30px;
  border-radius: 4px;
  object-fit: contain;
}

.platform-trigger-icon--nhentai {
  padding: 3px 2px;
  border-radius: 50%;
  background: #0c0c0c;
  box-sizing: border-box;
}

.platform-trigger :deep(.n-button__icon),
.search-action :deep(.n-button__icon) {
  margin-right: 0;
}

.search-form {
  display: flex;
  gap: 0.1rem;
  align-items: center;
  width: min(100%, 760px);
  margin-top: 0.5rem;
  padding: 0.35rem;
  border: 1px solid rgba(127, 127, 127, 0.28);
  border-radius: 999px;
  background: rgba(127, 127, 127, 0.07);
}

.search-input {
  flex: 1;
  min-width: 0;
  text-align: left;
  background-color: transparent !important;
  box-shadow: none !important;
  --n-color: transparent !important;
  --n-color-focus: transparent !important;
  --n-padding-left: 3px !important;
  --n-padding-right: 3px !important;
}

.search-input :deep(.n-input-wrapper) {
  padding-left: 3px !important;
  padding-right: 3px !important;
}

.search-input :deep(.n-input__border),
.search-input :deep(.n-input__state-border) {
  display: none !important;
}

.search-input :deep(.n-input__input-el) {
  text-align: left;
}

.search-action {
  flex: none;
}

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

.result-card {
  overflow: hidden;
}

.result-title {
  display: -webkit-box;
  overflow: hidden;
  margin: 0.6rem 0 0.35rem;
  font-size: 0.95rem;
  font-weight: 600;
  line-height: 1.35;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.result-meta {
  margin: 0 0 0.2rem;
  overflow: hidden;
  font-size: 0.8rem;
  opacity: 0.65;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pagination {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
}

.status {
  display: flex;
  justify-content: center;
  padding: 2rem 0;
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
