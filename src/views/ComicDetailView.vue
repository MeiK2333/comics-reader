<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import type {
  ComicDetail,
  ComicPlatform,
  JmDetail as JmDetailData,
  ManhuaguiDetail as ManhuaguiDetailData,
  NhentaiDetail as NhentaiDetailData,
} from "../api/comics";
import { getComic, isComicPlatform } from "../api/comics";
import JmDetail from "../components/comic-detail/JmDetail.vue";
import ManhuaguiDetail from "../components/comic-detail/ManhuaguiDetail.vue";
import NhentaiDetail from "../components/comic-detail/NhentaiDetail.vue";
import { useErrorMessage } from "../composables/useErrorMessage";
import { AppError, toAppError } from "../errors";
import { useAppI18n } from "../i18n";
import { setWindowTitle, updateWindowTitle } from "../router";
import { useSearchStore } from "../stores/search";

const route = useRoute();
const router = useRouter();
const search = useSearchStore();
const { t } = useAppI18n();
const formatError = useErrorMessage();

const detail = ref<ComicDetail | null>(null);
const loading = ref(true);
const error = ref<AppError | null>(null);
let requestId = 0;

const platform = computed<ComicPlatform | null>(() => {
  const value = route.params.platform;
  return isComicPlatform(value) ? value : null;
});

const id = computed(() => {
  const value = route.params.id;
  return typeof value === "string" ? value : "";
});

const canReturnToSearch = computed(() => search.active);
const backTarget = computed(() =>
  canReturnToSearch.value ? { name: "search" } : { name: "home" },
);
const backLabel = computed(() =>
  t(canReturnToSearch.value ? "detail.backToSearch" : "detail.backHome"),
);
// Only nhentai has a reader page source so far. Keeping the entry point gated
// avoids sending users into an unsupported reader for the other platforms.
const canRead = computed(
  () => detail.value?.platform === "nhentai" && platform.value !== null,
);

const manhuaguiDetail = computed<ManhuaguiDetailData | null>(() =>
  detail.value?.platform === "manhuagui" ? detail.value : null,
);
const jmDetail = computed<JmDetailData | null>(() =>
  detail.value?.platform === "jm" ? detail.value : null,
);
const nhentaiDetail = computed<NhentaiDetailData | null>(() =>
  detail.value?.platform === "nhentai" ? detail.value : null,
);

const notFound = computed(
  () =>
    error.value?.code === "notFound" || error.value?.code === "invalidInput",
);
const errorTitle = computed(() =>
  notFound.value ? t("detail.notFound") : t("detail.loadFailed"),
);
const errorText = computed(() => {
  const current = error.value;
  if (!current) {
    return "";
  }
  return notFound.value
    ? t("detail.notFoundDescription")
    : formatError(current);
});

function goBack() {
  void router.push(backTarget.value);
}

function openReader() {
  if (!detail.value || detail.value.platform !== "nhentai") {
    return;
  }
  void router.push({
    name: "comic-reader",
    params: { platform: detail.value.platform, id: id.value },
  });
}

async function load() {
  const currentRequest = ++requestId;
  const currentPlatform = platform.value;
  const currentId = id.value;

  loading.value = true;
  error.value = null;
  detail.value = null;
  // Drop a previous comic's dynamic title while the next request is running.
  updateWindowTitle();

  if (!currentPlatform || !currentId) {
    error.value = new AppError(
      "invalidInput",
      "invalid comic detail route",
      { field: "id" },
    );
    loading.value = false;
    return;
  }

  try {
    const result = await getComic(currentPlatform, currentId);
    if (currentRequest !== requestId) {
      return;
    }
    detail.value = result;
    setWindowTitle(`${result.title} · ${t("app.name")}`);
  } catch (cause) {
    if (currentRequest !== requestId) {
      return;
    }
    error.value = toAppError(cause);
    updateWindowTitle();
  } finally {
    if (currentRequest === requestId) {
      loading.value = false;
    }
  }
}

watch([platform, id], () => void load(), { immediate: true });

onBeforeUnmount(() => {
  requestId++;
});
</script>

<template>
  <main class="detail-page">
    <header class="detail-topbar">
      <NButton
        attr-type="button"
        quaternary
        circle
        size="large"
        class="back-button"
        :aria-label="backLabel"
        :title="backLabel"
        @click="goBack"
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
      <NButton
        v-if="canRead"
        attr-type="button"
        type="primary"
        @click="openReader"
      >
        {{ t("detail.read") }}
      </NButton>
    </header>

    <div v-if="loading" class="detail-loading" aria-busy="true">
      <NSpin size="large" />
    </div>

    <NResult
      v-else-if="error"
      class="detail-result"
      :status="notFound ? '404' : 'error'"
      :title="errorTitle"
      :description="errorText"
    >
      <template #footer>
        <div class="detail-actions">
          <NButton v-if="!notFound" type="primary" @click="load">
            {{ t("detail.retry") }}
          </NButton>
          <NButton quaternary @click="goBack">{{ backLabel }}</NButton>
        </div>
      </template>
    </NResult>

    <ManhuaguiDetail v-else-if="manhuaguiDetail" :detail="manhuaguiDetail" />
    <JmDetail v-else-if="jmDetail" :detail="jmDetail" />
    <NhentaiDetail v-else-if="nhentaiDetail" :detail="nhentaiDetail" />

    <NResult
      v-else
      status="404"
      :title="t('detail.notFound')"
      :description="t('detail.notFoundDescription')"
    />
  </main>
</template>

<style scoped>
.detail-page {
  width: min(960px, calc(100% - 2.5rem));
  margin: 0 auto;
  padding: 1.25rem 1.25rem 4rem;
}

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

.detail-loading {
  display: flex;
  justify-content: center;
  padding: 4rem 1rem;
}

.detail-result {
  padding: 2rem 0;
}

.detail-actions {
  display: flex;
  justify-content: center;
  gap: 0.75rem;
}

@media (max-width: 560px) {
  .detail-page {
    padding-top: 0.75rem;
  }
}
</style>
