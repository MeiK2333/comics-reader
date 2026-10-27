<script setup lang="ts">
import { computed, watch } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import type { ComicPlatform } from "../api/comics";
import { isComicPlatform } from "../api/comics";
import FavoriteGroupPicker from "../components/favorites/FavoriteGroupPicker.vue";
import ComicDetailSkeleton from "../components/comic-detail/ComicDetailSkeleton.vue";
import DetailActions from "../components/comic-detail/DetailActions.vue";
import DetailTopbar from "../components/comic-detail/DetailTopbar.vue";
import OfflineDetail from "../components/comic-detail/OfflineDetail.vue";
import { useBackNavigation } from "../composables/useBackNavigation";
import { useDownloadedComicDetail } from "../composables/useDownloadedComicDetail";
import { useErrorMessage } from "../composables/useErrorMessage";
import { useFavoriteAction } from "../composables/useFavoriteAction";
import { useAppI18n } from "../i18n";
import { setWindowTitle, updateWindowTitle } from "../router";

const route = useRoute();
const router = useRouter();
const { t } = useAppI18n();
const formatError = useErrorMessage();
const { goBack: back } = useBackNavigation();

const platform = computed<ComicPlatform | null>(() => {
  const value = route.params.platform;
  return isComicPlatform(value) ? value : null;
});

const id = computed(() => {
  const value = route.params.id;
  return typeof value === "string" ? value : "";
});

const comic = useDownloadedComicDetail({ platform, id });
const {
  download,
  loading,
  error,
  startChapterId,
  canRead,
  canResume,
  resumeChapterId,
  resumePage,
  resumeTitle,
  favoriteSnapshot,
  load,
} = comic;

const favorite = useFavoriteAction({
  platform,
  id,
  detail: favoriteSnapshot,
});
const {
  favorited,
  currentGroupId,
  loading: favoriteLoading,
  pending: favoritePending,
  showGroupPicker,
  favoriteButtonText,
  favoriteActionLabel,
  handlers: favoriteHandlers,
  handleClick: handleFavoriteClick,
  onSaved: onGroupSaved,
} = favorite;

const notFound = computed(
  () => error.value?.code === "notFound" || error.value?.code === "invalidInput",
);
const readerActive = computed(() => route.name === "download-reader");
const errorTitle = computed(() =>
  notFound.value ? t("detail.downloadMissing") : t("detail.loadFailed"),
);
const errorText = computed(() => {
  const current = error.value;
  if (!current) {
    return "";
  }
  return notFound.value
    ? t("detail.downloadMissingDescription")
    : formatError(current);
});

/** Restore the downloaded-detail title after the nested reader closes. */
function syncTitle() {
  if (route.name !== "download-detail") {
    return;
  }
  const current = download.value;
  if (current) {
    setWindowTitle(`${current.title} · ${t("app.name")}`);
    return;
  }
  updateWindowTitle();
}

watch([download, error, () => route.name], syncTitle, { immediate: true });

function goBack() {
  back({ name: "downloads" });
}

function openChapter(chapterId: string) {
  const current = download.value;
  if (!current) {
    return;
  }
  void router.push({
    name: "download-reader",
    params: {
      platform: current.platform,
      id: current.id,
      chapterId,
    },
  });
}

function openReader() {
  const current = download.value;
  if (!current) {
    return;
  }

  const params: Record<string, string> = {
    platform: current.platform,
    id: current.id,
  };
  const chapterId = startChapterId.value;
  if (chapterId) {
    params.chapterId = chapterId;
  }
  void router.push({ name: "download-reader", params });
}

function openResume() {
  const current = download.value;
  const entry = comic.historyEntry.value;
  if (!current || !entry) {
    return;
  }

  const params: Record<string, string> = {
    platform: current.platform,
    id: current.id,
  };
  const chapterId = resumeChapterId.value;
  if (chapterId) {
    params.chapterId = chapterId;
  }
  const page = resumePage.value;

  void router.push({
    name: "download-reader",
    params,
    query: page > 1 ? { page: String(page) } : {},
  });
}
</script>

<template>
  <main
    class="detail-page"
    :inert="readerActive || undefined"
    :aria-hidden="readerActive || undefined"
  >
    <DetailTopbar
      :back-label="t('detail.backToDownloads')"
      :loading="loading"
      @back="goBack"
    >
      <DetailActions
        v-if="download"
        :show-favorite="true"
        :favorited="favorited"
        :favorite-loading="favoriteLoading"
        :favorite-pending="favoritePending"
        :favorite-action-label="favoriteActionLabel"
        :favorite-button-text="favoriteButtonText"
        :favorite-handlers="favoriteHandlers"
        :show-download="false"
        :can-read="canRead"
        :can-resume="canResume"
        :download-loading="false"
        :download-pending="false"
        :download-button-label="''"
        :resume-title="resumeTitle"
        @favorite="handleFavoriteClick"
        @read="openReader"
        @resume="openResume"
      />
    </DetailTopbar>

    <FavoriteGroupPicker
      v-if="download"
      v-model:show="showGroupPicker"
      :platform="download.platform"
      :id="id"
      :title="download.title"
      :thumb="download.thumb ?? ''"
      :favorited="favorited"
      :current-group-id="currentGroupId"
      @saved="onGroupSaved"
    />

    <ComicDetailSkeleton v-if="loading" />

    <NResult
      v-else-if="error"
      class="detail-result"
      :status="notFound ? '404' : 'error'"
      :title="errorTitle"
      :description="errorText"
    >
      <template #footer>
        <div class="detail-actions">
          <NButton v-if="!notFound" type="primary" @click="load()">
            {{ t("detail.retry") }}
          </NButton>
          <NButton quaternary @click="goBack">
            {{ t("detail.backToDownloads") }}
          </NButton>
        </div>
      </template>
    </NResult>

    <OfflineDetail
      v-else-if="download"
      :detail="download"
      @open-chapter="openChapter"
    />

    <NResult
      v-else
      status="404"
      :title="t('detail.downloadMissing')"
      :description="t('detail.downloadMissingDescription')"
    />
  </main>

  <!-- Nested reader: the downloaded detail stays mounted underneath. -->
  <RouterView />
</template>

<style scoped>
.detail-page {
  width: min(960px, calc(100% - 2.5rem));
  margin: 0 auto;
  padding: 1.25rem 1.25rem 4rem;
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
